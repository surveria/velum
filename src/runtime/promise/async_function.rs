use crate::{
    error::{Error, JavaScriptErrorMetadata, Result},
    runtime::{
        Context, VmStorageKind,
        call::RuntimeCallArgs,
        control::{Completion, Suspension, runtime_exception_value},
        roots::VmRootKind,
    },
    value::{ErrorName, FunctionId, Value},
};

use super::{
    PromiseId, PromiseJob, PromiseReaction, PromiseSettledState, PromiseState, job::PromiseStatus,
};

impl Context {
    pub(in crate::runtime) fn eval_async_function_with_this(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
        new_target: Value,
    ) -> Result<Value> {
        let (promise, object) = self.create_pending_promise()?;
        let _object_root =
            self.transient_root_scope(VmRootKind::TransientTemporary, core::iter::once(&object))?;
        self.with_active_async_promise(promise, |context| {
            let completion = context.eval_async_function_completion_with_this_and_new_target(
                id, args, this_value, new_target,
            )?;
            context.settle_or_suspend_async_function(id, promise, completion)
        })?;
        Ok(object)
    }

    fn with_active_async_promise<T>(
        &mut self,
        promise: PromiseId,
        run: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.active_async_function_promises
            .try_reserve(1)
            .map_err(|error| {
                Error::limit(format!(
                    "active async Promise root storage exhausted: {error}"
                ))
            })?;
        self.active_async_function_promises.push(promise);
        let result = run(self);
        if self.active_async_function_promises.pop() != Some(promise) {
            return Err(Error::runtime("active async Promise root mismatch"));
        }
        result
    }

    fn settle_or_suspend_async_function(
        &mut self,
        function: FunctionId,
        result_promise: PromiseId,
        completion: Completion,
    ) -> Result<()> {
        let completion = self.normalize_resumed_tail_call(completion)?;
        match completion {
            Completion::Normal(_) => self.resolve_promise(result_promise, Value::Undefined),
            Completion::Return(value) | Completion::ReturnDirect(value) => {
                self.resolve_promise(result_promise, value)
            }
            Completion::Throw(value) => self.reject_promise(result_promise, value),
            Completion::TailCall(_) => Err(Error::runtime("tail call escaped async function")),
            Completion::Break { .. } | Completion::Continue { .. } => {
                let reason = self.create_error_object(
                    JavaScriptErrorMetadata::new(
                        ErrorName::SyntaxError,
                        "invalid async function completion",
                    ),
                    true,
                )?;
                self.reject_promise(result_promise, reason)
            }
            Completion::Suspend(Suspension::Await(awaited)) => {
                let continuation =
                    self.detach_suspended_async_function(function, result_promise)?;
                self.add_async_await_reaction(awaited, continuation)
            }
            Completion::Suspend(Suspension::Yield(value)) => {
                let reason = self.create_error_object(
                    JavaScriptErrorMetadata::new(
                        ErrorName::TypeError,
                        format!("async function yielded unexpected value {value}"),
                    ),
                    true,
                )?;
                self.reject_promise(result_promise, reason)
            }
            Completion::Suspend(Suspension::DelegatedYield(delegated)) => {
                let value = delegated.root_value();
                let reason = self.create_error_object(
                    JavaScriptErrorMetadata::new(
                        ErrorName::TypeError,
                        format!("async function yielded unexpected value {value}"),
                    ),
                    true,
                )?;
                self.reject_promise(result_promise, reason)
            }
            Completion::Suspend(Suspension::GeneratorStart) => {
                Err(Error::runtime("async function entered generator start"))
            }
        }
    }

    pub(in crate::runtime) fn eval_bytecode_await(&mut self, value: Value) -> Result<Completion> {
        let promise = self.promise_resolve_for_await(value)?;
        Ok(Completion::Suspend(Suspension::Await(promise)))
    }

    pub(in crate::runtime) fn promise_resolve_for_await(
        &mut self,
        value: Value,
    ) -> Result<PromiseId> {
        let _value_root =
            self.transient_root_scope(VmRootKind::TransientTemporary, core::iter::once(&value))?;
        if let Ok(promise) = self.promise_id_from_value(&value) {
            let constructor = self.get_named(&value, "constructor")?;
            let intrinsic = self.promise_constructor_value()?;
            if constructor == intrinsic {
                return Ok(promise);
            }
        }
        let (promise, object) = self.create_pending_promise()?;
        let _promise_root =
            self.transient_root_scope(VmRootKind::TransientTemporary, [&value, &object])?;
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

    pub(super) fn resume_async_function(
        &mut self,
        continuation: crate::runtime::function::SuspendedAsyncFunction,
        state: PromiseSettledState,
    ) -> Result<()> {
        let function = continuation.function();
        let result_promise = continuation.result_promise();
        self.with_active_async_promise(result_promise, |context| {
            let resume = match state.status {
                PromiseStatus::Fulfilled => Completion::Normal(state.value),
                PromiseStatus::Rejected => Completion::Throw(state.value),
            };
            let completion = match context.resume_suspended_async_function(continuation, resume) {
                Ok(completion) => completion,
                Err(error) => {
                    let Some(reason) = runtime_exception_value(context, &error)? else {
                        return Err(error);
                    };
                    return context.reject_promise(result_promise, reason);
                }
            };
            context.settle_or_suspend_async_function(function, result_promise, completion)
        })
    }
}
