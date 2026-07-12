use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{
            AsyncIteratorCloseStep, AsyncIteratorContinuation, AsyncIteratorStep,
        },
        async_trace::VmAsyncEdgeKind,
        control::{Completion, runtime_exception_value},
        promise::{PromiseId, PromiseReaction},
        roots::{DirectRootVisitor, VmRootKind},
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::Value,
};

const ARRAY_FROM_ASYNC_INDEX_LIMIT_ERROR: &str = "Array.fromAsync index exceeded supported range";
const ARRAY_FROM_ASYNC_MAP_ERROR: &str = "Array.fromAsync map function is not callable";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug)]
enum ArrayFromAsyncSource {
    Iterator(AsyncIteratorContinuation),
    ArrayLike { items: Value, length: usize },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ArrayFromAsyncPending {
    IteratorStep,
    ArrayLikeValue,
    MappedValue,
    IteratorClose,
}

#[derive(Debug)]
pub(in crate::runtime) struct ArrayFromAsyncContinuation {
    result_promise: PromiseId,
    result: Value,
    map_function: Option<Value>,
    this_argument: Value,
    source: ArrayFromAsyncSource,
    index: usize,
    pending: Option<ArrayFromAsyncPending>,
}

impl ArrayFromAsyncContinuation {
    pub(in crate::runtime) fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        visitor.visit(
            VmAsyncEdgeKind::PromiseReaction,
            StrongEdgeReference::Promise(self.result_promise),
        )?;
        visitor.visit(
            VmAsyncEdgeKind::PromiseReaction,
            StrongEdgeReference::Value(&self.result),
        )?;
        if let Some(map_function) = &self.map_function {
            visitor.visit(
                VmAsyncEdgeKind::PromiseReaction,
                StrongEdgeReference::Value(map_function),
            )?;
        }
        visitor.visit(
            VmAsyncEdgeKind::PromiseReaction,
            StrongEdgeReference::Value(&self.this_argument),
        )?;
        match &self.source {
            ArrayFromAsyncSource::Iterator(iterator) => {
                for value in iterator.root_values() {
                    visitor.visit(
                        VmAsyncEdgeKind::PromiseReaction,
                        StrongEdgeReference::Value(value),
                    )?;
                }
            }
            ArrayFromAsyncSource::ArrayLike { items, .. } => visitor.visit(
                VmAsyncEdgeKind::PromiseReaction,
                StrongEdgeReference::Value(items),
            )?,
        }
        Ok(())
    }

    pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        visitor.visit_promise(VmRootKind::QueuedJob, self.result_promise)?;
        visitor.visit_value(VmRootKind::QueuedJob, &self.result)?;
        if let Some(map_function) = &self.map_function {
            visitor.visit_value(VmRootKind::QueuedJob, map_function)?;
        }
        visitor.visit_value(VmRootKind::QueuedJob, &self.this_argument)?;
        match &self.source {
            ArrayFromAsyncSource::Iterator(iterator) => {
                for value in iterator.root_values() {
                    visitor.visit_value(VmRootKind::QueuedJob, value)?;
                }
            }
            ArrayFromAsyncSource::ArrayLike { items, .. } => {
                visitor.visit_value(VmRootKind::QueuedJob, items)?;
            }
        }
        Ok(())
    }
}

enum ArrayFromAsyncDrive {
    Await(PromiseId),
    Resolve(Value),
    Reject(Value),
}

