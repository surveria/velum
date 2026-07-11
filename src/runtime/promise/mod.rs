use crate::{
    error::{Error, JavaScriptErrorMetadata, Result},
    runtime::{
        Context, VmStorageKind,
        call::RuntimeCallArgs,
        control::{Completion, runtime_exception_value},
    },
    value::{ErrorName, FunctionId, ObjectId, Value},
};

mod job;
mod state;

use job::PromiseStatus;
pub(in crate::runtime) use job::{
    PromiseContinuationCancellation, PromiseJob, PromiseReaction, PromiseSettledState,
};
use state::PromiseState;
pub(in crate::runtime) use state::{Promise, PromiseId, PromiseResolverKind};

impl Context {
    pub(in crate::runtime) fn promise_reaction_count(&self) -> Result<usize> {
        self.promises.iter().try_fold(0_usize, |count, promise| {
            let reaction_count = match &promise.state {
                PromiseState::Pending { reactions } => reactions.len(),
                PromiseState::Fulfilled(_) | PromiseState::Rejected(_) => 0,
            };
            count
                .checked_add(reaction_count)
                .ok_or_else(|| Error::limit("Promise reaction count overflowed"))
        })
    }

    pub(in crate::runtime) fn suspended_async_execution_frame_count(&self) -> Result<usize> {
        let promise_frames = self.promises.iter().try_fold(0_usize, |count, promise| {
            count
                .checked_add(promise.suspended_execution_frame_count()?)
                .ok_or_else(|| Error::limit("suspended execution frame count overflowed"))
        })?;
        self.promise_jobs
            .iter()
            .try_fold(promise_frames, |count, job| {
                count
                    .checked_add(job.execution_frame_count()?)
                    .ok_or_else(|| Error::limit("suspended execution frame count overflowed"))
            })
    }

    pub(in crate::runtime) fn suspended_async_cache_entry_count(&self) -> Result<usize> {
        let promise_entries = self.promises.iter().try_fold(0_usize, |count, promise| {
            count
                .checked_add(promise.suspended_cache_entry_count()?)
                .ok_or_else(|| Error::limit("suspended cache entry count overflowed"))
        })?;
        self.promise_jobs
            .iter()
            .try_fold(promise_entries, |count, job| {
                count
                    .checked_add(job.cache_entry_count()?)
                    .ok_or_else(|| Error::limit("suspended cache entry count overflowed"))
            })
    }

