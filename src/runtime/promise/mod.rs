use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, control::Completion},
    value::{FunctionId, ObjectId, Value},
};

mod job;
mod state;

use job::PromiseStatus;
pub(in crate::runtime) use job::{PromiseJob, PromiseReaction, PromiseSettledState};
use state::PromiseState;
pub(in crate::runtime) use state::{Promise, PromiseId, PromiseResolverKind};

impl Context {
    pub(in crate::runtime) fn create_pending_promise(&mut self) -> Result<(PromiseId, Value)> {
        let id = PromiseId::new(self.promises.len());
        let object = self.create_promise_object(id)?;
        self.promises.push(Promise::pending());
        Ok((id, object))
    }

    pub(in crate::runtime) fn create_fulfilled_promise(&mut self, value: Value) -> Result<Value> {
        let (id, object) = self.create_pending_promise()?;
        self.fulfill_promise(id, value)?;
        Ok(object)
    }

    pub(in crate::runtime) fn create_rejected_promise(&mut self, reason: Value) -> Result<Value> {
        let (id, object) = self.create_pending_promise()?;
        self.reject_promise(id, reason)?;
        Ok(object)
    }

    pub(in crate::runtime) fn promise_id_from_value(&self, value: &Value) -> Result<PromiseId> {
        let Value::Object(object) = value else {
            return Err(Error::runtime(
                "Promise operation requires a Promise receiver",
            ));
        };
        self.promise_id_for_object(*object)
    }

    pub(in crate::runtime) fn promise_id_for_object(&self, object: ObjectId) -> Result<PromiseId> {
        self.promise_object_slots
            .get(object.index())
            .copied()
            .flatten()
            .ok_or_else(|| Error::runtime("Promise operation requires a Promise object"))
    }

    pub(in crate::runtime) fn resolve_promise(
        &mut self,
        promise: PromiseId,
        value: Value,
    ) -> Result<()> {
        if let Ok(adopted) = self.promise_id_from_value(&value) {
            if adopted == promise {
                return self.reject_promise(
                    promise,
                    Value::Error(crate::value::ErrorObject::new(
                        crate::value::ErrorName::TypeError,
                        "Promise cannot resolve to itself",
                    )),
                );
            }
            return self.adopt_promise(promise, adopted);
        }
        self.fulfill_promise(promise, value)
    }

    pub(in crate::runtime) fn reject_promise(
        &mut self,
        promise: PromiseId,
        reason: Value,
    ) -> Result<()> {
        self.settle_promise(promise, &PromiseSettledState::rejected(reason))
    }

    pub(in crate::runtime) fn fulfill_promise(
        &mut self,
        promise: PromiseId,
        value: Value,
    ) -> Result<()> {
        self.settle_promise(promise, &PromiseSettledState::fulfilled(value))
    }

    pub(in crate::runtime) fn add_promise_reaction(
        &mut self,
        promise: PromiseId,
        reaction: PromiseReaction,
    ) -> Result<()> {
        let state = self.promise_state(promise)?.clone();
        match state {
            PromiseState::Pending { .. } => {
                let PromiseState::Pending { reactions } = &mut self.promise_mut(promise)?.state
                else {
                    return Err(Error::runtime(
                        "Promise state changed while adding reaction",
                    ));
                };
                reactions.push(reaction);
            }
            PromiseState::Fulfilled(value) => {
                self.enqueue_promise_job(PromiseJob::Reaction {
                    reaction,
                    state: PromiseSettledState::fulfilled(value),
                });
            }
            PromiseState::Rejected(reason) => {
                self.enqueue_promise_job(PromiseJob::Reaction {
                    reaction,
                    state: PromiseSettledState::rejected(reason),
                });
            }
        }
        Ok(())
    }

    pub(in crate::runtime) fn eval_async_function_with_this(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
        new_target: Value,
    ) -> Result<Value> {
        let (promise, object) = self.create_pending_promise()?;
        match self
            .eval_function_completion_with_this_and_new_target(id, args, this_value, new_target)?
        {
            Completion::Normal(_) => self.resolve_promise(promise, Value::Undefined)?,
            Completion::Return(value) => self.resolve_promise(promise, value)?,
            Completion::Throw(value) => self.reject_promise(promise, value)?,
            Completion::Break { .. } | Completion::Continue(_) => {
                self.reject_promise(
                    promise,
                    Value::Error(crate::value::ErrorObject::new(
                        crate::value::ErrorName::SyntaxError,
                        "invalid async function completion",
                    )),
                )?;
            }
        }
        Ok(object)
    }

    pub(in crate::runtime) fn eval_bytecode_await(&mut self, value: Value) -> Result<Completion> {
        let Ok(promise) = self.promise_id_from_value(&value) else {
            return Ok(Completion::Normal(value));
        };
        self.drain_promise_jobs()?;
        match self.promise_state(promise)? {
            PromiseState::Fulfilled(value) => Ok(Completion::Normal(value.clone())),
            PromiseState::Rejected(value) => Ok(Completion::Throw(value.clone())),
            PromiseState::Pending { .. } => Err(Error::runtime(
                "awaited Promise is still pending after draining the job queue",
            )),
        }
    }

