use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        control::{Completion, DelegatedYield},
        object::PropertyKey,
        property::DynamicPropertyKey,
        roots::VmRootKind,
        transient_roots::TransientRootScope,
    },
    value::Value,
};

use super::to_boolean;

mod async_delegate;
mod async_iteration;

pub(in crate::runtime) use async_iteration::{
    AsyncIteratorCloseStep, AsyncIteratorContinuation, AsyncIteratorStep,
};

const ITERATOR_SYMBOL_DISPLAY_NAME: &str = "Symbol(Symbol.iterator)";
const ITERATOR_NEXT_PROPERTY: &str = "next";
const ITERATOR_RETURN_PROPERTY: &str = "return";
const ITERATOR_RESULT_DONE_PROPERTY: &str = "done";
const ITERATOR_RESULT_VALUE_PROPERTY: &str = "value";

/// One iterator source. Direct array and string variants are guarded
/// implementations of the built-in iterators that are not installed yet.
#[derive(Debug)]
pub(in crate::runtime) enum IteratorSource {
    /// Live array index iteration, matching the built-in Array iterator's
    /// observable length reads and element access.
    ArrayIndex { array: Value, index: usize },
    /// Code-point iteration over an immutable string snapshot.
    Utf16CodePoints {
        units: std::iter::Peekable<std::vec::IntoIter<u16>>,
    },
    /// ECMAScript iterator record with the `next` method cached at acquisition.
    Protocol {
        iterator: Value,
        next: Value,
        done: bool,
    },
}

impl IteratorSource {
    pub(in crate::runtime) const fn root_value_slots(&self) -> [Option<&Value>; 2] {
        match self {
            Self::ArrayIndex { array, .. } => [Some(array), None],
            Self::Protocol { iterator, next, .. } => [Some(iterator), Some(next)],
            Self::Utf16CodePoints { .. } => [None, None],
        }
    }

    pub(in crate::runtime) fn root_values(&self) -> impl Iterator<Item = &Value> {
        self.root_value_slots().into_iter().flatten()
    }

    pub(in crate::runtime) const fn mark_done(&mut self) {
        set_protocol_done(self);
    }
}

/// Outcome of the shared `IteratorStep` and `IteratorValue` sequence.
pub(in crate::runtime) enum IteratorStep {
    Value(Value),
    Done,
    /// An abrupt completion thrown directly by the iterator's `next` method.
    Abrupt(Completion),
}

/// Result of `IteratorStep` before `IteratorValue` observes the `value` field.
pub(in crate::runtime) enum IteratorResultStep {
    Result(Value),
    Done,
    Abrupt(Completion),
}

/// Iterator state selected by an ordinary or asynchronous for-of head.
#[derive(Debug)]
pub(in crate::runtime) enum ForOfIterator {
    Synchronous(IteratorSource),
    Asynchronous(AsyncIteratorContinuation),
}

impl ForOfIterator {
    pub(in crate::runtime) fn root_values(&self) -> impl Iterator<Item = &Value> {
        match self {
            Self::Synchronous(source) => source.root_values().collect::<Vec<_>>(),
            Self::Asynchronous(continuation) => continuation.root_values().collect::<Vec<_>>(),
        }
        .into_iter()
    }

    pub(in crate::runtime) const fn source_mut(&mut self) -> &mut IteratorSource {
        match self {
            Self::Synchronous(source) => source,
            Self::Asynchronous(continuation) => continuation.source_mut(),
        }
    }
}

/// Persistent iterator state owned by a suspended `yield*` instruction.
#[derive(Debug)]
pub(in crate::runtime) struct YieldDelegateContinuation {
    source: IteratorSource,
    asynchronous: bool,
    await_yielded_values: bool,
    pending: Option<YieldDelegateDone>,
}

impl YieldDelegateContinuation {
    pub(in crate::runtime) const fn new(
        source: IteratorSource,
        asynchronous: bool,
        await_yielded_values: bool,
    ) -> Self {
        Self {
            source,
            asynchronous,
            await_yielded_values,
            pending: None,
        }
    }