    pub(in crate::runtime) fn create_pending_promise(&mut self) -> Result<(PromiseId, Value)> {
        self.promises.reserve_insert()?;
        self.storage_ledger.grow_count(VmStorageKind::Promise, 1)?;
        let id = PromiseId::new(self.promises.next_index());
        let object = match self.create_promise_object(id) {
            Ok(object) => object,
            Err(error) => {
                self.storage_ledger
                    .release_count(VmStorageKind::Promise, 1)?;
                return Err(error);
            }
        };
        if let Err(error) = self.promises.insert_at_next(id.index(), Promise::pending()) {
            self.storage_ledger
                .release_count(VmStorageKind::Promise, 1)?;
            return Err(error);
        }
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
                let reason = self.create_error_object(
                    JavaScriptErrorMetadata::new(
                        ErrorName::TypeError,
                        "Promise cannot resolve to itself",
                    ),
                    true,
                )?;
                return self.reject_promise(promise, reason);
            }
            return self.adopt_promise(promise, adopted);
        }
        if self.semantic_object_ref(&value)?.is_some() {
            let then = match self.get_named(&value, "then") {
                Ok(then) => then,
                Err(error) => {
                    let Some(reason) = runtime_exception_value(self, &error)? else {
                        return Err(error);
                    };
                    return self.reject_promise(promise, reason);
                }
            };
            if self.semantic_is_callable(&then)? {
                return self.enqueue_promise_job(PromiseJob::ResolveThenable {
                    promise,
                    thenable: value,
                    then,
                });
            }
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
        let settled = match self.promise_state(promise)? {
            PromiseState::Pending { .. } => None,
            PromiseState::Fulfilled(value) => Some(PromiseSettledState::fulfilled(value.clone())),
            PromiseState::Rejected(reason) => Some(PromiseSettledState::rejected(reason.clone())),
        };
        if let Some(state) = settled {
            return self.enqueue_promise_job(PromiseJob::Reaction { reaction, state });
        }
        self.storage_ledger
            .grow_count(VmStorageKind::PromiseReaction, 1)?;
        let PromiseState::Pending { reactions } = &mut self.promise_mut(promise)?.state else {
            self.storage_ledger
                .release_count(VmStorageKind::PromiseReaction, 1)?;
            return Err(Error::runtime(
                "Promise state changed while adding reaction",
            ));
        };
        reactions.push(reaction);
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
        match self.eval_async_function_completion_with_this_and_new_target(
            id, args, this_value, new_target,
        )? {
            Completion::Normal(_) => self.resolve_promise(promise, Value::Undefined)?,
            Completion::Return(value) | Completion::ReturnDirect(value) => {
                self.resolve_promise(promise, value)?;
            }
            Completion::Throw(value) => self.reject_promise(promise, value)?,
            Completion::Break { .. } | Completion::Continue(_) => {
                let reason = self.create_error_object(
                    JavaScriptErrorMetadata::new(
                        ErrorName::SyntaxError,
                        "invalid async function completion",
                    ),
                    true,
                )?;
                self.reject_promise(promise, reason)?;
            }
            Completion::Suspended(awaited) => {
                let continuation = self.detach_suspended_async_function(id, promise)?;
                self.add_async_await_reaction(awaited, continuation)?;
            }
            Completion::Yielded(value) | Completion::YieldedIteratorResult(value) => {
                let reason = self.create_error_object(
                    JavaScriptErrorMetadata::new(
                        ErrorName::TypeError,
                        format!("async function yielded unexpected value {value}"),
                    ),
                    true,
                )?;
                self.reject_promise(promise, reason)?;
            }
            Completion::GeneratorStart => {
                return Err(Error::runtime("async function entered generator start"));
            }
        }
        Ok(object)
    }

    pub(in crate::runtime) fn eval_bytecode_await(&mut self, value: Value) -> Result<Completion> {
        let promise = self.promise_resolve_for_await(value)?;
        Ok(Completion::Suspended(promise))
    }

    fn promise_resolve_for_await(&mut self, value: Value) -> Result<PromiseId> {
        if let Ok(promise) = self.promise_id_from_value(&value) {
            let constructor = self.get_named(&value, "constructor")?;
            let intrinsic = self.promise_constructor_value()?;
            if constructor == intrinsic {
                return Ok(promise);
            }
        }
        let (promise, _object) = self.create_pending_promise()?;
        self.resolve_promise(promise, value)?;
        Ok(promise)
    }

    fn add_async_await_reaction(
        &mut self,
        promise: PromiseId,
        continuation: crate::runtime::function::SuspendedAsyncFunction,
    ) -> Result<()> {
        let settled = match self.promise_state(promise)? {
            PromiseState::Pending { .. } => None,
            PromiseState::Fulfilled(value) => Some(PromiseSettledState::fulfilled(value.clone())),
            PromiseState::Rejected(reason) => Some(PromiseSettledState::rejected(reason.clone())),
        };
        let storage_kind = if settled.is_some() {
            VmStorageKind::PromiseJob
        } else {
            VmStorageKind::PromiseReaction
        };
        if let Err(error) = self.storage_ledger.grow_count(storage_kind, 1) {
            continuation.cancel_storage(&self.storage_ledger)?;
            return Err(error);
        }
        let reaction = PromiseReaction::awaiting(continuation);
        if let Some(state) = settled {
            self.promise_jobs
                .push_back(PromiseJob::Reaction { reaction, state });
            return Ok(());
        }
        let PromiseState::Pending { reactions } = &mut self.promise_mut(promise)?.state else {
            self.storage_ledger.release_count(storage_kind, 1)?;
            let Some(continuation) = reaction.into_suspended() else {
                return Err(Error::runtime("async await reaction disappeared"));
            };
            continuation.cancel_storage(&self.storage_ledger)?;
            return Err(Error::runtime(
                "Promise state changed while adding await reaction",
            ));
        };
        reactions.push(reaction);
        Ok(())
    }

    /// Runs queued Promise reactions until the VM job queue is empty.
    ///
    /// Returns the number of jobs executed, including jobs enqueued by an
    /// earlier job in the same drain.
    ///
    /// # Errors
    /// Fails when a job raises an unhandled runtime or resource-limit error.
    pub fn run_jobs(&mut self) -> Result<usize> {
        let mut count = 0_usize;
        while let Some(job) = self.promise_jobs.pop_front() {
            self.storage_ledger
                .release_count(VmStorageKind::PromiseJob, 1)?;
            self.step()?;
            self.run_promise_job(job)?;
            count = count
                .checked_add(1)
                .ok_or_else(|| Error::limit("Promise job execution count overflowed"))?;
        }
        Ok(count)
    }

    /// Returns the number of Promise jobs currently ready to run.
    #[must_use]
    pub fn pending_job_count(&self) -> usize {
        self.promise_jobs.len()
    }

    /// Discards every ready Promise job and every reaction waiting on a
    /// pending Promise, including parked async function activations.
    ///
    /// Pending Promise objects remain pending. Their discarded handlers will
    /// not run if the Promise is settled later. This is an embedder shutdown
    /// and cancellation boundary, not a JavaScript-visible rejection.
    ///
    /// Returns the number of discarded ready jobs and pending reactions.
    ///
    /// # Errors
    /// Fails if VM storage-accounting invariants cannot be reconciled.
    pub fn cancel_jobs(&mut self) -> Result<usize> {
        let mut reaction_count = 0_usize;
        let mut cancellations = Vec::new();
        for promise in &mut self.promises {
            let PromiseState::Pending { reactions } = &mut promise.state else {
                continue;
            };
            let removed = std::mem::take(reactions);
            reaction_count = reaction_count
                .checked_add(removed.len())
                .ok_or_else(|| Error::limit("Promise cancellation count overflowed"))?;
            cancellations.extend(
                removed
                    .into_iter()
                    .filter_map(PromiseReaction::into_cancellation),
            );
        }
        let jobs = std::mem::take(&mut self.promise_jobs);
        let job_count = jobs.len();
        cancellations.extend(jobs.into_iter().filter_map(PromiseJob::into_cancellation));

        self.storage_ledger
            .release_count(VmStorageKind::PromiseReaction, reaction_count)?;
        self.storage_ledger
            .release_count(VmStorageKind::PromiseJob, job_count)?;
        let storage_ledger = self.storage_ledger.clone();
        for cancellation in cancellations {
            match cancellation {
                PromiseContinuationCancellation::AsyncFunction(continuation) => {
                    continuation.cancel_storage(&storage_ledger)?;
                }
                PromiseContinuationCancellation::AsyncGenerator(generator) => {
                    self.cancel_async_generator_await(generator)?;
                }
            }
        }
        reaction_count
            .checked_add(job_count)
            .ok_or_else(|| Error::limit("Promise cancellation count overflowed"))
    }

    pub(crate) fn drain_promise_jobs(&mut self) -> Result<()> {
        self.run_jobs().map(|_count| ())
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
        let adds_association = self
            .promise_object_slots
            .get(object.index())
            .and_then(Option::as_ref)
            .is_none();
        if adds_association {
            self.storage_ledger
                .grow_count(VmStorageKind::Association, 1)?;
        }
        if self.promise_object_slots.len() < required_len {
            self.promise_object_slots.resize(required_len, None);
        }
        let slot = self
            .promise_object_slots
            .get_mut(object.index())
            .ok_or_else(|| Error::runtime("Promise object slot is not defined"));
        let slot = match slot {
            Ok(slot) => slot,
            Err(error) => {
                if adds_association {
                    self.storage_ledger
                        .release_count(VmStorageKind::Association, 1)?;
                }
                return Err(error);
            }
        };
        *slot = Some(promise);
        Ok(())
    }

    fn adopt_promise(&mut self, promise: PromiseId, adopted: PromiseId) -> Result<()> {
        let reaction = PromiseReaction::new(promise, None, None);
        self.add_promise_reaction(adopted, reaction)
    }

    fn settle_promise(&mut self, promise: PromiseId, state: &PromiseSettledState) -> Result<()> {
        let reaction_count = match self.promise_state(promise)? {
            PromiseState::Pending { reactions } => reactions.len(),
            PromiseState::Fulfilled(_) | PromiseState::Rejected(_) => return Ok(()),
        };
        self.storage_ledger
            .grow_count(VmStorageKind::PromiseJob, reaction_count)?;
        let reactions = {
            let promise = self.promise_mut(promise)?;
            let PromiseState::Pending { reactions } = &mut promise.state else {
                self.storage_ledger
                    .release_count(VmStorageKind::PromiseJob, reaction_count)?;
                return Ok(());
            };
            let reactions = std::mem::take(reactions);
            promise.state = match state.status {
                PromiseStatus::Fulfilled => PromiseState::Fulfilled(state.value.clone()),
                PromiseStatus::Rejected => PromiseState::Rejected(state.value.clone()),
            };
            reactions
        };
        self.storage_ledger
            .release_count(VmStorageKind::PromiseReaction, reaction_count)?;
        for reaction in reactions {
            self.promise_jobs.push_back(PromiseJob::Reaction {
                reaction,
                state: (*state).clone(),
            });
        }
        Ok(())
    }

    fn enqueue_promise_job(&mut self, job: PromiseJob) -> Result<()> {
        self.storage_ledger
            .grow_count(VmStorageKind::PromiseJob, 1)?;
        self.promise_jobs.push_back(job);
        Ok(())
    }

    fn run_promise_job(&mut self, job: PromiseJob) -> Result<()> {
        match job {
            PromiseJob::Reaction { reaction, state } => self.run_promise_reaction(reaction, state),
            PromiseJob::ResolveThenable {
                promise,
                thenable,
                then,
            } => self.run_promise_resolve_thenable(promise, thenable, &then),
        }
    }

    fn run_promise_resolve_thenable(
        &mut self,
        promise: PromiseId,
        thenable: Value,
        then: &Value,
    ) -> Result<()> {
        let resolve =
            self.create_promise_resolving_function(promise, PromiseResolverKind::Resolve)?;
        let reject =
            self.create_promise_resolving_function(promise, PromiseResolverKind::Reject)?;
        match self.call_value(then, &[resolve, reject], thenable) {
            Ok(_) => Ok(()),
            Err(error) => {
                let Some(reason) = runtime_exception_value(self, &error)? else {
                    return Err(error);
                };
                self.reject_promise(promise, reason)
            }
        }
    }

    fn run_promise_reaction(
        &mut self,
        reaction: PromiseReaction,
        state: PromiseSettledState,
    ) -> Result<()> {
        let PromiseReaction::Then {
            result,
            on_fulfilled,
            on_rejected,
        } = reaction
        else {
            return match reaction {
                PromiseReaction::Await { continuation } => {
                    self.resume_async_function(*continuation, state)
                }
                PromiseReaction::AsyncGeneratorAwait { generator } => {
                    let resume = match state.status {
                        PromiseStatus::Fulfilled => Completion::Normal(state.value),
                        PromiseStatus::Rejected => Completion::Throw(state.value),
                    };
                    self.resume_async_generator_await(generator, resume)
                }
                PromiseReaction::Then { .. } => {
                    Err(Error::runtime("Promise reaction kind disappeared"))
                }
            };
        };
        let handler = match state.status {
            PromiseStatus::Fulfilled => on_fulfilled,
            PromiseStatus::Rejected => on_rejected,
        };
        let Some(handler) = handler else {
            return match state.status {
                PromiseStatus::Fulfilled => self.resolve_promise(result, state.value),
                PromiseStatus::Rejected => self.reject_promise(result, state.value),
            };
        };
        match self.call_value(&handler, &[state.value], Value::Undefined) {
            Ok(value) => self.resolve_promise(result, value),
            Err(error) => {
                let Some(reason) = runtime_exception_value(self, &error)? else {
                    return Err(error);
                };
                self.reject_promise(result, reason)
            }
        }
    }

    fn resume_async_function(
        &mut self,
        continuation: crate::runtime::function::SuspendedAsyncFunction,
        state: PromiseSettledState,
    ) -> Result<()> {
        let function = continuation.function();
        let result_promise = continuation.result_promise();
        let resume = match state.status {
            PromiseStatus::Fulfilled => Completion::Normal(state.value),
            PromiseStatus::Rejected => Completion::Throw(state.value),
        };
        let completion = match self.resume_suspended_async_function(continuation, resume) {
            Ok(completion) => completion,
            Err(error) => {
                let Some(reason) = runtime_exception_value(self, &error)? else {
                    return Err(error);
                };
                return self.reject_promise(result_promise, reason);
            }
        };
        match completion {
            Completion::Normal(_) => self.resolve_promise(result_promise, Value::Undefined),
            Completion::Return(value) | Completion::ReturnDirect(value) => {
                self.resolve_promise(result_promise, value)
            }
            Completion::Throw(value) => self.reject_promise(result_promise, value),
            Completion::Suspended(awaited) => {
                let continuation =
                    self.detach_suspended_async_function(function, result_promise)?;
                self.add_async_await_reaction(awaited, continuation)
            }
            Completion::Yielded(value) | Completion::YieldedIteratorResult(value) => {
                let reason = self.create_error_object(
                    JavaScriptErrorMetadata::new(
                        ErrorName::TypeError,
                        format!("async function yielded unexpected value {value}"),
                    ),
                    true,
                )?;
                self.reject_promise(result_promise, reason)
            }
            Completion::GeneratorStart => Err(Error::runtime(
                "async function resumed into generator start",
            )),
            Completion::Break { .. } | Completion::Continue(_) => {
                let reason = self.create_error_object(
                    JavaScriptErrorMetadata::new(
                        ErrorName::SyntaxError,
                        "invalid async function completion",
                    ),
                    true,
                )?;
                self.reject_promise(result_promise, reason)
            }
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