    pub(crate) fn drain_promise_jobs(&mut self) -> Result<()> {
        while let Some(job) = self.promise_jobs.pop_front() {
            self.step()?;
            self.run_promise_job(job)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn create_promise_resolving_function(
        &mut self,
        promise: PromiseId,
        kind: PromiseResolverKind,
    ) -> Result<Value> {
        self.create_ephemeral_native_function(
            crate::runtime::native::NativeFunctionKind::PromiseResolver { promise, kind },
            Value::Undefined,
        )
    }

    pub(in crate::runtime) fn eval_promise_resolver(
        &mut self,
        promise: PromiseId,
        kind: PromiseResolverKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        match kind {
            PromiseResolverKind::Resolve => self.resolve_promise(promise, value)?,
            PromiseResolverKind::Reject => self.reject_promise(promise, value)?,
        }
        Ok(Value::Undefined)
    }

    pub(in crate::runtime) fn promise_then(
        &mut self,
        promise: PromiseId,
        on_fulfilled: Option<Value>,
        on_rejected: Option<Value>,
    ) -> Result<Value> {
        let (result, object) = self.create_pending_promise()?;
        let reaction = PromiseReaction::new(result, on_fulfilled, on_rejected);
        self.add_promise_reaction(promise, reaction)?;
        Ok(object)
    }

    pub(in crate::runtime) fn promise_reaction_handler(
        &self,
        value: Option<&Value>,
    ) -> Result<Option<Value>> {
        let Some(value) = value else {
            return Ok(None);
        };
        if self.semantic_is_callable(value)? {
            return Ok(Some(value.clone()));
        }
        Ok(None)
    }

    fn create_promise_object(&mut self, promise: PromiseId) -> Result<Value> {
        let prototype = self.promise_constructor_prototype()?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype_id(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.remember_promise_object(object, promise)?;
        Ok(Value::Object(object))
    }

    fn remember_promise_object(&mut self, object: ObjectId, promise: PromiseId) -> Result<()> {
        let required_len = object
            .index()
            .checked_add(1)
            .ok_or_else(|| Error::limit("Promise object slot index overflowed"))?;
        if self.promise_object_slots.len() < required_len {
            self.promise_object_slots.resize(required_len, None);
        }
        let slot = self
            .promise_object_slots
            .get_mut(object.index())
            .ok_or_else(|| Error::runtime("Promise object slot is not defined"))?;
        *slot = Some(promise);
        Ok(())
    }

    fn adopt_promise(&mut self, promise: PromiseId, adopted: PromiseId) -> Result<()> {
        let reaction = PromiseReaction::new(promise, None, None);
        self.add_promise_reaction(adopted, reaction)
    }

    fn settle_promise(&mut self, promise: PromiseId, state: &PromiseSettledState) -> Result<()> {
        let reactions = {
            let promise = self.promise_mut(promise)?;
            let PromiseState::Pending { reactions } = &mut promise.state else {
                return Ok(());
            };
            let reactions = std::mem::take(reactions);
            promise.state = match state.status {
                PromiseStatus::Fulfilled => PromiseState::Fulfilled(state.value.clone()),
                PromiseStatus::Rejected => PromiseState::Rejected(state.value.clone()),
            };
            reactions
        };
        for reaction in reactions {
            self.enqueue_promise_job(PromiseJob::Reaction {
                reaction,
                state: (*state).clone(),
            });
        }
        Ok(())
    }

    fn enqueue_promise_job(&mut self, job: PromiseJob) {
        self.promise_jobs.push_back(job);
    }

    fn run_promise_job(&mut self, job: PromiseJob) -> Result<()> {
        match job {
            PromiseJob::Reaction { reaction, state } => self.run_promise_reaction(reaction, state),
        }
    }

    fn run_promise_reaction(
        &mut self,
        reaction: PromiseReaction,
        state: PromiseSettledState,
    ) -> Result<()> {
        let handler = match state.status {
            PromiseStatus::Fulfilled => reaction.on_fulfilled,
            PromiseStatus::Rejected => reaction.on_rejected,
        };
        let Some(handler) = handler else {
            return match state.status {
                PromiseStatus::Fulfilled => self.resolve_promise(reaction.result, state.value),
                PromiseStatus::Rejected => self.reject_promise(reaction.result, state.value),
            };
        };
        match self.eval_call_value(&handler, &[state.value], Value::Undefined) {
            Ok(value) => self.resolve_promise(reaction.result, value),
            Err(error) => self.reject_promise(
                reaction.result,
                Value::Error(crate::value::ErrorObject::new(
                    crate::value::ErrorName::Base,
                    error.to_string(),
                )),
            ),
        }
    }

    fn promise_state(&self, id: PromiseId) -> Result<&PromiseState> {
        self.promises
            .get(id.index())
            .map(|promise| &promise.state)
            .ok_or_else(|| Error::runtime("Promise id is not defined"))
    }

    fn promise_mut(&mut self, id: PromiseId) -> Result<&mut Promise> {
        self.promises
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("Promise id is not defined"))
    }
}