    pub(in crate::runtime) fn root_values(&self) -> impl Iterator<Item = &Value> {
        self.source.root_values()
    }
}

/// One externally observable step of the `yield*` delegation loop.
pub(in crate::runtime) enum YieldDelegateStep {
    Await(crate::runtime::promise::PromiseId),
    Yielded(Value),
    DelegatedYield(DelegatedYield),
    Complete(Value),
    Return(Value),
    Abrupt(Completion),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum YieldDelegateDone {
    Normal,
    Return,
    ResumeReturn,
    CloseThrow,
}

impl Context {
    /// ECMAScript `GetIterator` with guarded direct implementations for Array
    /// and String while their built-in protocol methods remain uninstalled.
    pub(in crate::runtime) fn get_iterator(&mut self, iterable: &Value) -> Result<IteratorSource> {
        match iterable {
            Value::String(_) => {
                if let Some(method) = self.iterator_method(iterable)? {
                    return self.get_iterator_from_method(iterable, &method);
                }
                let units = iterable
                    .string_units()
                    .ok_or_else(|| Error::runtime("string iterator lost its source"))?;
                Ok(utf16_source(units.into_owned().into_iter()))
            }
            Value::Object(id) => {
                if let Some(method) = self.iterator_method(iterable)? {
                    return self.get_iterator_from_method_with_array_fast_path(iterable, &method);
                }
                if self.objects.array_len_if_array(*id)?.is_some() {
                    return Err(not_iterable_error(iterable));
                }
                if let Some(text) = self.string_object_utf16_primitive_value(*id)? {
                    return Ok(utf16_source(text.iter().copied()));
                }
                Err(not_iterable_error(iterable))
            }
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::Symbol(_) => {
                let Some(method) = self.iterator_method(iterable)? else {
                    return Err(not_iterable_error(iterable));
                };
                self.get_iterator_from_method(iterable, &method)
            }
            Value::Undefined | Value::Null => Err(not_iterable_error(iterable)),
        }
    }

    pub(in crate::runtime) fn get_async_iterator(
        &mut self,
        iterable: &Value,
    ) -> Result<(IteratorSource, bool)> {
        if let Some(method) = self.async_iterator_method(iterable)? {
            return self
                .get_iterator_from_method(iterable, &method)
                .map(|source| (source, false));
        }
        if let Some(method) = self.iterator_method(iterable)? {
            return self
                .get_iterator_from_method(iterable, &method)
                .map(|source| (source, true));
        }
        self.get_iterator(iterable).map(|source| (source, true))
    }

