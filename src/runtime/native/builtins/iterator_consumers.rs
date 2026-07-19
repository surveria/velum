#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{IteratorSource, IteratorStep, to_boolean},
        call::RuntimeCallArgs,
        control::Completion,
    },
    value::Value,
};

const ITERATOR_RECEIVER_ERROR: &str = "Iterator helper method requires an object receiver";
const ITERATOR_CALLBACK_ERROR: &str = "Iterator helper callback must be callable";
const REDUCE_EMPTY_ERROR: &str =
    "Iterator.prototype.reduce of a finished iterator with no initial value";
const ITERATOR_NEXT_NAME: &str = "next";
const CONSUMER_STEP_CHARGE: usize = 1;

/// Which eager consumer drives the shared iteration loop.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum IteratorConsumer {
    ToArray,
    ForEach,
    Some,
    Every,
    Find,
}

impl Context {
    fn consumer_source(&mut self, this_value: &Value) -> Result<IteratorSource> {
        if self.semantic_object_ref(this_value)?.is_none() {
            return Err(Error::type_error(ITERATOR_RECEIVER_ERROR));
        }
        let next = self.get_named(this_value, ITERATOR_NEXT_NAME)?;
        Ok(IteratorSource::Protocol {
            iterator: this_value.clone(),
            next,
            done: false,
        })
    }

    fn consumer_callable(&self, args: &RuntimeCallArgs<'_>) -> Result<Value> {
        let callback = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_callable(&callback)? {
            return Err(Error::type_error(ITERATOR_CALLBACK_ERROR));
        }
        Ok(callback)
    }

    fn close_after_consumer_validation_error(&mut self, iterator: &Value, error: Error) -> Error {
        let mut source = IteratorSource::Protocol {
            iterator: iterator.clone(),
            next: Value::Undefined,
            done: false,
        };
        self.iterator_close_on_error(&mut source, error)
    }

    fn consumer_callback_call(
        &mut self,
        source: &mut IteratorSource,
        callback: &Value,
        value: &Value,
        counter: f64,
    ) -> Result<Value> {
        let args = [value.clone(), Value::Number(counter)];
        let completion = match self.call(callback, &args, Value::Undefined) {
            Ok(completion) => completion,
            Err(error) => return Err(self.iterator_close_on_error(source, error)),
        };
        match completion {
            Completion::Normal(result) => Ok(result),
            completion => match completion.into_result() {
                Ok(value) => Ok(value),
                Err(error) => Err(self.iterator_close_on_error(source, error)),
            },
        }
    }

    /// Closes the iterator with a normal completion carrying `value` and
    /// propagates a close failure as the result error.
    fn consumer_close(&mut self, source: &mut IteratorSource, value: Value) -> Result<Value> {
        self.iterator_close(source, Completion::Normal(value))?
            .into_result()
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_to_array(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let mut source = self.consumer_source(this_value)?;
        let mut items = Vec::new();
        loop {
            self.charge_runtime_steps(CONSUMER_STEP_CHARGE)?;
            match self.iterator_step(&mut source)? {
                IteratorStep::Value(value) => items.push(value),
                IteratorStep::Done => break,
                IteratorStep::Abrupt(completion) => {
                    completion.into_result()?;
                    break;
                }
            }
        }
        self.create_array_from_elements(items)
    }

    pub(in crate::runtime::native) fn eval_iterator_consumer(
        &mut self,
        consumer: IteratorConsumer,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if consumer == IteratorConsumer::ToArray {
            return self.eval_iterator_prototype_to_array(this_value);
        }
        if self.semantic_object_ref(this_value)?.is_none() {
            return Err(Error::type_error(ITERATOR_RECEIVER_ERROR));
        }
        let callback = match self.consumer_callable(&args) {
            Ok(callback) => callback,
            Err(error) => {
                return Err(self.close_after_consumer_validation_error(this_value, error));
            }
        };
        let next = self.get_named(this_value, ITERATOR_NEXT_NAME)?;
        let mut source = IteratorSource::Protocol {
            iterator: this_value.clone(),
            next,
            done: false,
        };
        let mut counter = 0.0;
        loop {
            self.charge_runtime_steps(CONSUMER_STEP_CHARGE)?;
            let value = match self.iterator_step(&mut source)? {
                IteratorStep::Value(value) => value,
                IteratorStep::Done => break,
                IteratorStep::Abrupt(completion) => {
                    completion.into_result()?;
                    break;
                }
            };
            let result = self.consumer_callback_call(&mut source, &callback, &value, counter)?;
            counter += 1.0;
            match consumer {
                IteratorConsumer::ForEach | IteratorConsumer::ToArray => {}
                IteratorConsumer::Some => {
                    if to_boolean(self, &result)? {
                        return self.consumer_close(&mut source, Value::Bool(true));
                    }
                }
                IteratorConsumer::Every => {
                    if !to_boolean(self, &result)? {
                        return self.consumer_close(&mut source, Value::Bool(false));
                    }
                }
                IteratorConsumer::Find => {
                    if to_boolean(self, &result)? {
                        return self.consumer_close(&mut source, value);
                    }
                }
            }
        }
        match consumer {
            IteratorConsumer::ForEach | IteratorConsumer::ToArray | IteratorConsumer::Find => {
                Ok(Value::Undefined)
            }
            IteratorConsumer::Some => Ok(Value::Bool(false)),
            IteratorConsumer::Every => Ok(Value::Bool(true)),
        }
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_reduce(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if self.semantic_object_ref(this_value)?.is_none() {
            return Err(Error::type_error(ITERATOR_RECEIVER_ERROR));
        }
        let reducer = match self.consumer_callable(&args) {
            Ok(reducer) => reducer,
            Err(error) => {
                return Err(self.close_after_consumer_validation_error(this_value, error));
            }
        };
        let initial = args.as_slice().get(1).cloned();
        let next = self.get_named(this_value, ITERATOR_NEXT_NAME)?;
        let mut source = IteratorSource::Protocol {
            iterator: this_value.clone(),
            next,
            done: false,
        };
        let mut counter = 0.0;
        let mut accumulator = match initial {
            Some(value) => value,
            None => match self.iterator_step(&mut source)? {
                IteratorStep::Value(value) => {
                    counter = 1.0;
                    value
                }
                IteratorStep::Done => {
                    return Err(Error::type_error(REDUCE_EMPTY_ERROR));
                }
                IteratorStep::Abrupt(completion) => return completion.into_result(),
            },
        };
        loop {
            self.charge_runtime_steps(CONSUMER_STEP_CHARGE)?;
            let value = match self.iterator_step(&mut source)? {
                IteratorStep::Value(value) => value,
                IteratorStep::Done => break,
                IteratorStep::Abrupt(completion) => {
                    completion.into_result()?;
                    break;
                }
            };
            let call_args = [accumulator.clone(), value, Value::Number(counter)];
            let completion = match self.call(&reducer, &call_args, Value::Undefined) {
                Ok(completion) => completion,
                Err(error) => return Err(self.iterator_close_on_error(&mut source, error)),
            };
            accumulator = match completion {
                Completion::Normal(result) => result,
                completion => match completion.into_result() {
                    Ok(result) => result,
                    Err(error) => return Err(self.iterator_close_on_error(&mut source, error)),
                },
            };
            counter += 1.0;
        }
        Ok(accumulator)
    }
}
