#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{IteratorSource, IteratorStep},
        collections::{CollectionIteratorId, InnerIteratorState, IteratorHelperMode},
        control::Completion,
        native::IteratorFunctionKind,
        object::{ObjectPropertyInit, PropertyEnumerable},
    },
    value::{ObjectId, Value},
};

use super::{
    NativeFunctionKind,
    iterator::{ITERATOR_NEXT_NAME, ITERATOR_RETURN_NAME},
};

const ITERATOR_RESULT_VALUE_NAME: &str = "value";
const ITERATOR_RESULT_DONE_NAME: &str = "done";
const ITERATOR_STEP_CHARGE: usize = 1;
const FLAT_MAP_PRIMITIVE_ERROR: &str = "Iterator.prototype.flatMap result must be an object";

/// The next lazy action one helper `next()` call decided on while holding the
/// state borrow. JavaScript re-entry happens only after the borrow ends.
enum HelperPlan {
    Finished,
    StepOuter { iterator: Value, next: Value },
    StepInner { iterator: Value, next: Value },
    CloseTakeLimit { iterator: Value, next: Value },
}

impl Context {
    pub(super) fn install_iterator_helper_prototype_methods(
        &mut self,
        prototype: ObjectId,
    ) -> Result<()> {
        let next = self.create_native_function(
            NativeFunctionKind::Iterator(IteratorFunctionKind::HelperPrototypeNext),
            Value::Undefined,
        )?;
        let return_fn = self.create_native_function(
            NativeFunctionKind::Iterator(IteratorFunctionKind::HelperPrototypeReturn),
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(prototype, ITERATOR_NEXT_NAME, next)?;
        self.define_non_enumerable_object_property(prototype, ITERATOR_RETURN_NAME, return_fn)
    }

    pub(super) fn iterator_inherits_prototype(
        &mut self,
        iterator: &Value,
        target: ObjectId,
    ) -> Result<bool> {
        let target = Value::Object(target);
        let mut current = iterator.clone();
        loop {
            let Some(prototype) = self.semantic_get_prototype(&current)? else {
                return Ok(false);
            };
            if crate::runtime::abstract_operations::same_value(&prototype, &target) {
                return Ok(true);
            }
            if matches!(prototype, Value::Null) {
                return Ok(false);
            }
            self.step()?;
            current = prototype;
        }
    }

    pub(in crate::runtime::native) fn create_iterator_result_object(
        &mut self,
        value: Value,
        done: bool,
    ) -> Result<Value> {
        let value_key = self.intern_property_key(ITERATOR_RESULT_VALUE_NAME)?;
        let done_key = self.intern_property_key(ITERATOR_RESULT_DONE_NAME)?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create(
            vec![
                ObjectPropertyInit::new(
                    value_key,
                    ITERATOR_RESULT_VALUE_NAME,
                    value,
                    PropertyEnumerable::Yes,
                ),
                ObjectPropertyInit::new(
                    done_key,
                    ITERATOR_RESULT_DONE_NAME,
                    Value::Bool(done),
                    PropertyEnumerable::Yes,
                ),
            ],
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    /// One `IteratorStepValue` over a protocol pair, translating an abrupt
    /// `next` completion into an error after latching the helper as done.
    fn protocol_step(
        &mut self,
        iterator: Value,
        next: Value,
        state_id: CollectionIteratorId,
    ) -> Result<Option<Value>> {
        let mut source = IteratorSource::Protocol {
            iterator,
            next,
            done: false,
        };
        match self.iterator_step(&mut source)? {
            IteratorStep::Value(value) => Ok(Some(value)),
            IteratorStep::Done => {
                self.iterator_helper_state_mut(state_id)?.done = true;
                Ok(None)
            }
            IteratorStep::Abrupt(completion) => {
                self.iterator_helper_state_mut(state_id)?.done = true;
                completion.into_result().map(Some)
            }
        }
    }

    /// Closes the underlying iterator after a callback threw, preserving the
    /// original error, and latches the helper as done.
    fn close_after_callback_error(
        &mut self,
        iterator: Value,
        next: Value,
        state_id: CollectionIteratorId,
        error: Error,
    ) -> Error {
        if let Ok(state) = self.iterator_helper_state_mut(state_id) {
            state.done = true;
        }
        let mut source = IteratorSource::Protocol {
            iterator,
            next,
            done: false,
        };
        self.iterator_close_on_error(&mut source, error)
    }

    fn helper_callback_call(
        &mut self,
        callback: &Value,
        value: &Value,
        counter: f64,
    ) -> Result<Value> {
        let args = [value.clone(), Value::Number(counter)];
        match self.call(callback, &args, Value::Undefined)? {
            Completion::Normal(result) => Ok(result),
            completion => completion.into_result(),
        }
    }

    fn bump_helper_counter(&mut self, state_id: CollectionIteratorId) -> Result<f64> {
        let state = self.iterator_helper_state_mut(state_id)?;
        let counter = state.counter;
        state.counter += 1.0;
        Ok(counter)
    }

    pub(in crate::runtime::native) fn eval_iterator_helper_next(
        &mut self,
        state_id: CollectionIteratorId,
    ) -> Result<Value> {
        let state = self.iterator_helper_state_mut(state_id)?;
        if state.executing {
            return Err(Error::type_error("Iterator helper is already running"));
        }
        state.executing = true;
        let result = self.eval_iterator_helper_next_active(state_id);
        if let Ok(state) = self.iterator_helper_state_mut(state_id) {
            state.executing = false;
        }
        result
    }

    pub(in crate::runtime::native) fn eval_iterator_helper_next_method(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let state_id = self.iterator_helper_receiver_state(this_value)?;
        self.eval_iterator_helper_next(state_id)
    }

    fn eval_iterator_helper_next_active(
        &mut self,
        state_id: CollectionIteratorId,
    ) -> Result<Value> {
        loop {
            self.charge_runtime_steps(ITERATOR_STEP_CHARGE)?;
            let plan = self.plan_helper_step(state_id)?;
            match plan {
                HelperPlan::Finished => {
                    return self.create_iterator_result_object(Value::Undefined, true);
                }
                HelperPlan::CloseTakeLimit { iterator, next } => {
                    self.iterator_helper_state_mut(state_id)?.done = true;
                    let mut source = IteratorSource::Protocol {
                        iterator,
                        next,
                        done: false,
                    };
                    self.iterator_close(&mut source, Completion::Normal(Value::Undefined))?
                        .into_result()?;
                    return self.create_iterator_result_object(Value::Undefined, true);
                }
                HelperPlan::StepInner { iterator, next } => {
                    if let Some(value) = self.flat_map_inner_step(state_id, iterator, next)? {
                        return self.create_iterator_result_object(value, false);
                    }
                }
                HelperPlan::StepOuter { iterator, next } => {
                    let Some(value) =
                        self.protocol_step(iterator.clone(), next.clone(), state_id)?
                    else {
                        return self.create_iterator_result_object(Value::Undefined, true);
                    };
                    if let Some(result) = self.apply_helper_mode(state_id, iterator, next, value)? {
                        return self.create_iterator_result_object(result, false);
                    }
                }
            }
        }
    }

    /// Reads the state and decides the next lazy action without re-entering
    /// JavaScript while the borrow is held.
    fn plan_helper_step(&mut self, state_id: CollectionIteratorId) -> Result<HelperPlan> {
        let state = self.iterator_helper_state_mut(state_id)?;
        if state.done {
            return Ok(HelperPlan::Finished);
        }
        let iterator = state.iterator.clone();
        let next = state.next.clone();
        match &mut state.mode {
            IteratorHelperMode::Take { remaining } => {
                if *remaining <= 0.0 {
                    return Ok(HelperPlan::CloseTakeLimit { iterator, next });
                }
                if remaining.is_finite() {
                    *remaining -= 1.0;
                }
                Ok(HelperPlan::StepOuter { iterator, next })
            }
            IteratorHelperMode::FlatMap {
                inner: Some(inner), ..
            } => Ok(HelperPlan::StepInner {
                iterator: inner.iterator.clone(),
                next: inner.next.clone(),
            }),
            IteratorHelperMode::Map { .. }
            | IteratorHelperMode::Filter { .. }
            | IteratorHelperMode::Drop { .. }
            | IteratorHelperMode::FlatMap { inner: None, .. } => {
                Ok(HelperPlan::StepOuter { iterator, next })
            }
        }
    }

    /// Applies the helper transformation to one outer value. `None` means the
    /// loop must continue stepping.
    fn apply_helper_mode(
        &mut self,
        state_id: CollectionIteratorId,
        iterator: Value,
        next: Value,
        value: Value,
    ) -> Result<Option<Value>> {
        let mode_action = {
            let state = self.iterator_helper_state_mut(state_id)?;
            match &mut state.mode {
                IteratorHelperMode::Map { mapper } => ModeAction::Map(mapper.clone()),
                IteratorHelperMode::Filter { predicate } => ModeAction::Filter(predicate.clone()),
                IteratorHelperMode::FlatMap { mapper, .. } => ModeAction::FlatMap(mapper.clone()),
                IteratorHelperMode::Take { .. } => ModeAction::Pass,
                IteratorHelperMode::Drop { remaining } => {
                    if *remaining > 0.0 {
                        if remaining.is_finite() {
                            *remaining -= 1.0;
                        }
                        ModeAction::Skip
                    } else {
                        ModeAction::Pass
                    }
                }
            }
        };
        match mode_action {
            ModeAction::Pass => Ok(Some(value)),
            ModeAction::Skip => Ok(None),
            ModeAction::Map(mapper) => {
                let counter = self.bump_helper_counter(state_id)?;
                match self.helper_callback_call(&mapper, &value, counter) {
                    Ok(result) => Ok(Some(result)),
                    Err(error) => {
                        Err(self.close_after_callback_error(iterator, next, state_id, error))
                    }
                }
            }
            ModeAction::Filter(predicate) => {
                let counter = self.bump_helper_counter(state_id)?;
                match self.helper_callback_call(&predicate, &value, counter) {
                    Ok(selected) => {
                        if crate::runtime::abstract_operations::to_boolean(self, &selected)? {
                            Ok(Some(value))
                        } else {
                            Ok(None)
                        }
                    }
                    Err(error) => {
                        Err(self.close_after_callback_error(iterator, next, state_id, error))
                    }
                }
            }
            ModeAction::FlatMap(mapper) => {
                let counter = self.bump_helper_counter(state_id)?;
                let result = match self.helper_callback_call(&mapper, &value, counter) {
                    Ok(result) => result,
                    Err(error) => {
                        return Err(
                            self.close_after_callback_error(iterator, next, state_id, error)
                        );
                    }
                };
                match self.open_flat_map_inner(&result) {
                    Ok((inner_iterator, inner_next)) => {
                        let state = self.iterator_helper_state_mut(state_id)?;
                        if let IteratorHelperMode::FlatMap { inner, .. } = &mut state.mode {
                            *inner = Some(Box::new(InnerIteratorState {
                                iterator: inner_iterator,
                                next: inner_next,
                            }));
                        }
                        Ok(None)
                    }
                    Err(error) => {
                        Err(self.close_after_callback_error(iterator, next, state_id, error))
                    }
                }
            }
        }
    }

    /// `GetIteratorFlattenable(mapped, reject-primitives)` for `flatMap`.
    fn open_flat_map_inner(&mut self, mapped: &Value) -> Result<(Value, Value)> {
        if self.semantic_object_ref(mapped)?.is_none() {
            return Err(Error::type_error(FLAT_MAP_PRIMITIVE_ERROR));
        }
        let method = self.flattenable_iterator_method(mapped)?;
        let inner = if let Some(method) = method {
            self.call_value(&method, &[], mapped.clone())?
        } else {
            mapped.clone()
        };
        if self.semantic_object_ref(&inner)?.is_none() {
            return Err(Error::type_error(FLAT_MAP_PRIMITIVE_ERROR));
        }
        let next = self.iterator_direct_next(&inner)?;
        Ok((inner, next))
    }

    /// Steps the active inner iterator; `None` means the inner iterator is
    /// exhausted and the outer loop must continue.
    fn flat_map_inner_step(
        &mut self,
        state_id: CollectionIteratorId,
        iterator: Value,
        next: Value,
    ) -> Result<Option<Value>> {
        let mut source = IteratorSource::Protocol {
            iterator,
            next,
            done: false,
        };
        match self.iterator_step(&mut source) {
            Ok(IteratorStep::Value(value)) => Ok(Some(value)),
            Ok(IteratorStep::Done) => {
                let state = self.iterator_helper_state_mut(state_id)?;
                if let IteratorHelperMode::FlatMap { inner, .. } = &mut state.mode {
                    *inner = None;
                }
                Ok(None)
            }
            Ok(IteratorStep::Abrupt(completion)) => {
                let Err(error) = completion.into_result() else {
                    return Ok(None);
                };
                Err(self.close_outer_after_inner_error(state_id, error))
            }
            Err(error) => Err(self.close_outer_after_inner_error(state_id, error)),
        }
    }

    /// Inner iterator failures close the outer iterated record per
    /// `IfAbruptCloseIterator`.
    fn close_outer_after_inner_error(
        &mut self,
        state_id: CollectionIteratorId,
        error: Error,
    ) -> Error {
        let (iterator, next) = match self.iterator_helper_state_mut(state_id) {
            Ok(state) => {
                state.done = true;
                (state.iterator.clone(), state.next.clone())
            }
            Err(state_error) => return state_error,
        };
        let mut source = IteratorSource::Protocol {
            iterator,
            next,
            done: false,
        };
        self.iterator_close_on_error(&mut source, error)
    }

    pub(in crate::runtime::native) fn eval_iterator_helper_return(
        &mut self,
        state_id: CollectionIteratorId,
    ) -> Result<Value> {
        let (iterator, next, inner, was_done) = {
            let state = self.iterator_helper_state_mut(state_id)?;
            let was_done = state.done;
            state.done = true;
            let inner = if let IteratorHelperMode::FlatMap { inner, .. } = &mut state.mode {
                inner.take()
            } else {
                None
            };
            (state.iterator.clone(), state.next.clone(), inner, was_done)
        };
        if !was_done {
            if let Some(inner) = inner {
                let mut inner_source = IteratorSource::Protocol {
                    iterator: inner.iterator,
                    next: inner.next,
                    done: false,
                };
                self.iterator_close(&mut inner_source, Completion::Normal(Value::Undefined))?
                    .into_result()?;
            }
            let mut source = IteratorSource::Protocol {
                iterator,
                next,
                done: false,
            };
            self.iterator_close(&mut source, Completion::Normal(Value::Undefined))?
                .into_result()?;
        }
        self.create_iterator_result_object(Value::Undefined, true)
    }

    pub(in crate::runtime::native) fn eval_iterator_helper_return_method(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let state_id = self.iterator_helper_receiver_state(this_value)?;
        self.eval_iterator_helper_return(state_id)
    }

    fn iterator_helper_receiver_state(
        &mut self,
        this_value: &Value,
    ) -> Result<CollectionIteratorId> {
        let Some(kind) = self.iterator_receiver_state_function_kind(this_value)? else {
            return Err(Error::type_error(
                "Iterator helper method requires a helper receiver",
            ));
        };
        let NativeFunctionKind::Iterator(IteratorFunctionKind::HelperNext(state_id)) = kind else {
            return Err(Error::type_error(
                "Iterator helper method requires a helper receiver",
            ));
        };
        Ok(state_id)
    }

    pub(in crate::runtime::native) fn eval_wrapped_iterator_next(
        &mut self,
        _state_id: CollectionIteratorId,
        this_value: &Value,
    ) -> Result<Value> {
        let state_id = self.wrapped_iterator_receiver_state(this_value)?;
        let (iterator, next) = {
            let state = self.wrapped_iterator_state(state_id)?;
            (state.iterator.clone(), state.next.clone())
        };
        match self.call(&next, &[], iterator)? {
            Completion::Normal(value) => Ok(value),
            completion => completion.into_result(),
        }
    }

    pub(in crate::runtime::native) fn eval_wrapped_iterator_return(
        &mut self,
        _state_id: CollectionIteratorId,
        this_value: &Value,
    ) -> Result<Value> {
        let state_id = self.wrapped_iterator_receiver_state(this_value)?;
        let iterator = self.wrapped_iterator_state(state_id)?.iterator.clone();
        let return_method = self.get_named_method(&iterator, ITERATOR_RETURN_NAME)?;
        let Some(return_method) = return_method else {
            return self.create_iterator_result_object(Value::Undefined, true);
        };
        match self.call(&return_method, &[], iterator)? {
            Completion::Normal(value) => Ok(value),
            completion => completion.into_result(),
        }
    }

    fn wrapped_iterator_receiver_state(
        &mut self,
        this_value: &Value,
    ) -> Result<CollectionIteratorId> {
        let Some(NativeFunctionKind::Iterator(IteratorFunctionKind::WrapNext(state_id))) =
            self.iterator_receiver_state_function_kind(this_value)?
        else {
            return Err(Error::type_error(
                "WrapForValidIterator method requires a compatible receiver",
            ));
        };
        Ok(state_id)
    }
}

/// Owned copy of the mode decision so JavaScript callbacks run without any
/// live borrow of the helper state.
enum ModeAction {
    Pass,
    Skip,
    Map(Value),
    Filter(Value),
    FlatMap(Value),
}