    /// ECMAScript `GetIteratorFromMethod`, shared by ordinary iterable
    /// acquisition and algorithms that already captured a protocol method.
    pub(in crate::runtime) fn get_iterator_from_method(
        &mut self,
        iterable: &Value,
        method: &Value,
    ) -> Result<IteratorSource> {
        let iterator = self.call_value(method, &[], iterable.clone())?;
        if self.semantic_object_ref(&iterator)?.is_none() {
            return Err(Error::type_error(format!(
                "iterator '{iterator}' is not an object"
            )));
        }
        let _root_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&iterator))?;
        let next = self.get_named(&iterator, ITERATOR_NEXT_PROPERTY)?;
        Ok(IteratorSource::Protocol {
            iterator,
            next,
            done: false,
        })
    }

    /// Guarded direct Array implementation for synchronous consumers that
    /// already captured the default iterator method exactly once.
    pub(in crate::runtime) fn get_iterator_from_method_with_array_fast_path(
        &mut self,
        iterable: &Value,
        method: &Value,
    ) -> Result<IteratorSource> {
        if let Value::Object(id) = iterable
            && self.objects.array_len_if_array(*id)?.is_some()
            && self.is_default_array_iterator_method(method)?
            && self.default_array_iterator_next_is_intact()?
        {
            return Ok(IteratorSource::ArrayIndex {
                array: iterable.clone(),
                index: 0,
            });
        }
        self.get_iterator_from_method(iterable, method)
    }

    /// ECMAScript `IteratorStep` followed by `IteratorValue` when the result
    /// is not complete.
    pub(in crate::runtime) fn iterator_step(
        &mut self,
        source: &mut IteratorSource,
    ) -> Result<IteratorStep> {
        let _root_scope = self.iterator_root_scope(source)?;
        match source {
            IteratorSource::ArrayIndex { array, index } => {
                let Value::Object(id) = array else {
                    return Err(Error::runtime("array iterator source is not an object"));
                };
                let Some(len) = self.objects.array_len_if_array(*id)? else {
                    return Ok(IteratorStep::Done);
                };
                if *index >= len {
                    return Ok(IteratorStep::Done);
                }
                let key = index.to_string();
                *index = index
                    .checked_add(1)
                    .ok_or_else(|| Error::runtime("array iterator index overflowed"))?;
                let array = array.clone();
                Ok(IteratorStep::Value(self.get_named(&array, &key)?))
            }
            IteratorSource::Utf16CodePoints { units } => match units.next() {
                Some(first) => {
                    let mut code_point = vec![first];
                    if (0xD800..=0xDBFF).contains(&first)
                        && units
                            .peek()
                            .is_some_and(|second| (0xDC00..=0xDFFF).contains(second))
                    {
                        let Some(second) = units.next() else {
                            return Err(Error::runtime("UTF-16 iterator lookahead disappeared"));
                        };
                        code_point.push(second);
                    }
                    Ok(IteratorStep::Value(
                        self.heap_utf16_string_value(&code_point)?,
                    ))
                }
                None => Ok(IteratorStep::Done),
            },
            IteratorSource::Protocol {
                iterator: _,
                next: _,
                done: _,
            } => match self.iterator_step_result(source)? {
                IteratorResultStep::Result(result) => Ok(IteratorStep::Value(
                    self.get_named(&result, ITERATOR_RESULT_VALUE_PROPERTY)?,
                )),
                IteratorResultStep::Done => Ok(IteratorStep::Done),
                IteratorResultStep::Abrupt(completion) => Ok(IteratorStep::Abrupt(completion)),
            },
        }
    }

    /// ECMAScript `IteratorStep` without eagerly reading `value` from a
    /// non-complete result object. Joint iteration strict-mode checks use this
    /// to test the remaining iterators without observing poisoned values.
    pub(in crate::runtime) fn iterator_step_result(
        &mut self,
        source: &mut IteratorSource,
    ) -> Result<IteratorResultStep> {
        let _root_scope = self.iterator_root_scope(source)?;
        let IteratorSource::Protocol {
            iterator,
            next,
            done,
        } = source
        else {
            return Err(Error::runtime(
                "raw iterator result stepping requires a protocol iterator",
            ));
        };
        if *done {
            return Ok(IteratorResultStep::Done);
        }
        let next = next.clone();
        let iterator = iterator.clone();
        let result = match self.call(&next, &[], iterator)? {
            Completion::Normal(value) => value,
            Completion::Throw(value) => {
                set_protocol_done(source);
                return Ok(IteratorResultStep::Abrupt(Completion::Throw(value)));
            }
            completion => {
                return completion.into_result().map(IteratorResultStep::Result);
            }
        };
        if self.semantic_object_ref(&result)?.is_none() {
            return Err(Error::type_error(format!(
                "iterator result '{result}' is not an object"
            )));
        }
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        if to_boolean(&self.get_named(&result, ITERATOR_RESULT_DONE_PROPERTY)?) {
            set_protocol_done(source);
            return Ok(IteratorResultStep::Done);
        }
        Ok(IteratorResultStep::Result(result))
    }

    /// Resumes the ECMAScript `yield*` delegation loop.
    pub(in crate::runtime) fn yield_delegate_step(
        &mut self,
        continuation: &mut YieldDelegateContinuation,
        resume: Option<Completion>,
    ) -> Result<YieldDelegateStep> {
        let _source_scope = self.iterator_root_scope(&continuation.source)?;
        let _resume_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            resume.as_ref().and_then(completion_value),
        )?;
        if continuation.asynchronous {
            return self.async_yield_delegate_step(continuation, resume);
        }
        match resume {
            None => self.yield_delegate_next(&mut continuation.source, &Value::Undefined),
            Some(Completion::Normal(value)) => {
                self.yield_delegate_next(&mut continuation.source, &value)
            }
            Some(Completion::Return(value) | Completion::ReturnDirect(value)) => {
                self.yield_delegate_return(&mut continuation.source, &value)
            }
            Some(Completion::Throw(value)) => {
                self.yield_delegate_throw(&mut continuation.source, &value)
            }
            Some(
                Completion::TailCall(_)
                | Completion::Break { .. }
                | Completion::Continue { .. }
                | Completion::Suspend(_),
            ) => Err(Error::runtime("invalid yield delegation resume completion")),
        }
    }

    fn yield_delegate_next(
        &mut self,
        source: &mut IteratorSource,
        value: &Value,
    ) -> Result<YieldDelegateStep> {
        match source {
            IteratorSource::ArrayIndex { .. } | IteratorSource::Utf16CodePoints { .. } => {
                match self.iterator_step(source)? {
                    IteratorStep::Value(value) => Ok(YieldDelegateStep::Yielded(value)),
                    IteratorStep::Done => Ok(YieldDelegateStep::Complete(Value::Undefined)),
                    IteratorStep::Abrupt(completion) => Ok(YieldDelegateStep::Abrupt(completion)),
                }
            }
            IteratorSource::Protocol {
                iterator,
                next,
                done,
            } => {
                if *done {
                    return Ok(YieldDelegateStep::Complete(Value::Undefined));
                }
                let iterator = iterator.clone();
                let next = next.clone();
                let result = match self.call(&next, std::slice::from_ref(value), iterator)? {
                    Completion::Normal(result) => result,
                    Completion::Throw(value) => {
                        set_protocol_done(source);
                        return Ok(YieldDelegateStep::Abrupt(Completion::Throw(value)));
                    }
                    completion => return completion.into_result().map(YieldDelegateStep::Complete),
                };
                self.yield_delegate_result(source, &result, YieldDelegateDone::Normal)
            }
        }
    }

    fn yield_delegate_return(
        &mut self,
        source: &mut IteratorSource,
        value: &Value,
    ) -> Result<YieldDelegateStep> {
        let IteratorSource::Protocol { iterator, done, .. } = source else {
            return Ok(YieldDelegateStep::Return(value.clone()));
        };
        if *done {
            return Ok(YieldDelegateStep::Return(value.clone()));
        }
        let iterator = iterator.clone();
        let Some(return_method) = self.get_named_method(&iterator, ITERATOR_RETURN_PROPERTY)?
        else {
            set_protocol_done(source);
            return Ok(YieldDelegateStep::Return(value.clone()));
        };
        let result = match self.call(&return_method, std::slice::from_ref(value), iterator)? {
            Completion::Normal(result) => result,
            Completion::Throw(value) => {
                set_protocol_done(source);
                return Ok(YieldDelegateStep::Abrupt(Completion::Throw(value)));
            }
            completion => return completion.into_result().map(YieldDelegateStep::Complete),
        };
        self.yield_delegate_result(source, &result, YieldDelegateDone::Return)
    }

    fn yield_delegate_throw(
        &mut self,
        source: &mut IteratorSource,
        value: &Value,
    ) -> Result<YieldDelegateStep> {
        let IteratorSource::Protocol { iterator, done, .. } = source else {
            return Err(Error::type_error("delegated iterator has no throw method"));
        };
        if *done {
            return Err(Error::type_error("delegated iterator has no throw method"));
        }
        let iterator = iterator.clone();
        let Some(throw_method) = self.get_named_method(&iterator, "throw")? else {
            return match self.iterator_close(source, Completion::Normal(Value::Undefined))? {
                Completion::Normal(_) => {
                    Err(Error::type_error("delegated iterator has no throw method"))
                }
                abrupt @ Completion::Throw(_) => Ok(YieldDelegateStep::Abrupt(abrupt)),
                completion => completion.into_result().map(YieldDelegateStep::Complete),
            };
        };
        let result = match self.call(&throw_method, std::slice::from_ref(value), iterator)? {
            Completion::Normal(result) => result,
            Completion::Throw(value) => {
                set_protocol_done(source);
                return Ok(YieldDelegateStep::Abrupt(Completion::Throw(value)));
            }
            completion => return completion.into_result().map(YieldDelegateStep::Complete),
        };
        self.yield_delegate_result(source, &result, YieldDelegateDone::Normal)
    }

    fn yield_delegate_result(
        &mut self,
        source: &mut IteratorSource,
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
        if !done {
            return Ok(YieldDelegateStep::DelegatedYield(
                DelegatedYield::iterator_result(result.clone()),
            ));
        }
        set_protocol_done(source);
        let value = self.get_named(result, ITERATOR_RESULT_VALUE_PROPERTY)?;
        Ok(match done_kind {
            YieldDelegateDone::Normal => YieldDelegateStep::Complete(value),
            YieldDelegateDone::Return => YieldDelegateStep::Return(value),
            YieldDelegateDone::ResumeReturn => {
                return Err(Error::runtime(
                    "async return resumption reached synchronous iterator result handling",
                ));
            }
            YieldDelegateDone::CloseThrow => {
                return Err(Error::runtime(
                    "async iterator close reached ordinary delegation result handling",
                ));
            }
        })
    }

    /// ECMAScript `IteratorClose`, including the rule that an original throw
    /// completion wins over failures while looking up or calling `return`.
    pub(in crate::runtime) fn iterator_close(
        &mut self,
        source: &mut IteratorSource,
        completion: Completion,
    ) -> Result<Completion> {
        let _source_scope = self.iterator_root_scope(source)?;
        let _completion_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            completion_value(&completion),
        )?;
        let Some(iterator) = protocol_iterator_to_close(source) else {
            return Ok(completion);
        };
        let original_is_throw = matches!(completion, Completion::Throw(_));
        let return_method = match self.get_named_method(&iterator, ITERATOR_RETURN_PROPERTY) {
            Ok(method) => method,
            Err(error) if original_is_throw && !is_resource_limit(&error) => {
                return Ok(completion);
            }
            Err(error) => return Err(error),
        };
        let Some(return_method) = return_method else {
            return Ok(completion);
        };
        let close_completion = match self.call(&return_method, &[], iterator) {
            Ok(close_completion) => close_completion,
            Err(error) if original_is_throw && !is_resource_limit(&error) => {
                return Ok(completion);
            }
            Err(error) => return Err(error),
        };
        if original_is_throw {
            return Ok(completion);
        }
        match close_completion {
            Completion::Normal(value) if self.semantic_object_ref(&value)?.is_some() => {
                Ok(completion)
            }
            Completion::Normal(_) => Err(Error::type_error(
                "iterator return method must return an object",
            )),
            abrupt @ Completion::Throw(_) => Ok(abrupt),
            completion @ (Completion::Return(_)
            | Completion::ReturnDirect(_)
            | Completion::Break { .. }
            | Completion::Continue { .. }) => completion.into_result().map(Completion::Normal),
            completion @ Completion::Suspend(_) => Ok(completion),
            Completion::TailCall(_) => {
                Err(Error::runtime("tail call escaped iterator return method"))
            }
        }
    }

    /// Closes after an error already represented outside `Completion`.
    /// JavaScript throw precedence requires every close failure to be ignored.
    pub(in crate::runtime) fn iterator_close_on_error(
        &mut self,
        source: &mut IteratorSource,
        error: Error,
    ) -> Error {
        let _source_scope = match self.iterator_root_scope(source) {
            Ok(scope) => scope,
            Err(error) => return error,
        };
        let _error_scope = match self
            .transient_root_scope(VmRootKind::TransientTemporary, error.javascript_value())
        {
            Ok(scope) => scope,
            Err(error) => return error,
        };
        if is_resource_limit(&error) {
            return error;
        }
        let Some(iterator) = protocol_iterator_to_close(source) else {
            return error;
        };
        let return_method = match self.get_named_method(&iterator, ITERATOR_RETURN_PROPERTY) {
            Ok(Some(return_method)) => return_method,
            Ok(None) => return error,
            Err(close_error) if is_resource_limit(&close_error) => return close_error,
            Err(_close_error) => return error,
        };
        match self.call(&return_method, &[], iterator) {
            Err(close_error) if is_resource_limit(&close_error) => close_error,
            Ok(_) | Err(_) => error,
        }
    }

    pub(in crate::runtime) fn iterator_method(
        &mut self,
        iterable: &Value,
    ) -> Result<Option<Value>> {
        let Some(symbol) = self.iterator_symbol() else {
            return Ok(None);
        };
        let key = DynamicPropertyKey::new(
            ITERATOR_SYMBOL_DISPLAY_NAME.to_owned(),
            Some(PropertyKey::symbol(symbol)),
        );
        self.get_method(iterable, key.lookup())
    }

    pub(in crate::runtime) fn async_iterator_method(
        &mut self,
        iterable: &Value,
    ) -> Result<Option<Value>> {
        let constructor = self.symbol_constructor_value()?;
        let symbol = self.get_named(&constructor, "asyncIterator")?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("Symbol.asyncIterator is not initialized"));
        };
        let key = DynamicPropertyKey::new(
            "[Symbol.asyncIterator]".to_owned(),
            Some(PropertyKey::symbol(symbol.id())),
        );
        self.get_method(iterable, key.lookup())
    }

    fn is_default_array_iterator_method(&self, method: &Value) -> Result<bool> {
        let Value::NativeFunction(id) = method else {
            return Ok(false);
        };
        Ok(self.native_function(*id)?.kind()
            == crate::runtime::native::NativeFunctionKind::ArrayValues)
    }

    fn iterator_root_scope(&self, source: &IteratorSource) -> Result<TransientRootScope> {
        match source {
            IteratorSource::ArrayIndex { array, .. } => {
                self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(array))
            }
            IteratorSource::Protocol { iterator, next, .. } => {
                self.transient_root_scope(VmRootKind::TransientTemporary, [iterator, next])
            }
            IteratorSource::Utf16CodePoints { .. } => {
                self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::empty())
            }
        }
    }
}

