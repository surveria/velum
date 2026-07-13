use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        control::{Completion, DelegatedYield, Suspension},
        roots::VmRootKind,
    },
    value::Value,
};

use super::{
    ITERATOR_RESULT_DONE_PROPERTY, ITERATOR_RESULT_VALUE_PROPERTY, ITERATOR_RETURN_PROPERTY,
    YieldDelegateContinuation, YieldDelegateDone, YieldDelegateStep, protocol_iterator,
    set_protocol_done, to_boolean,
};

impl Context {
    pub(super) fn async_yield_delegate_step(
        &mut self,
        continuation: &mut YieldDelegateContinuation,
        resume: Option<Completion>,
    ) -> Result<YieldDelegateStep> {
        if let Some(done_kind) = continuation.pending.take() {
            return self.resume_async_yield_delegate_pending(continuation, done_kind, resume);
        }

        let (method, value, done_kind) = match resume {
            None => (
                Self::yield_delegate_next_method(&continuation.source)?,
                Value::Undefined,
                YieldDelegateDone::Normal,
            ),
            Some(Completion::Normal(value)) => (
                Self::yield_delegate_next_method(&continuation.source)?,
                value,
                YieldDelegateDone::Normal,
            ),
            Some(Completion::Return(value)) => {
                let Completion::Suspend(Suspension::Await(awaited)) =
                    self.eval_bytecode_await(value)?
                else {
                    return Err(Error::runtime(
                        "async yield delegation return did not await its resumption value",
                    ));
                };
                continuation.pending = Some(YieldDelegateDone::ResumeReturn);
                return Ok(YieldDelegateStep::Await(awaited));
            }
            Some(Completion::ReturnDirect(value)) => {
                return self.async_yield_delegate_return(continuation, value);
            }
            Some(Completion::Throw(value)) => {
                let Some(method) =
                    self.yield_delegate_named_method(&continuation.source, "throw")?
                else {
                    let return_method = self.yield_delegate_named_method(
                        &continuation.source,
                        ITERATOR_RETURN_PROPERTY,
                    )?;
                    if let Some(return_method) = return_method {
                        let iterator = protocol_iterator(&continuation.source)?;
                        let result = match self.call(&return_method, &[], iterator)? {
                            Completion::Normal(result) => result,
                            Completion::Throw(value) => {
                                set_protocol_done(&mut continuation.source);
                                return Ok(YieldDelegateStep::Abrupt(Completion::Throw(value)));
                            }
                            completion => {
                                return completion.into_result().map(YieldDelegateStep::Complete);
                            }
                        };
                        let Completion::Suspend(Suspension::Await(awaited)) =
                            self.eval_bytecode_await(result)?
                        else {
                            return Err(Error::runtime(
                                "async iterator close did not await its result",
                            ));
                        };
                        continuation.pending = Some(YieldDelegateDone::CloseThrow);
                        set_protocol_done(&mut continuation.source);
                        return Ok(YieldDelegateStep::Await(awaited));
                    }
                    set_protocol_done(&mut continuation.source);
                    return Err(Error::type_error("delegated iterator has no throw method"));
                };
                (method, value, YieldDelegateDone::Normal)
            }
            Some(completion) => {
                return Err(Error::runtime(format!(
                    "invalid async yield delegation resume completion {completion:?}"
                )));
            }
        };
        let iterator = protocol_iterator(&continuation.source)?;
        let result = match self.call(&method, &[value], iterator)? {
            Completion::Normal(result) => result,
            Completion::Throw(value) => {
                set_protocol_done(&mut continuation.source);
                return Ok(YieldDelegateStep::Abrupt(Completion::Throw(value)));
            }
            completion => return completion.into_result().map(YieldDelegateStep::Complete),
        };
        let Completion::Suspend(Suspension::Await(awaited)) = self.eval_bytecode_await(result)?
        else {
            return Err(Error::runtime(
                "async yield delegation did not await an iterator result",
            ));
        };
        continuation.pending = Some(done_kind);
        Ok(YieldDelegateStep::Await(awaited))
    }

