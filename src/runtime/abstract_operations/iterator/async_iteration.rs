use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        control::{Completion, runtime_exception_value},
        promise::{PromiseId, PromiseReaction},
        roots::VmRootKind,
    },
    value::Value,
};

use super::{
    ITERATOR_RESULT_DONE_PROPERTY, ITERATOR_RESULT_VALUE_PROPERTY, ITERATOR_RETURN_PROPERTY,
    IteratorSource, IteratorStep, is_resource_limit, protocol_iterator_to_close, set_protocol_done,
    to_boolean,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum AsyncIteratorPending {
    IteratorResult,
    SyncIteratorResult { done: bool },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum AsyncIteratorClosePending {
    AsyncResult,
    SyncValue,
}

/// Persistent state for one `for await...of` iterator.
#[derive(Debug)]
pub(in crate::runtime) struct AsyncIteratorContinuation {
    source: IteratorSource,
    await_yielded_values: bool,
    pending: Option<AsyncIteratorPending>,
    close_pending: Option<AsyncIteratorClosePending>,
    closing: Option<Completion>,
}

impl AsyncIteratorContinuation {
    pub(in crate::runtime) const fn new(
        source: IteratorSource,
        await_yielded_values: bool,
    ) -> Self {
        Self {
            source,
            await_yielded_values,
            pending: None,
            close_pending: None,
            closing: None,
        }
    }

    pub(in crate::runtime) fn root_values(&self) -> impl Iterator<Item = &Value> {
        self.source
            .root_values()
            .chain(self.closing.as_ref().and_then(completion_value))
    }

    pub(in crate::runtime) const fn source_mut(&mut self) -> &mut IteratorSource {
        &mut self.source
    }
}

pub(in crate::runtime) enum AsyncIteratorStep {
    Await(PromiseId),
    Value(Value),
    Done,
    Abrupt(Completion),
}

pub(in crate::runtime) enum AsyncIteratorCloseStep {
    Await(PromiseId),
    Complete(Completion),
}

impl Context {
    pub(in crate::runtime) fn async_iterator_step(
        &mut self,
        continuation: &mut AsyncIteratorContinuation,
        resume: Option<Completion>,
    ) -> Result<AsyncIteratorStep> {
        if let Some(pending) = continuation.pending.take() {
            return self.resume_async_iterator_step(continuation, pending, resume);
        }
        if resume.is_some() {
            return Err(Error::runtime(
                "async iterator received an unexpected resume completion",
            ));
        }
        match &mut continuation.source {
            source @ (IteratorSource::ArrayIndex { .. }
            | IteratorSource::Utf16CodePoints { .. }) => match self.iterator_step(source)? {
                IteratorStep::Value(value) => {
                    self.await_sync_iterator_result(continuation, value, false)
                }
                IteratorStep::Done => {
                    self.await_sync_iterator_result(continuation, Value::Undefined, true)
                }
                IteratorStep::Abrupt(completion) => Ok(AsyncIteratorStep::Abrupt(completion)),
            },
            IteratorSource::Protocol {
                iterator,
                next,
                done,
            } => {
                if *done {
                    return Ok(AsyncIteratorStep::Done);
                }
                let iterator = iterator.clone();
                let next = next.clone();
                let result = match self.call(&next, &[], iterator)? {
                    Completion::Normal(result) => result,
                    Completion::Throw(value) => {
                        set_protocol_done(&mut continuation.source);
                        return Ok(AsyncIteratorStep::Abrupt(Completion::Throw(value)));
                    }
                    completion => {
                        return completion.into_result().map(AsyncIteratorStep::Value);
                    }
                };
                if continuation.await_yielded_values {
                    self.consume_sync_iterator_result(continuation, &result)
                } else {
                    self.await_async_iterator_result(continuation, &result)
                }
            }
        }
    }

    fn resume_async_iterator_step(
        &mut self,
        continuation: &mut AsyncIteratorContinuation,
        pending: AsyncIteratorPending,
        resume: Option<Completion>,
    ) -> Result<AsyncIteratorStep> {
        match (pending, resume) {
            (AsyncIteratorPending::IteratorResult, Some(Completion::Normal(result))) => {
                self.consume_async_iterator_result(continuation, &result)
            }
            (
                AsyncIteratorPending::SyncIteratorResult { done: true },
                Some(Completion::Normal(_)),
            ) => {
                set_protocol_done(&mut continuation.source);
                Ok(AsyncIteratorStep::Done)
            }
            (
                AsyncIteratorPending::SyncIteratorResult { done: false },
                Some(Completion::Normal(value)),
            ) => Ok(AsyncIteratorStep::Value(value)),
            (_, Some(Completion::Throw(value))) => {
                set_protocol_done(&mut continuation.source);
                Ok(AsyncIteratorStep::Abrupt(Completion::Throw(value)))
            }
            (_, Some(completion)) => Err(Error::runtime(format!(
                "invalid async iterator resume completion {completion:?}"
            ))),
            (_, None) => Err(Error::runtime(
                "async iterator resumed without a completion",
            )),
        }
    }

    fn await_async_iterator_result(
        &mut self,
        continuation: &mut AsyncIteratorContinuation,
        result: &Value,
    ) -> Result<AsyncIteratorStep> {
        continuation.pending = Some(AsyncIteratorPending::IteratorResult);
        let Completion::Suspended(awaited) = self.eval_bytecode_await(result.clone())? else {
            return Err(Error::runtime(
                "async iterator result did not await a Promise",
            ));
        };
        Ok(AsyncIteratorStep::Await(awaited))
    }

    fn consume_async_iterator_result(
        &mut self,
        continuation: &mut AsyncIteratorContinuation,
        result: &Value,
    ) -> Result<AsyncIteratorStep> {
        if self.semantic_object_ref(result)?.is_none() {
            return Err(Error::type_error(format!(
                "iterator result '{result}' is not an object"
            )));
        }
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(result))?;
        if to_boolean(&self.get_named(result, ITERATOR_RESULT_DONE_PROPERTY)?) {
            set_protocol_done(&mut continuation.source);
            return Ok(AsyncIteratorStep::Done);
        }
        self.get_named(result, ITERATOR_RESULT_VALUE_PROPERTY)
            .map(AsyncIteratorStep::Value)
    }

    fn consume_sync_iterator_result(
        &mut self,
        continuation: &mut AsyncIteratorContinuation,
        result: &Value,
    ) -> Result<AsyncIteratorStep> {
        if self.semantic_object_ref(result)?.is_none() {
            return Err(Error::type_error(format!(
                "iterator result '{result}' is not an object"
            )));
        }
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(result))?;
        let done = to_boolean(&self.get_named(result, ITERATOR_RESULT_DONE_PROPERTY)?);
        let value = self.get_named(result, ITERATOR_RESULT_VALUE_PROPERTY)?;
        self.await_sync_iterator_result(continuation, value, done)
    }

    fn await_sync_iterator_result(
        &mut self,
        continuation: &mut AsyncIteratorContinuation,
        value: Value,
        done: bool,
    ) -> Result<AsyncIteratorStep> {
        let wrapper = self.create_async_from_sync_value_wrapper(value)?;
        continuation.pending = Some(AsyncIteratorPending::SyncIteratorResult { done });
        let Completion::Suspended(awaited) = self.eval_bytecode_await(wrapper)? else {
            return Err(Error::runtime(
                "async-from-sync iterator result did not await a Promise",
            ));
        };
        Ok(AsyncIteratorStep::Await(awaited))
    }

    fn create_async_from_sync_value_wrapper(&mut self, value: Value) -> Result<Value> {
        let (wrapper, object) = self.create_pending_promise()?;
        match self.promise_resolve_for_await(value) {
            Ok(value_promise) => {
                self.add_promise_reaction(
                    value_promise,
                    PromiseReaction::new(wrapper, None, None),
                )?;
            }
            Err(error) => {
                let Some(reason) = runtime_exception_value(self, &error)? else {
                    return Err(error);
                };
                self.reject_promise(wrapper, reason)?;
            }
        }
        Ok(object)
    }

    pub(in crate::runtime) fn async_iterator_close(
        &mut self,
        continuation: &mut AsyncIteratorContinuation,
        completion: Option<Completion>,
        resume: Option<Completion>,
    ) -> Result<AsyncIteratorCloseStep> {
        if let Some(pending) = continuation.close_pending.take() {
            return self.resume_async_iterator_close(continuation, pending, resume);
        }
        if resume.is_some() || continuation.closing.is_some() {
            return Err(Error::runtime("async iterator close state is inconsistent"));
        }
        continuation.closing = Some(
            completion.ok_or_else(|| Error::runtime("async iterator close has no completion"))?,
        );
        let original_is_throw = continuation
            .closing
            .as_ref()
            .is_some_and(|completion| matches!(completion, Completion::Throw(_)));
        let Some(iterator) = protocol_iterator_to_close(&mut continuation.source) else {
            return Self::complete_async_iterator_close(continuation);
        };
        let return_method = match self.get_named_method(&iterator, ITERATOR_RETURN_PROPERTY) {
            Ok(method) => method,
            Err(error) if original_is_throw && !is_resource_limit(&error) => {
                return Self::complete_async_iterator_close(continuation);
            }
            Err(error) => return Err(error),
        };
        let Some(return_method) = return_method else {
            return Self::complete_async_iterator_close(continuation);
        };
        let result = match self.call(&return_method, &[], iterator) {
            Ok(Completion::Normal(result)) => result,
            Ok(Completion::Throw(_)) if original_is_throw => {
                return Self::complete_async_iterator_close(continuation);
            }
            Ok(Completion::Throw(value)) => {
                continuation.closing = Some(Completion::Throw(value));
                return Self::complete_async_iterator_close(continuation);
            }
            Ok(completion) => {
                return completion
                    .into_result()
                    .map(|value| AsyncIteratorCloseStep::Complete(Completion::Normal(value)));
            }
            Err(error) if original_is_throw && !is_resource_limit(&error) => {
                return Self::complete_async_iterator_close(continuation);
            }
            Err(error) => return Err(error),
        };
        let (awaited, pending) = if continuation.await_yielded_values {
            (
                self.create_async_from_sync_close_wrapper(&result)?,
                AsyncIteratorClosePending::SyncValue,
            )
        } else {
            (result, AsyncIteratorClosePending::AsyncResult)
        };
        continuation.close_pending = Some(pending);
        let Completion::Suspended(awaited) = self.eval_bytecode_await(awaited)? else {
            return Err(Error::runtime(
                "async iterator close did not await a Promise",
            ));
        };
        Ok(AsyncIteratorCloseStep::Await(awaited))
    }

    fn resume_async_iterator_close(
        &self,
        continuation: &mut AsyncIteratorContinuation,
        pending: AsyncIteratorClosePending,
        resume: Option<Completion>,
    ) -> Result<AsyncIteratorCloseStep> {
        let original_is_throw = continuation
            .closing
            .as_ref()
            .is_some_and(|completion| matches!(completion, Completion::Throw(_)));
        match resume {
            Some(Completion::Normal(result)) => {
                if pending == AsyncIteratorClosePending::AsyncResult
                    && self.semantic_object_ref(&result)?.is_none()
                    && !original_is_throw
                {
                    return Err(Error::type_error(
                        "iterator return method must return an object",
                    ));
                }
                Self::complete_async_iterator_close(continuation)
            }
            Some(Completion::Throw(_)) if original_is_throw => {
                Self::complete_async_iterator_close(continuation)
            }
            Some(Completion::Throw(value)) => {
                continuation.closing = Some(Completion::Throw(value));
                Self::complete_async_iterator_close(continuation)
            }
            Some(completion) => completion
                .into_result()
                .map(|value| AsyncIteratorCloseStep::Complete(Completion::Normal(value))),
            None => Err(Error::runtime(
                "async iterator close resumed without a completion",
            )),
        }
    }

    fn create_async_from_sync_close_wrapper(&mut self, result: &Value) -> Result<Value> {
        let (wrapper, object) = self.create_pending_promise()?;
        let value = (|| {
            if self.semantic_object_ref(result)?.is_none() {
                return Err(Error::type_error(
                    "iterator return method must return an object",
                ));
            }
            let _result_scope =
                self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(result))?;
            let _done = self.get_named(result, ITERATOR_RESULT_DONE_PROPERTY)?;
            self.get_named(result, ITERATOR_RESULT_VALUE_PROPERTY)
        })();
        match value.and_then(|value| self.promise_resolve_for_await(value)) {
            Ok(value_promise) => {
                self.add_promise_reaction(
                    value_promise,
                    PromiseReaction::new(wrapper, None, None),
                )?;
            }
            Err(error) => {
                let Some(reason) = runtime_exception_value(self, &error)? else {
                    return Err(error);
                };
                self.reject_promise(wrapper, reason)?;
            }
        }
        Ok(object)
    }

    fn complete_async_iterator_close(
        continuation: &mut AsyncIteratorContinuation,
    ) -> Result<AsyncIteratorCloseStep> {
        continuation
            .closing
            .take()
            .map(AsyncIteratorCloseStep::Complete)
            .ok_or_else(|| Error::runtime("async iterator close completion disappeared"))
    }
}

const fn completion_value(completion: &Completion) -> Option<&Value> {
    match completion {
        Completion::Normal(value)
        | Completion::Throw(value)
        | Completion::Return(value)
        | Completion::ReturnDirect(value)
        | Completion::Break { value, .. }
        | Completion::Yielded(value)
        | Completion::YieldedIteratorResult(value) => Some(value),
        Completion::Continue(_) | Completion::Suspended(_) | Completion::GeneratorStart => None,
    }
}
