use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, control::Completion},
    value::Value,
};

const ARRAY_CALLBACK_NOT_CALLABLE_ERROR: &str = "Array.prototype callback must be callable";
const ARRAY_REDUCE_EMPTY_ERROR: &str = "Reduce of empty array with no initial value";
const INDEX_NOT_FOUND: f64 = -1.0;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CallbackVisitMode {
    PresentOnly,
    EveryIndex,
}

impl Context {
    pub(in crate::runtime::native) fn eval_array_for_each(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_for_each(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_for_each(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = Self::array_callback_and_this_arg(args)?;
        self.visit_array_like_present(this_value, |context, index, value| {
            context.call_array_callback(
                callback,
                callback_this.clone(),
                value,
                index,
                this_value,
            )?;
            Ok(ArrayCallbackAction::Continue)
        })?;
        Ok(Value::Undefined)
    }

    pub(in crate::runtime::native) fn eval_array_map(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_map(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_map(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = Self::array_callback_and_this_arg(args)?;
        let length = self.array_like_length_for_callback(this_value)?;
        let result = self.create_array_callback_result(length)?;
        self.visit_array_like_present(this_value, |context, index, value| {
            let mapped = context.call_array_callback(
                callback,
                callback_this.clone(),
                value,
                index,
                this_value,
            )?;
            context.set_array_like_index(&result, index, mapped)?;
            Ok(ArrayCallbackAction::Continue)
        })?;
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_array_filter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_filter(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_filter(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = Self::array_callback_and_this_arg(args)?;
        let result = self.create_array_callback_result(0)?;
        self.visit_array_like_present(this_value, |context, index, value| {
            let keep = context.call_array_callback(
                callback,
                callback_this.clone(),
                value,
                index,
                this_value,
            )?;
            if keep.is_truthy() {
                context.push_array_callback_result(&result, value.clone())?;
            }
            Ok(ArrayCallbackAction::Continue)
        })?;
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_array_some(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_some(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_some(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = Self::array_callback_and_this_arg(args)?;
        let mut matched = false;
        self.visit_array_like_present(this_value, |context, index, value| {
            let result = context.call_array_callback(
                callback,
                callback_this.clone(),
                value,
                index,
                this_value,
            )?;
            if result.is_truthy() {
                matched = true;
                return Ok(ArrayCallbackAction::Stop);
            }
            Ok(ArrayCallbackAction::Continue)
        })?;
        Ok(Value::Bool(matched))
    }

    pub(in crate::runtime::native) fn eval_array_every(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_every(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_every(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = Self::array_callback_and_this_arg(args)?;
        let mut matched = true;
        self.visit_array_like_present(this_value, |context, index, value| {
            let result = context.call_array_callback(
                callback,
                callback_this.clone(),
                value,
                index,
                this_value,
            )?;
            if !result.is_truthy() {
                matched = false;
                return Ok(ArrayCallbackAction::Stop);
            }
            Ok(ArrayCallbackAction::Continue)
        })?;
        Ok(Value::Bool(matched))
    }

    pub(in crate::runtime::native) fn eval_array_find(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_find(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_find(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = Self::array_callback_and_this_arg(args)?;
        let mut found = Value::Undefined;
        self.visit_array_like_indices(
            this_value,
            CallbackVisitMode::EveryIndex,
            |context, index, value| {
                let result = context.call_array_callback(
                    callback,
                    callback_this.clone(),
                    value,
                    index,
                    this_value,
                )?;
                if result.is_truthy() {
                    found = value.clone();
                    return Ok(ArrayCallbackAction::Stop);
                }
                Ok(ArrayCallbackAction::Continue)
            },
        )?;
        Ok(found)
    }

    pub(in crate::runtime::native) fn eval_array_find_index(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_find_index(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_find_index(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (callback, callback_this) = Self::array_callback_and_this_arg(args)?;
        let mut found = Value::Number(INDEX_NOT_FOUND);
        self.visit_array_like_indices(
            this_value,
            CallbackVisitMode::EveryIndex,
            |context, index, value| {
                let result = context.call_array_callback(
                    callback,
                    callback_this.clone(),
                    value,
                    index,
                    this_value,
                )?;
                if result.is_truthy() {
                    found = Self::array_like_index_value(index)?;
                    return Ok(ArrayCallbackAction::Stop);
                }
                Ok(ArrayCallbackAction::Continue)
            },
        )?;
        Ok(found)
    }

    pub(in crate::runtime::native) fn eval_array_reduce(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_reduce(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_reduce(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_reduce_with_direction(args, this_value, ReduceDirection::Forward)
    }

    pub(in crate::runtime::native) fn eval_array_reduce_right(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_reduce_right(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_reduce_right(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_reduce_with_direction(args, this_value, ReduceDirection::Reverse)
    }

    fn visit_array_like_present<F>(&mut self, object: &Value, visitor: F) -> Result<()>
    where
        F: FnMut(&mut Self, usize, &Value) -> Result<ArrayCallbackAction>,
    {
        self.visit_array_like_indices(object, CallbackVisitMode::PresentOnly, visitor)
    }

    fn visit_array_like_indices<F>(
        &mut self,
        object: &Value,
        mode: CallbackVisitMode,
        mut visitor: F,
    ) -> Result<()>
    where
        F: FnMut(&mut Self, usize, &Value) -> Result<ArrayCallbackAction>,
    {
        let length = self.array_like_length_for_callback(object)?;
        for index in 0..length {
            self.step()?;
            let present = self.has_array_like_index(object, index)?;
            if mode == CallbackVisitMode::PresentOnly && !present {
                continue;
            }
            let value = if present {
                self.get_array_like_index(object, index)?
            } else {
                Value::Undefined
            };
            if visitor(self, index, &value)? == ArrayCallbackAction::Stop {
                return Ok(());
            }
        }
        Ok(())
    }

    fn eval_direct_array_reduce_with_direction(
        &mut self,
        args: &[Value],
        this_value: &Value,
        direction: ReduceDirection,
    ) -> Result<Value> {
        let callback = Self::array_callback_arg(args)?;
        let has_initial = args.get(1).is_some();
        let initial = args.get(1).cloned();
        let length = self.array_like_length_for_callback(this_value)?;
        let Some(mut state) = self.initial_reduce_state(this_value, length, direction, initial)?
        else {
            return Err(Error::type_error(ARRAY_REDUCE_EMPTY_ERROR));
        };

        while let Some(index) = state.next_index(direction) {
            self.step()?;
            if self.has_array_like_index(this_value, index)? {
                let value = self.get_array_like_index(this_value, index)?;
                if has_initial || state.started {
                    state.accumulator = self.call_reduce_callback(
                        callback,
                        state.accumulator,
                        value,
                        index,
                        this_value,
                    )?;
                } else {
                    state.accumulator = value;
                    state.started = true;
                }
            }
        }

        if state.started || has_initial {
            return Ok(state.accumulator);
        }
        Err(Error::type_error(ARRAY_REDUCE_EMPTY_ERROR))
    }

    fn initial_reduce_state(
        &mut self,
        object: &Value,
        length: usize,
        direction: ReduceDirection,
        initial: Option<Value>,
    ) -> Result<Option<ReduceState>> {
        Self::ensure_array_like_object(object)?;
        if let Some(accumulator) = initial {
            let next = match direction {
                ReduceDirection::Forward => 0,
                ReduceDirection::Reverse => length,
            };
            return Ok(Some(ReduceState::with_next(
                accumulator,
                next,
                length,
                true,
            )));
        }
        match direction {
            ReduceDirection::Forward => self.initial_forward_reduce_state(object, length),
            ReduceDirection::Reverse => self.initial_reverse_reduce_state(object, length),
        }
    }

    fn initial_forward_reduce_state(
        &mut self,
        object: &Value,
        length: usize,
    ) -> Result<Option<ReduceState>> {
        for index in 0..length {
            self.step()?;
            if self.has_array_like_index(object, index)? {
                let accumulator = self.get_array_like_index(object, index)?;
                let next = index
                    .checked_add(1)
                    .ok_or_else(|| Error::limit("array-like index exceeded supported range"))?;
                return Ok(Some(ReduceState::with_next(
                    accumulator,
                    next,
                    length,
                    true,
                )));
            }
        }
        Ok(None)
    }

    fn initial_reverse_reduce_state(
        &mut self,
        object: &Value,
        length: usize,
    ) -> Result<Option<ReduceState>> {
        for index in (0..length).rev() {
            self.step()?;
            if self.has_array_like_index(object, index)? {
                let accumulator = self.get_array_like_index(object, index)?;
                return Ok(Some(ReduceState::with_next(
                    accumulator,
                    index,
                    length,
                    true,
                )));
            }
        }
        Ok(None)
    }

    fn call_array_callback(
        &mut self,
        callback: &Value,
        callback_this: Value,
        value: &Value,
        index: usize,
        object: &Value,
    ) -> Result<Value> {
        let index = Self::array_like_index_value(index)?;
        let call_args = [value.clone(), index, object.clone()];
        match self.eval_call_completion(callback.clone(), &call_args, callback_this)? {
            Completion::Normal(value) => Ok(value),
            completion => completion.into_result(),
        }
    }

    fn call_reduce_callback(
        &mut self,
        callback: &Value,
        accumulator: Value,
        value: Value,
        index: usize,
        object: &Value,
    ) -> Result<Value> {
        let index = Self::array_like_index_value(index)?;
        let call_args = [accumulator, value, index, object.clone()];
        match self.eval_call_completion(callback.clone(), &call_args, Value::Undefined)? {
            Completion::Normal(value) => Ok(value),
            completion => completion.into_result(),
        }
    }

    fn create_array_callback_result(&mut self, length: usize) -> Result<Value> {
        let prototype = self.existing_array_constructor_prototype()?;
        self.objects
            .create_array_with_length(length, prototype, self.limits.max_objects)
    }

    fn push_array_callback_result(&mut self, result: &Value, value: Value) -> Result<()> {
        let Value::Object(id) = result else {
            return Err(Error::runtime("array callback result is not an object"));
        };
        let values = [value];
        self.objects
            .array_push(*id, &values, self.limits.max_object_properties)
            .map(|_| ())
    }

    fn array_like_length_for_callback(&mut self, this_value: &Value) -> Result<usize> {
        Self::ensure_array_like_object(this_value)?;
        self.array_like_length(this_value)
    }

    fn array_callback_and_this_arg(args: &[Value]) -> Result<(&Value, Value)> {
        let callback = Self::array_callback_arg(args)?;
        let callback_this = args.get(1).cloned().unwrap_or(Value::Undefined);
        Ok((callback, callback_this))
    }

    fn array_callback_arg(args: &[Value]) -> Result<&Value> {
        let Some(callback) = args.first() else {
            return Err(Error::type_error(ARRAY_CALLBACK_NOT_CALLABLE_ERROR));
        };
        if Self::is_callable(callback) {
            return Ok(callback);
        }
        Err(Error::type_error(ARRAY_CALLBACK_NOT_CALLABLE_ERROR))
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ArrayCallbackAction {
    Continue,
    Stop,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ReduceDirection {
    Forward,
    Reverse,
}

#[derive(Debug, Clone)]
struct ReduceState {
    accumulator: Value,
    next: usize,
    end: usize,
    started: bool,
}

impl ReduceState {
    const fn with_next(accumulator: Value, next: usize, end: usize, started: bool) -> Self {
        Self {
            accumulator,
            next,
            end,
            started,
        }
    }

    const fn next_index(&mut self, direction: ReduceDirection) -> Option<usize> {
        match direction {
            ReduceDirection::Forward => self.next_forward_index(),
            ReduceDirection::Reverse => self.next_reverse_index(),
        }
    }

    const fn next_forward_index(&mut self) -> Option<usize> {
        if self.next >= self.end {
            return None;
        }
        let index = self.next;
        self.next = self.next.saturating_add(1);
        Some(index)
    }

    const fn next_reverse_index(&mut self) -> Option<usize> {
        if self.next == 0 {
            return None;
        }
        self.next = self.next.saturating_sub(1);
        Some(self.next)
    }
}
