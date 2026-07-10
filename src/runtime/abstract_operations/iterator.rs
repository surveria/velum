use crate::{
    error::{Error, Result},
    runtime::{Context, control::Completion, object::PropertyKey, property::DynamicPropertyKey},
    value::Value,
};

use super::to_boolean;

const ITERATOR_SYMBOL_DISPLAY_NAME: &str = "Symbol(Symbol.iterator)";
const ITERATOR_NEXT_PROPERTY: &str = "next";
const ITERATOR_RETURN_PROPERTY: &str = "return";
const ITERATOR_RESULT_DONE_PROPERTY: &str = "done";
const ITERATOR_RESULT_VALUE_PROPERTY: &str = "value";

/// One iterator source. Direct array and string variants are guarded
/// implementations of the built-in iterators that are not installed yet.
pub(in crate::runtime) enum IteratorSource {
    /// Live array index iteration, matching the built-in Array iterator's
    /// observable length reads and element access.
    ArrayIndex { array: Value, index: usize },
    /// Code-point iteration over an immutable string snapshot.
    Chars { chars: std::vec::IntoIter<char> },
    /// ECMAScript iterator record with the `next` method cached at acquisition.
    Protocol {
        iterator: Value,
        next: Value,
        done: bool,
    },
}

/// Outcome of the shared `IteratorStep` and `IteratorValue` sequence.
pub(in crate::runtime) enum IteratorStep {
    Value(Value),
    Done,
    /// An abrupt completion thrown directly by the iterator's `next` method.
    Abrupt(Completion),
}

impl Context {
    /// ECMAScript `GetIterator` with guarded direct implementations for Array
    /// and String while their built-in protocol methods remain uninstalled.
    pub(in crate::runtime) fn get_iterator(&mut self, iterable: Value) -> Result<IteratorSource> {
        match &iterable {
            Value::String(text) => {
                if let Some(method) = self.iterator_method(&iterable)? {
                    return self.get_iterator_from_method(&iterable, &method);
                }
                Ok(chars_source(text))
            }
            Value::HeapString(text) => {
                if let Some(method) = self.iterator_method(&iterable)? {
                    return self.get_iterator_from_method(&iterable, &method);
                }
                Ok(chars_source(text.as_str()))
            }
            Value::Object(id) => {
                if let Some(method) = self.iterator_method(&iterable)? {
                    return self.get_iterator_from_method(&iterable, &method);
                }
                if self.objects.array_len_if_array(*id)?.is_some() {
                    return Ok(IteratorSource::ArrayIndex {
                        array: iterable,
                        index: 0,
                    });
                }
                if let Some(text) = self.string_object_primitive_value(*id)? {
                    return Ok(chars_source(text));
                }
                Err(not_iterable_error(&iterable))
            }
            Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_) => {
                let Some(method) = self.iterator_method(&iterable)? else {
                    return Err(not_iterable_error(&iterable));
                };
                self.get_iterator_from_method(&iterable, &method)
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Symbol(_)
            | Value::Error(_) => Err(not_iterable_error(&iterable)),
        }
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
        let next = self.get_named(&iterator, ITERATOR_NEXT_PROPERTY)?;
        Ok(IteratorSource::Protocol {
            iterator,
            next,
            done: false,
        })
    }

    /// ECMAScript `IteratorStep` followed by `IteratorValue` when the result
    /// is not complete.
    pub(in crate::runtime) fn iterator_step(
        &mut self,
        source: &mut IteratorSource,
    ) -> Result<IteratorStep> {
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
            IteratorSource::Chars { chars } => match chars.next() {
                Some(ch) => Ok(IteratorStep::Value(self.heap_string_char_value(ch)?)),
                None => Ok(IteratorStep::Done),
            },
            IteratorSource::Protocol {
                iterator,
                next,
                done,
            } => {
                if *done {
                    return Ok(IteratorStep::Done);
                }
                let next = next.clone();
                let iterator = iterator.clone();
                let result = match self.call(&next, &[], iterator)? {
                    Completion::Normal(value) => value,
                    Completion::Throw(value) => {
                        set_protocol_done(source);
                        return Ok(IteratorStep::Abrupt(Completion::Throw(value)));
                    }
                    completion => {
                        return completion.into_result().map(IteratorStep::Value);
                    }
                };
                if self.semantic_object_ref(&result)?.is_none() {
                    return Err(Error::type_error(format!(
                        "iterator result '{result}' is not an object"
                    )));
                }
                if to_boolean(&self.get_named(&result, ITERATOR_RESULT_DONE_PROPERTY)?) {
                    set_protocol_done(source);
                    return Ok(IteratorStep::Done);
                }
                Ok(IteratorStep::Value(
                    self.get_named(&result, ITERATOR_RESULT_VALUE_PROPERTY)?,
                ))
            }
        }
    }

    /// ECMAScript `IteratorClose`, including the rule that an original throw
    /// completion wins over failures while looking up or calling `return`.
    pub(in crate::runtime) fn iterator_close(
        &mut self,
        source: &mut IteratorSource,
        completion: Completion,
    ) -> Result<Completion> {
        let Some(iterator) = protocol_iterator_to_close(source) else {
            return Ok(completion);
        };
        let original_is_throw = matches!(completion, Completion::Throw(_));
        let return_method = match self.get_named_method(&iterator, ITERATOR_RETURN_PROPERTY) {
            Ok(method) => method,
            Err(_error) if original_is_throw => return Ok(completion),
            Err(error) => return Err(error),
        };
        let Some(return_method) = return_method else {
            return Ok(completion);
        };
        let close_completion = match self.call(&return_method, &[], iterator) {
            Ok(close_completion) => close_completion,
            Err(_error) if original_is_throw => return Ok(completion),
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
            | Completion::Break { .. }
            | Completion::Continue(_)) => completion.into_result().map(Completion::Normal),
        }
    }

    /// Closes after an error already represented outside `Completion`.
    /// JavaScript throw precedence requires every close failure to be ignored.
    pub(in crate::runtime) fn iterator_close_on_error(
        &mut self,
        source: &mut IteratorSource,
        error: Error,
    ) -> Error {
        let Some(iterator) = protocol_iterator_to_close(source) else {
            return error;
        };
        if let Ok(Some(return_method)) = self.get_named_method(&iterator, ITERATOR_RETURN_PROPERTY)
        {
            let close_result = self.call(&return_method, &[], iterator);
            drop(close_result);
        }
        error
    }

    fn iterator_method(&mut self, iterable: &Value) -> Result<Option<Value>> {
        let Some(symbol) = self.iterator_symbol() else {
            return Ok(None);
        };
        let key = DynamicPropertyKey::new(
            ITERATOR_SYMBOL_DISPLAY_NAME.to_owned(),
            Some(PropertyKey::symbol(symbol)),
        );
        self.get_method(iterable, key.lookup())
    }
}

fn chars_source(text: &str) -> IteratorSource {
    IteratorSource::Chars {
        chars: text.chars().collect::<Vec<_>>().into_iter(),
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
