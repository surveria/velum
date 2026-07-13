use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{same_value_zero, strict_equality, to_boolean},
        native::TypedArrayFunctionKind,
    },
    value::Value,
};

const CALLBACK_NOT_CALLABLE_ERROR: &str = "TypedArray.prototype callback must be callable";
const REDUCE_EMPTY_ERROR: &str = "Reduce of empty typed array with no initial value";
const INDEX_NOT_FOUND: f64 = -1.0;

impl Context {
    pub(super) fn eval_typed_array_iteration_kind(
        &mut self,
        kind: TypedArrayFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            TypedArrayFunctionKind::Filter | TypedArrayFunctionKind::Map => {
                Some(self.eval_typed_array_map_filter(kind, args, this_value))
            }
            TypedArrayFunctionKind::Every
            | TypedArrayFunctionKind::Find
            | TypedArrayFunctionKind::FindIndex
            | TypedArrayFunctionKind::FindLast
            | TypedArrayFunctionKind::FindLastIndex
            | TypedArrayFunctionKind::ForEach
            | TypedArrayFunctionKind::Some => {
                Some(self.eval_typed_array_callback_iteration(kind, args, this_value))
            }
            TypedArrayFunctionKind::Includes
            | TypedArrayFunctionKind::IndexOf
            | TypedArrayFunctionKind::LastIndexOf => {
                Some(self.eval_typed_array_search(kind, args, this_value))
            }
            TypedArrayFunctionKind::Reduce | TypedArrayFunctionKind::ReduceRight => {
                Some(self.eval_typed_array_reduce(kind, args, this_value))
            }
            _ => None,
        }
    }

    fn typed_array_callback<'args>(&self, args: &'args [Value]) -> Result<(&'args Value, Value)> {
        let Some(callback) = args.first() else {
            return Err(Error::type_error(CALLBACK_NOT_CALLABLE_ERROR));
        };
        if !self.semantic_is_callable(callback)? {
            return Err(Error::type_error(CALLBACK_NOT_CALLABLE_ERROR));
        }
        let callback_this = args.get(1).cloned().unwrap_or(Value::Undefined);
        Ok((callback, callback_this))
    }

    fn call_typed_array_callback(
        &mut self,
        callback: &Value,
        callback_this: &Value,
        value: &Value,
        index: usize,
        this_value: &Value,
    ) -> Result<Value> {
        let call_args = [
            value.clone(),
            Self::typed_array_usize_value(index)?,
            this_value.clone(),
        ];
        self.call_value(callback, &call_args, callback_this.clone())
    }

    fn eval_typed_array_callback_iteration(
        &mut self,
        kind: TypedArrayFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let record = self.typed_array_view_record(this_value)?;
        let (callback, callback_this) = self.typed_array_callback(args)?;
        let reverse = matches!(
            kind,
            TypedArrayFunctionKind::FindLast | TypedArrayFunctionKind::FindLastIndex
        );
        let mut indices: Box<dyn Iterator<Item = usize>> = if reverse {
            Box::new((0..record.length).rev())
        } else {
            Box::new(0..record.length)
        };
        for index in &mut indices {
            self.step()?;
            let value = record.value(index)?;
            let selected = self.call_typed_array_callback(
                callback,
                &callback_this,
                &value,
                index,
                this_value,
            )?;
            match kind {
                TypedArrayFunctionKind::Every if !to_boolean(&selected) => {
                    return Ok(Value::Bool(false));
                }
                TypedArrayFunctionKind::Some if to_boolean(&selected) => {
                    return Ok(Value::Bool(true));
                }
                TypedArrayFunctionKind::Find | TypedArrayFunctionKind::FindLast
                    if to_boolean(&selected) =>
                {
                    return Ok(value);
                }
                TypedArrayFunctionKind::FindIndex | TypedArrayFunctionKind::FindLastIndex
                    if to_boolean(&selected) =>
                {
                    return Self::typed_array_usize_value(index);
                }
                TypedArrayFunctionKind::Every
                | TypedArrayFunctionKind::Find
                | TypedArrayFunctionKind::FindIndex
                | TypedArrayFunctionKind::FindLast
                | TypedArrayFunctionKind::FindLastIndex
                | TypedArrayFunctionKind::ForEach
                | TypedArrayFunctionKind::Some => {}
                _ => {
                    return Err(Error::runtime(
                        "typed array callback iteration was routed incorrectly",
                    ));
                }
            }
        }
        Ok(match kind {
            TypedArrayFunctionKind::Every => Value::Bool(true),
            TypedArrayFunctionKind::Some => Value::Bool(false),
            TypedArrayFunctionKind::Find
            | TypedArrayFunctionKind::FindLast
            | TypedArrayFunctionKind::ForEach => Value::Undefined,
            TypedArrayFunctionKind::FindIndex | TypedArrayFunctionKind::FindLastIndex => {
                Value::Number(INDEX_NOT_FOUND)
            }
            _ => {
                return Err(Error::runtime(
                    "typed array callback result was routed incorrectly",
                ));
            }
        })
    }

    fn eval_typed_array_search(
        &mut self,
        kind: TypedArrayFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let record = self.typed_array_view_record(this_value)?;
        let search = args.first().unwrap_or(&Value::Undefined);
        if record.length == 0 {
            return Ok(Self::typed_array_search_miss(kind));
        }
        let from_index = args.get(1);
        match kind {
            TypedArrayFunctionKind::Includes | TypedArrayFunctionKind::IndexOf => {
                let start = self.typed_array_relative_index(from_index, record.length, 0)?;
                for index in start..record.length {
                    self.step()?;
                    let value = record.read(index)?;
                    let matched = match kind {
                        TypedArrayFunctionKind::Includes => {
                            same_value_zero(value.as_ref().unwrap_or(&Value::Undefined), search)
                        }
                        TypedArrayFunctionKind::IndexOf => value
                            .as_ref()
                            .is_some_and(|value| strict_equality(value, search)),
                        _ => false,
                    };
                    if matched {
                        if matches!(kind, TypedArrayFunctionKind::Includes) {
                            return Ok(Value::Bool(true));
                        }
                        return Self::typed_array_usize_value(index);
                    }
                }
            }
            TypedArrayFunctionKind::LastIndexOf => {
                let start = self.typed_array_last_index(from_index, record.length)?;
                let Some(start) = start else {
                    return Ok(Value::Number(INDEX_NOT_FOUND));
                };
                for index in (0..=start).rev() {
                    self.step()?;
                    if record
                        .read(index)?
                        .as_ref()
                        .is_some_and(|value| strict_equality(value, search))
                    {
                        return Self::typed_array_usize_value(index);
                    }
                }
            }
            _ => {
                return Err(Error::runtime("typed array search was routed incorrectly"));
            }
        }
        Ok(Self::typed_array_search_miss(kind))
    }

    const fn typed_array_search_miss(kind: TypedArrayFunctionKind) -> Value {
        if matches!(kind, TypedArrayFunctionKind::Includes) {
            Value::Bool(false)
        } else {
            Value::Number(INDEX_NOT_FOUND)
        }
    }

    fn typed_array_last_index(
        &mut self,
        from_index: Option<&Value>,
        length: usize,
    ) -> Result<Option<usize>> {
        let Some(from_index) = from_index else {
            return Ok(length.checked_sub(1));
        };
        let relative = self.to_integer_or_infinity(from_index)?;
        if relative == f64::NEG_INFINITY {
            return Ok(None);
        }
        if relative >= 0.0 {
            let maximum = length.saturating_sub(1);
            return Self::finite_nonnegative_integer_to_usize(
                relative.min(Self::typed_array_usize_number(maximum)?),
                super::methods::TYPED_ARRAY_LENGTH_ERROR,
            )
            .map(Some);
        }
        let candidate = Self::typed_array_usize_number(length)? + relative;
        if candidate < 0.0 {
            return Ok(None);
        }
        Self::finite_nonnegative_integer_to_usize(
            candidate,
            super::methods::TYPED_ARRAY_LENGTH_ERROR,
        )
        .map(Some)
    }

    fn eval_typed_array_reduce(
        &mut self,
        kind: TypedArrayFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let record = self.typed_array_view_record(this_value)?;
        let Some(callback) = args.first() else {
            return Err(Error::type_error(CALLBACK_NOT_CALLABLE_ERROR));
        };
        if !self.semantic_is_callable(callback)? {
            return Err(Error::type_error(CALLBACK_NOT_CALLABLE_ERROR));
        }
        let reverse = matches!(kind, TypedArrayFunctionKind::ReduceRight);
        let mut indices: Box<dyn Iterator<Item = usize>> = if reverse {
            Box::new((0..record.length).rev())
        } else {
            Box::new(0..record.length)
        };
        let mut accumulator = if let Some(initial) = args.get(1) {
            initial.clone()
        } else {
            let Some(index) = indices.next() else {
                return Err(Error::type_error(REDUCE_EMPTY_ERROR));
            };
            record.value(index)?
        };
        for index in indices {
            self.step()?;
            let value = record.value(index)?;
            let call_args = [
                accumulator,
                value,
                Self::typed_array_usize_value(index)?,
                this_value.clone(),
            ];
            accumulator = self.call_value(callback, &call_args, Value::Undefined)?;
        }
        Ok(accumulator)
    }

    fn eval_typed_array_map_filter(
        &mut self,
        kind: TypedArrayFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let record = self.typed_array_view_record(this_value)?;
        let (callback, callback_this) = self.typed_array_callback(args)?;
        if matches!(kind, TypedArrayFunctionKind::Filter) {
            let mut selected = Vec::new();
            for index in 0..record.length {
                self.step()?;
                let value = record.value(index)?;
                let keep = self.call_typed_array_callback(
                    callback,
                    &callback_this,
                    &value,
                    index,
                    this_value,
                )?;
                if to_boolean(&keep) {
                    selected.push(value);
                }
            }
            return self.typed_array_species_create_from_values(this_value, selected);
        }

        let (result, result_id, result_view) =
            self.typed_array_species_create_with_length(this_value, record.length)?;
        result_view.ensure_mutable()?;
        let _result_scope = self.transient_root_scope(
            crate::runtime::roots::VmRootKind::TransientTemporary,
            std::iter::once(&result),
        )?;
        for index in 0..record.length {
            self.step()?;
            let value = record.value(index)?;
            let mapped = self.call_typed_array_callback(
                callback,
                &callback_this,
                &value,
                index,
                this_value,
            )?;
            let element =
                self.convert_typed_array_element_value(result_view.element_kind(), &mapped)?;
            if !self
                .objects
                .set_typed_array_value(result_id, index, &element)?
            {
                return Err(Error::type_error(
                    "TypedArray map result became out of bounds",
                ));
            }
        }
        Ok(result)
    }
}