fn utf16_source(units: impl Iterator<Item = u16>) -> IteratorSource {
    IteratorSource::Utf16CodePoints {
        units: units.collect::<Vec<_>>().into_iter().peekable(),
    }
}

fn not_iterable_error(value: &Value) -> Error {
    Error::type_error(format!("'{value}' is not iterable"))
}

const fn set_protocol_done(source: &mut IteratorSource) {
    if let IteratorSource::Protocol { done, .. } = source {
        *done = true;
    }
}

fn protocol_iterator_to_close(source: &mut IteratorSource) -> Option<Value> {
    let IteratorSource::Protocol { iterator, done, .. } = source else {
        return None;
    };
    if *done {
        return None;
    }
    *done = true;
    Some(iterator.clone())
}

fn protocol_iterator(source: &IteratorSource) -> Result<Value> {
    let IteratorSource::Protocol { iterator, .. } = source else {
        return Err(Error::runtime("iterator source is not protocol-based"));
    };
    Ok(iterator.clone())
}

const fn is_resource_limit(error: &Error) -> bool {
    matches!(error, Error::ResourceLimit { .. })
}

const fn completion_value(completion: &Completion) -> Option<&Value> {
    match completion {
        Completion::Normal(value)
        | Completion::Throw(value)
        | Completion::Return(value)
        | Completion::ReturnDirect(value)
        | Completion::Break { value, .. }
        | Completion::Continue { value, .. } => Some(value),
        Completion::TailCall(request) => Some(request.callee()),
        Completion::Suspend(suspension) => suspension.root_value(),
    }
}
