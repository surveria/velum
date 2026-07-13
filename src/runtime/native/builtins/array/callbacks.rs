use crate::{
    error::{Error, Result},
    runtime::{
        Context, abstract_operations::to_boolean, call::RuntimeCallArgs, control::Completion,
        roots::VmRootKind,
    },
    value::Value,
};

use super::callback_state::{ArrayCallbackAction, ReduceDirection, ReduceState};

const ARRAY_CALLBACK_NOT_CALLABLE_ERROR: &str = "Array.prototype callback must be callable";
const ARRAY_FILTER_RESULT_LIMIT_ERROR: &str =
    "Array.prototype.filter result exceeded supported range";
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
        self.eval_direct_array_for_each(args.as_slice(), this_value, false)
    }

    pub(in crate::runtime::native) fn eval_direct_array_for_each(
        &mut self,
        args: &[Value],
        this_value: &Value,
        visit_every_index: bool,
    ) -> Result<Value> {
        let length = self.array_like_length_for_callback(this_value)?;
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        let mode = Self::array_callback_visit_mode(visit_every_index);
        self.visit_array_like_with_length(this_value, length, mode, |context, index, value| {
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
        self.eval_direct_array_map(args.as_slice(), this_value, false)
    }

    pub(in crate::runtime::native) fn eval_direct_array_map(
        &mut self,
        args: &[Value],
        this_value: &Value,
        visit_every_index: bool,
    ) -> Result<Value> {
        let length = self.array_like_length_for_callback(this_value)?;
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        let result = self.array_species_create(this_value, length)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        let mode = Self::array_callback_visit_mode(visit_every_index);
        self.visit_array_like_with_length(this_value, length, mode, |context, index, value| {
            let mapped = context.call_array_callback(
                callback,
                callback_this.clone(),
                value,
                index,
                this_value,
            )?;
            context.array_from_create_data_property(&result, index, mapped)?;
            Ok(ArrayCallbackAction::Continue)
        })?;
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_array_filter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_filter(args.as_slice(), this_value, false)
    }

    pub(in crate::runtime::native) fn eval_direct_array_filter(
        &mut self,
        args: &[Value],
        this_value: &Value,
        visit_every_index: bool,
    ) -> Result<Value> {
        let length = self.array_like_length_for_callback(this_value)?;
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        let result = self.array_species_create(this_value, 0)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        let mut next_index = 0_usize;
        let mode = Self::array_callback_visit_mode(visit_every_index);
        self.visit_array_like_with_length(this_value, length, mode, |context, index, value| {
            let keep = context.call_array_callback(
                callback,
                callback_this.clone(),
                value,
                index,
                this_value,
            )?;
            if to_boolean(&keep) {
                context.array_from_create_data_property(&result, next_index, value.clone())?;
                next_index = next_index
                    .checked_add(1)
                    .ok_or_else(|| Error::limit(ARRAY_FILTER_RESULT_LIMIT_ERROR))?;
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
        self.eval_direct_array_some(args.as_slice(), this_value, false)
    }

    pub(in crate::runtime::native) fn eval_direct_array_some(
        &mut self,
        args: &[Value],
        this_value: &Value,
        visit_every_index: bool,
    ) -> Result<Value> {
        let length = self.array_like_length_for_callback(this_value)?;
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        if let Some(value) = self.eval_packed_numeric_array_some(callback, this_value)? {
            return Ok(value);
        }
        let mut matched = false;
        let mode = Self::array_callback_visit_mode(visit_every_index);
        self.visit_array_like_with_length(this_value, length, mode, |context, index, value| {
            let result = context.call_array_callback(
                callback,
                callback_this.clone(),
                value,
                index,
                this_value,
            )?;
            if to_boolean(&result) {
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
        self.eval_direct_array_every(args.as_slice(), this_value, false)
    }

    pub(in crate::runtime::native) fn eval_direct_array_every(
        &mut self,
        args: &[Value],
        this_value: &Value,
        visit_every_index: bool,
    ) -> Result<Value> {
        let length = self.array_like_length_for_callback(this_value)?;
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        if let Some(value) = self.eval_packed_numeric_array_every(callback, this_value)? {
            return Ok(value);
        }
        let mut matched = true;
        let mode = Self::array_callback_visit_mode(visit_every_index);
        self.visit_array_like_with_length(this_value, length, mode, |context, index, value| {
            let result = context.call_array_callback(
                callback,
                callback_this.clone(),
                value,
                index,
                this_value,
            )?;
            if !to_boolean(&result) {
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
        let length = self.array_like_length_for_callback(this_value)?;
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        if let Some(value) = self.eval_packed_numeric_array_find(callback, this_value)? {
            return Ok(value);
        }
        let mut found = Value::Undefined;
        self.visit_array_like_with_length(
            this_value,
            length,
            CallbackVisitMode::EveryIndex,
            |context, index, value| {
                let result = context.call_array_callback(
                    callback,
                    callback_this.clone(),
                    value,
                    index,
                    this_value,
                )?;
                if to_boolean(&result) {
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
        let length = self.array_like_length_for_callback(this_value)?;
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        if let Some(value) = self.eval_packed_numeric_array_find_index(callback, this_value)? {
            return Ok(value);
        }
        let mut found = Value::Number(INDEX_NOT_FOUND);
        self.visit_array_like_with_length(
            this_value,
            length,
            CallbackVisitMode::EveryIndex,
            |context, index, value| {
                let result = context.call_array_callback(
                    callback,
                    callback_this.clone(),
                    value,
                    index,
                    this_value,
                )?;
                if to_boolean(&result) {
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
        self.eval_direct_array_reduce_with_direction(
            args,
            this_value,
            ReduceDirection::Forward,
            false,
        )
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
        self.eval_direct_array_reduce_with_direction(
            args,
            this_value,
            ReduceDirection::Reverse,
            false,
        )
    }

    fn eval_packed_numeric_array_some(
        &mut self,
        callback: &Value,
        this_value: &Value,
    ) -> Result<Option<Value>> {
        let Some(values) = self.packed_numeric_array_values(this_value)? else {
            return Ok(None);
        };
        for (index, value) in values.iter().enumerate() {
            self.step()?;
            let args = Self::array_callback_args(value, index, this_value)?;
            let Some(result) = self.eval_pure_function_callback_fast_path(callback, &args)? else {
                return Ok(None);
            };
            if to_boolean(&result) {
                return Ok(Some(Value::Bool(true)));
            }
        }
        Ok(Some(Value::Bool(false)))
    }

    fn eval_packed_numeric_array_every(
        &mut self,
        callback: &Value,
        this_value: &Value,
    ) -> Result<Option<Value>> {
        let Some(values) = self.packed_numeric_array_values(this_value)? else {
            return Ok(None);
        };
        for (index, value) in values.iter().enumerate() {
            self.step()?;
            let args = Self::array_callback_args(value, index, this_value)?;
            let Some(result) = self.eval_pure_function_callback_fast_path(callback, &args)? else {
                return Ok(None);
            };
            if !to_boolean(&result) {
                return Ok(Some(Value::Bool(false)));
            }
        }
        Ok(Some(Value::Bool(true)))
    }

    fn eval_packed_numeric_array_find(
        &mut self,
        callback: &Value,
        this_value: &Value,
    ) -> Result<Option<Value>> {
        let Some(values) = self.packed_numeric_array_values(this_value)? else {
            return Ok(None);
        };
        for (index, value) in values.iter().enumerate() {
            self.step()?;
            let args = Self::array_callback_args(value, index, this_value)?;
            let Some(result) = self.eval_pure_function_callback_fast_path(callback, &args)? else {
                return Ok(None);
            };
            if to_boolean(&result) {
                return Ok(Some(value.clone()));
            }
        }
        Ok(Some(Value::Undefined))
    }

    fn eval_packed_numeric_array_find_index(
        &mut self,
        callback: &Value,
        this_value: &Value,
    ) -> Result<Option<Value>> {
        let Some(values) = self.packed_numeric_array_values(this_value)? else {
            return Ok(None);
        };
        for (index, value) in values.iter().enumerate() {
            self.step()?;
            let args = Self::array_callback_args(value, index, this_value)?;
            let Some(result) = self.eval_pure_function_callback_fast_path(callback, &args)? else {
                return Ok(None);
            };
            if to_boolean(&result) {
                return Self::array_like_index_value(index).map(Some);
            }
        }
        Ok(Some(Value::Number(INDEX_NOT_FOUND)))
    }

    fn eval_packed_numeric_array_reduce(
        &mut self,
        callback: &Value,
        this_value: &Value,
        direction: ReduceDirection,
        initial: Option<Value>,
    ) -> Result<Option<Value>> {
        let Some(values) = self.packed_numeric_array_values(this_value)? else {
            return Ok(None);
        };
        let Some(mut accumulator) = initial else {
            return Ok(None);
        };
        if !matches!(accumulator, Value::Number(_)) {
            return Ok(None);
        }
        let iter: Box<dyn Iterator<Item = (usize, &Value)> + '_> = match direction {
            ReduceDirection::Forward => Box::new(values.iter().enumerate()),
            ReduceDirection::Reverse => Box::new(values.iter().enumerate().rev()),
        };
        for (index, value) in iter {
            self.step()?;
            let args = Self::reduce_callback_args(&accumulator, value, index, this_value)?;
            let Some(value) = self.eval_pure_function_callback_fast_path(callback, &args)? else {
                return Ok(None);
            };
            accumulator = value;
        }
        Ok(Some(accumulator))
    }

    fn packed_numeric_array_values(&self, object: &Value) -> Result<Option<Vec<Value>>> {
        let Value::Object(id) = object else {
            return Ok(None);
        };
        let Some(values) = self.objects.packed_array_values_if_array(*id)? else {
            return Ok(None);
        };
        if values.iter().all(|value| matches!(value, Value::Number(_))) {
            return Ok(Some(values));
        }
        Ok(None)
    }

    fn array_callback_args(value: &Value, index: usize, object: &Value) -> Result<[Value; 3]> {
        Ok([
            value.clone(),
            Self::array_like_index_value(index)?,
            object.clone(),
        ])
    }

    fn reduce_callback_args(
        accumulator: &Value,
        value: &Value,
        index: usize,
        object: &Value,
    ) -> Result<[Value; 4]> {
        Ok([
            accumulator.clone(),
            value.clone(),
            Self::array_like_index_value(index)?,
            object.clone(),
        ])
    }

    fn visit_array_like_with_length<F>(
        &mut self,
        object: &Value,
        length: usize,
        mode: CallbackVisitMode,
        mut visitor: F,
    ) -> Result<()>
    where
        F: FnMut(&mut Self, usize, &Value) -> Result<ArrayCallbackAction>,
    {
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

    const fn array_callback_visit_mode(visit_every_index: bool) -> CallbackVisitMode {
        if visit_every_index {
            CallbackVisitMode::EveryIndex
        } else {
            CallbackVisitMode::PresentOnly
        }
    }

    fn eval_direct_array_reduce_with_direction(
        &mut self,
        args: &[Value],
        this_value: &Value,
        direction: ReduceDirection,
        visit_out_of_bounds: bool,
    ) -> Result<Value> {
        let length = self.array_like_length_for_callback(this_value)?;
        let callback = self.array_callback_arg(args)?;
        let has_initial = args.get(1).is_some();
        let initial = args.get(1).cloned();
        if !visit_out_of_bounds
            && let Some(value) = self.eval_packed_numeric_array_reduce(
                callback,
                this_value,
                direction,
                initial.clone(),
            )?
        {
            return Ok(value);
        }
        let Some(mut state) =
            self.initial_reduce_state(this_value, length, direction, initial, visit_out_of_bounds)?
        else {
            return Err(Error::type_error(ARRAY_REDUCE_EMPTY_ERROR));
        };

        while let Some(index) = state.next_index(direction) {
            self.step()?;
            if visit_out_of_bounds || self.has_array_like_index(this_value, index)? {
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
        visit_out_of_bounds: bool,
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
        if visit_out_of_bounds && length > 0 {
            let index = match direction {
                ReduceDirection::Forward => 0,
                ReduceDirection::Reverse => length.saturating_sub(1),
            };
            let accumulator = self.get_array_like_index(object, index)?;
            let next = match direction {
                ReduceDirection::Forward => 1,
                ReduceDirection::Reverse => index,
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

    pub(super) fn call_array_callback(
        &mut self,
        callback: &Value,
        callback_this: Value,
        value: &Value,
        index: usize,
        object: &Value,
    ) -> Result<Value> {
        let index = Self::array_like_index_value(index)?;
        let call_args = [value.clone(), index, object.clone()];
        match self.call(callback, &call_args, callback_this)? {
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
        match self.call(callback, &call_args, Value::Undefined)? {
            Completion::Normal(value) => Ok(value),
            completion => completion.into_result(),
        }
    }

    fn array_like_length_for_callback(&mut self, this_value: &Value) -> Result<usize> {
        Self::ensure_array_like_object(this_value)?;
        self.array_like_length(this_value)
    }

    pub(super) fn array_callback_and_this_arg<'args>(
        &self,
        args: &'args [Value],
    ) -> Result<(&'args Value, Value)> {
        let callback = self.array_callback_arg(args)?;
        let callback_this = args.get(1).cloned().unwrap_or(Value::Undefined);
        Ok((callback, callback_this))
    }

    fn array_callback_arg<'args>(&self, args: &'args [Value]) -> Result<&'args Value> {
        let Some(callback) = args.first() else {
            return Err(Error::type_error(ARRAY_CALLBACK_NOT_CALLABLE_ERROR));
        };
        if self.semantic_is_callable(callback)? {
            return Ok(callback);
        }
        Err(Error::type_error(ARRAY_CALLBACK_NOT_CALLABLE_ERROR))
    }
}