    fn resume_async_yield_delegate_pending(
        &mut self,
        continuation: &mut YieldDelegateContinuation,
        done_kind: YieldDelegateDone,
        resume: Option<Completion>,
    ) -> Result<YieldDelegateStep> {
        if done_kind == YieldDelegateDone::ResumeReturn {
            return match resume {
                Some(Completion::Normal(value)) => {
                    self.async_yield_delegate_return(continuation, value)
                }
                Some(Completion::Throw(value)) => {
                    Ok(YieldDelegateStep::Abrupt(Completion::Throw(value)))
                }
                Some(completion) => Err(Error::runtime(format!(
                    "invalid async yield return resumption completion {completion:?}"
                ))),
                None => Err(Error::runtime(
                    "async yield return resumption Promise resumed without a completion",
                )),
            };
        }
        if done_kind == YieldDelegateDone::CloseThrow {
            return match resume {
                Some(Completion::Normal(result))
                    if self.semantic_object_ref(&result)?.is_some() =>
                {
                    Err(Error::type_error("delegated iterator has no throw method"))
                }
                Some(Completion::Normal(_)) => Err(Error::type_error(
                    "iterator return method must return an object",
                )),
                Some(Completion::Throw(value)) => {
                    Ok(YieldDelegateStep::Abrupt(Completion::Throw(value)))
                }
                Some(completion) => Err(Error::runtime(format!(
                    "invalid async iterator close completion {completion:?}"
                ))),
                None => Err(Error::runtime(
                    "async iterator close resumed without a completion",
                )),
            };
        }
        match resume {
            Some(Completion::Normal(result)) => {
                self.async_yield_delegate_result(continuation, &result, done_kind)
            }
            Some(Completion::Throw(value)) => {
                set_protocol_done(&mut continuation.source);
                Ok(YieldDelegateStep::Abrupt(Completion::Throw(value)))
            }
            Some(completion) => Err(Error::runtime(format!(
                "invalid async yield delegation completion {completion:?}"
            ))),
            None => Err(Error::runtime(
                "async yield delegation Promise resumed without a completion",
            )),
        }
    }

    fn async_yield_delegate_return(
        &mut self,
        continuation: &mut YieldDelegateContinuation,
        value: Value,
    ) -> Result<YieldDelegateStep> {
        let Some(method) =
            self.yield_delegate_named_method(&continuation.source, ITERATOR_RETURN_PROPERTY)?
        else {
            set_protocol_done(&mut continuation.source);
            return Ok(YieldDelegateStep::Return(value));
        };
        let iterator = protocol_iterator(&continuation.source)?;
        let result = match self.call(&method, &[value], iterator)? {
            Completion::Normal(result) => result,
            Completion::Throw(value) => {
                set_protocol_done(&mut continuation.source);
                return Ok(YieldDelegateStep::Abrupt(Completion::Throw(value)));
            }
            completion => return completion.into_result().map(YieldDelegateStep::Complete),
        };
        let Completion::Suspend(Suspension::Await(awaited)) = self.eval_bytecode_await(result)?
        else {
            return Err(Error::runtime(
                "async yield delegation did not await an iterator return result",
            ));
        };
        continuation.pending = Some(YieldDelegateDone::Return);
        Ok(YieldDelegateStep::Await(awaited))
    }

    fn async_yield_delegate_result(
        &mut self,
        continuation: &mut YieldDelegateContinuation,
        result: &Value,
        done_kind: YieldDelegateDone,
    ) -> Result<YieldDelegateStep> {
        if self.semantic_object_ref(result)?.is_none() {
            return Err(Error::type_error(format!(
                "iterator result '{result}' is not an object"
            )));
        }
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(result))?;
        let done = to_boolean(&self.get_named(result, ITERATOR_RESULT_DONE_PROPERTY)?);
        let value = self.get_named(result, ITERATOR_RESULT_VALUE_PROPERTY)?;
        if !done {
            return Ok(YieldDelegateStep::DelegatedYield(
                DelegatedYield::async_value(value, continuation.await_yielded_values),
            ));
        }
        set_protocol_done(&mut continuation.source);
        Ok(match done_kind {
            YieldDelegateDone::Normal => YieldDelegateStep::Complete(value),
            YieldDelegateDone::Return => YieldDelegateStep::Return(value),
            YieldDelegateDone::ResumeReturn => {
                return Err(Error::runtime(
                    "async return resumption reached iterator result handling",
                ));
            }
            YieldDelegateDone::CloseThrow => {
                return Err(Error::runtime(
                    "async iterator close reached ordinary delegation result handling",
                ));
            }
        })
    }

    fn yield_delegate_next_method(source: &super::IteratorSource) -> Result<Value> {
        let super::IteratorSource::Protocol { next, .. } = source else {
            return Err(Error::runtime(
                "async yield delegation source is not protocol-based",
            ));
        };
        Ok(next.clone())
    }

    fn yield_delegate_named_method(
        &mut self,
        source: &super::IteratorSource,
        name: &str,
    ) -> Result<Option<Value>> {
        let iterator = protocol_iterator(source)?;
        self.get_named_method(&iterator, name)
    }
}