impl Context {
    pub(in crate::runtime) fn start_array_from_async(
        &mut self,
        args: &[Value],
        constructor: &Value,
    ) -> Result<Value> {
        let (result_promise, promise_object) = self.create_pending_promise()?;
        let items = args.first().cloned().unwrap_or(Value::Undefined);
        let map_function = args.get(1).cloned().unwrap_or(Value::Undefined);
        let this_argument = args.get(2).cloned().unwrap_or(Value::Undefined);
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            [
                &promise_object,
                constructor,
                &items,
                &map_function,
                &this_argument,
            ],
        )?;
        let setup = self.prepare_array_from_async(
            result_promise,
            constructor,
            items,
            map_function,
            this_argument,
        );
        match setup {
            Ok(continuation) => {
                self.continue_array_from_async(continuation, None)?;
            }
            Err(error) => self.reject_array_from_async_error(result_promise, &error)?,
        }
        Ok(promise_object)
    }

    fn prepare_array_from_async(
        &mut self,
        result_promise: PromiseId,
        constructor: &Value,
        items: Value,
        map_function: Value,
        this_argument: Value,
    ) -> Result<ArrayFromAsyncContinuation> {
        let map_function = if matches!(map_function, Value::Undefined) {
            None
        } else if self.semantic_is_callable(&map_function)? {
            Some(map_function)
        } else {
            return Err(Error::type_error(ARRAY_FROM_ASYNC_MAP_ERROR));
        };
        let source = if let Some(method) = self.async_iterator_method(&items)? {
            let source = self.get_iterator_from_method(&items, &method)?;
            ArrayFromAsyncSource::Iterator(AsyncIteratorContinuation::new(source, false))
        } else if let Some(method) = self.iterator_method(&items)? {
            let source = self.get_iterator_from_method_with_array_fast_path(&items, &method)?;
            ArrayFromAsyncSource::Iterator(AsyncIteratorContinuation::new(source, true))
        } else {
            let length = self.array_like_length(&items)?;
            ArrayFromAsyncSource::ArrayLike { items, length }
        };
        let length = match &source {
            ArrayFromAsyncSource::Iterator(_) => None,
            ArrayFromAsyncSource::ArrayLike { length, .. } => Some(*length),
        };
        let result = self.array_from_result(constructor, length)?;
        Ok(ArrayFromAsyncContinuation {
            result_promise,
            result,
            map_function,
            this_argument,
            source,
            index: 0,
            pending: None,
        })
    }

    pub(in crate::runtime) fn resume_array_from_async(
        &mut self,
        continuation: ArrayFromAsyncContinuation,
        resume: Completion,
    ) -> Result<()> {
        self.continue_array_from_async(continuation, Some(resume))
    }

    fn continue_array_from_async(
        &mut self,
        mut continuation: ArrayFromAsyncContinuation,
        resume: Option<Completion>,
    ) -> Result<()> {
        let result_promise = continuation.result_promise;
        let drive = self.drive_array_from_async(&mut continuation, resume);
        match drive {
            Ok(ArrayFromAsyncDrive::Await(awaited)) => {
                let reaction = PromiseReaction::awaiting_array_from_async(continuation);
                if let Err(error) = self.add_promise_reaction(awaited, reaction) {
                    self.reject_array_from_async_error(result_promise, &error)?;
                }
            }
            Ok(ArrayFromAsyncDrive::Resolve(value)) => {
                self.resolve_promise(result_promise, value)?;
            }
            Ok(ArrayFromAsyncDrive::Reject(reason)) => {
                self.reject_promise(result_promise, reason)?;
            }
            Err(error) => self.reject_array_from_async_error(result_promise, &error)?,
        }
        Ok(())
    }

    fn drive_array_from_async(
        &mut self,
        continuation: &mut ArrayFromAsyncContinuation,
        resume: Option<Completion>,
    ) -> Result<ArrayFromAsyncDrive> {
        let _state_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            std::iter::once(&continuation.result)
                .chain(continuation.map_function.iter())
                .chain(std::iter::once(&continuation.this_argument)),
        )?;
        let _source_scope = match &continuation.source {
            ArrayFromAsyncSource::Iterator(iterator) => Some(
                self.transient_root_scope(VmRootKind::TransientTemporary, iterator.root_values())?,
            ),
            ArrayFromAsyncSource::ArrayLike { items, .. } => Some(
                self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(items))?,
            ),
        };
        match (continuation.pending.take(), resume) {
            (None, None) => self.next_array_from_async_value(continuation),
            (Some(ArrayFromAsyncPending::IteratorStep), Some(resume)) => {
                self.step_array_from_async_iterator(continuation, Some(resume))
            }
            (Some(ArrayFromAsyncPending::ArrayLikeValue), Some(Completion::Normal(value))) => {
                self.consume_array_from_async_value(continuation, value)
            }
            (Some(ArrayFromAsyncPending::ArrayLikeValue), Some(Completion::Throw(reason))) => {
                Ok(ArrayFromAsyncDrive::Reject(reason))
            }
            (Some(ArrayFromAsyncPending::MappedValue), Some(Completion::Normal(value))) => {
                self.define_array_from_async_value(continuation, value)
            }
            (Some(ArrayFromAsyncPending::MappedValue), Some(Completion::Throw(reason))) => {
                self.close_array_from_async_iterator(continuation, Completion::Throw(reason), None)
            }
            (Some(ArrayFromAsyncPending::IteratorClose), Some(resume)) => {
                self.resume_array_from_async_close(continuation, resume)
            }
            (Some(_), Some(completion)) => Err(Error::runtime(format!(
                "invalid Array.fromAsync resume completion {completion:?}"
            ))),
            (Some(_), None) => Err(Error::runtime(
                "Array.fromAsync continuation resumed without a completion",
            )),
            (None, Some(_)) => Err(Error::runtime(
                "Array.fromAsync received an unexpected resume completion",
            )),
        }
    }

    fn next_array_from_async_value(
        &mut self,
        continuation: &mut ArrayFromAsyncContinuation,
    ) -> Result<ArrayFromAsyncDrive> {
        if matches!(continuation.source, ArrayFromAsyncSource::Iterator(_)) {
            if u64::try_from(continuation.index).map_or(true, |index| index >= MAX_SAFE_INTEGER) {
                let error = Error::type_error(ARRAY_FROM_ASYNC_INDEX_LIMIT_ERROR);
                return self.close_array_from_async_error(continuation, &error);
            }
            return self.step_array_from_async_iterator(continuation, None);
        }
        let ArrayFromAsyncSource::ArrayLike { items, length } = &continuation.source else {
            return Err(Error::runtime("Array.fromAsync source kind disappeared"));
        };
        if continuation.index >= *length {
            self.set_array_like_length(&continuation.result, *length)?;
            return Ok(ArrayFromAsyncDrive::Resolve(continuation.result.clone()));
        }
        let items = items.clone();
        let value = self.get_array_like_index(&items, continuation.index)?;
        let awaited = self.promise_resolve_for_await(value)?;
        continuation.pending = Some(ArrayFromAsyncPending::ArrayLikeValue);
        Ok(ArrayFromAsyncDrive::Await(awaited))
    }

    fn step_array_from_async_iterator(
        &mut self,
        continuation: &mut ArrayFromAsyncContinuation,
        resume: Option<Completion>,
    ) -> Result<ArrayFromAsyncDrive> {
        let ArrayFromAsyncSource::Iterator(iterator) = &mut continuation.source else {
            return Err(Error::runtime(
                "Array.fromAsync iterator source disappeared",
            ));
        };
        match self.async_iterator_step(iterator, resume)? {
            AsyncIteratorStep::Await(awaited) => {
                continuation.pending = Some(ArrayFromAsyncPending::IteratorStep);
                Ok(ArrayFromAsyncDrive::Await(awaited))
            }
            AsyncIteratorStep::Value(value) => {
                self.consume_array_from_async_value(continuation, value)
            }
            AsyncIteratorStep::Done => {
                self.set_array_like_length(&continuation.result, continuation.index)?;
                Ok(ArrayFromAsyncDrive::Resolve(continuation.result.clone()))
            }
            AsyncIteratorStep::Abrupt(Completion::Throw(reason)) => {
                Ok(ArrayFromAsyncDrive::Reject(reason))
            }
            AsyncIteratorStep::Abrupt(completion) => Err(Error::runtime(format!(
                "invalid Array.fromAsync iterator completion {completion:?}"
            ))),
        }
    }

    fn consume_array_from_async_value(
        &mut self,
        continuation: &mut ArrayFromAsyncContinuation,
        value: Value,
    ) -> Result<ArrayFromAsyncDrive> {
        let Some(map_function) = continuation.map_function.clone() else {
            return self.define_array_from_async_value(continuation, value);
        };
        let index = Self::array_like_index_value(continuation.index)?;
        let mapped = match self.call_value(
            &map_function,
            &[value, index],
            continuation.this_argument.clone(),
        ) {
            Ok(mapped) => mapped,
            Err(error) => return self.close_array_from_async_error(continuation, &error),
        };
        let awaited = match self.promise_resolve_for_await(mapped) {
            Ok(awaited) => awaited,
            Err(error) => return self.close_array_from_async_error(continuation, &error),
        };
        continuation.pending = Some(ArrayFromAsyncPending::MappedValue);
        Ok(ArrayFromAsyncDrive::Await(awaited))
    }

    fn define_array_from_async_value(
        &mut self,
        continuation: &mut ArrayFromAsyncContinuation,
        value: Value,
    ) -> Result<ArrayFromAsyncDrive> {
        if let Err(error) =
            self.array_from_create_data_property(&continuation.result, continuation.index, value)
        {
            return self.close_array_from_async_error(continuation, &error);
        }
        continuation.index = continuation
            .index
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_FROM_ASYNC_INDEX_LIMIT_ERROR))?;
        self.next_array_from_async_value(continuation)
    }

    fn close_array_from_async_error(
        &mut self,
        continuation: &mut ArrayFromAsyncContinuation,
        error: &Error,
    ) -> Result<ArrayFromAsyncDrive> {
        let Some(reason) = runtime_exception_value(self, error)? else {
            return Err(error.clone());
        };
        self.close_array_from_async_iterator(continuation, Completion::Throw(reason), None)
    }

    fn close_array_from_async_iterator(
        &mut self,
        continuation: &mut ArrayFromAsyncContinuation,
        completion: Completion,
        resume: Option<Completion>,
    ) -> Result<ArrayFromAsyncDrive> {
        let ArrayFromAsyncSource::Iterator(iterator) = &mut continuation.source else {
            let Completion::Throw(reason) = completion else {
                return Err(Error::runtime(
                    "Array.fromAsync array-like close completion was not a throw",
                ));
            };
            return Ok(ArrayFromAsyncDrive::Reject(reason));
        };
        match self.async_iterator_close(iterator, Some(completion), resume)? {
            AsyncIteratorCloseStep::Await(awaited) => {
                continuation.pending = Some(ArrayFromAsyncPending::IteratorClose);
                Ok(ArrayFromAsyncDrive::Await(awaited))
            }
            AsyncIteratorCloseStep::Complete(Completion::Throw(reason)) => {
                Ok(ArrayFromAsyncDrive::Reject(reason))
            }
            AsyncIteratorCloseStep::Complete(completion) => Err(Error::runtime(format!(
                "invalid Array.fromAsync close completion {completion:?}"
            ))),
        }
    }

    fn resume_array_from_async_close(
        &mut self,
        continuation: &mut ArrayFromAsyncContinuation,
        resume: Completion,
    ) -> Result<ArrayFromAsyncDrive> {
        let ArrayFromAsyncSource::Iterator(iterator) = &mut continuation.source else {
            return Err(Error::runtime(
                "Array.fromAsync close lost its iterator source",
            ));
        };
        match self.async_iterator_close(iterator, None, Some(resume))? {
            AsyncIteratorCloseStep::Await(awaited) => {
                continuation.pending = Some(ArrayFromAsyncPending::IteratorClose);
                Ok(ArrayFromAsyncDrive::Await(awaited))
            }
            AsyncIteratorCloseStep::Complete(Completion::Throw(reason)) => {
                Ok(ArrayFromAsyncDrive::Reject(reason))
            }
            AsyncIteratorCloseStep::Complete(completion) => Err(Error::runtime(format!(
                "invalid Array.fromAsync close completion {completion:?}"
            ))),
        }
    }

    fn reject_array_from_async_error(
        &mut self,
        result_promise: PromiseId,
        error: &Error,
    ) -> Result<()> {
        let Some(reason) = runtime_exception_value(self, error)? else {
            return Err(error.clone());
        };
        self.reject_promise(result_promise, reason)
    }
}
