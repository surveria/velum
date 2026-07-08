use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        bytecode::for_of::ForOfStep,
        call::RuntimeCallArgs,
        control::Completion,
        object::{PropertyKey, PropertyLookup},
        property::get_property,
    },
    value::Value,
};

use super::NativeFunctionKind;

const ARRAY_FROM_NULLISH_ERROR: &str = "Array.from requires a non-null source";
const ARRAY_FROM_CALLBACK_ERROR: &str = "Array.from map function must be callable";
const ARRAY_FROM_INDEX_LIMIT_ERROR: &str = "Array.from index exceeded supported range";
const ARRAY_LENGTH_LIMIT_ERROR: &str = "array length exceeded supported range";
const ITERATOR_SYMBOL_DISPLAY_NAME: &str = "Symbol(Symbol.iterator)";
const LENGTH_PROPERTY: &str = "length";

impl Context {
    pub(in crate::runtime::native) fn eval_array_from(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_from(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_from(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let items = args.first().cloned().unwrap_or(Value::Undefined);
        if matches!(items, Value::Undefined | Value::Null) {
            return Err(Error::type_error(ARRAY_FROM_NULLISH_ERROR));
        }
        let map_fn = Self::array_from_map_function(args)?;
        let map_this = args.get(2).cloned().unwrap_or(Value::Undefined);
        if self.array_from_has_iterator(&items)? {
            return self.array_from_iterable(this_value, items, map_fn.as_ref(), &map_this);
        }
        self.array_from_array_like(this_value, &items, map_fn.as_ref(), &map_this)
    }

    pub(in crate::runtime::native) fn eval_array_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_of(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_of(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let result = self.array_result_object(this_value, args.len(), true)?;
        for (index, value) in args.iter().enumerate() {
            self.step()?;
            self.set_array_like_index(&result, index, value.clone())?;
        }
        self.set_array_result_length(&result, args.len())?;
        Ok(result)
    }

    pub(in crate::runtime::native::builtins::array) fn array_to_length_value(
        value: &Value,
    ) -> Result<usize> {
        let number = Self::array_length_number(value);
        if number.is_nan() || number <= 0.0 {
            return Ok(0);
        }
        let max = Self::max_array_length()?;
        if !number.is_finite() {
            return Ok(max);
        }
        let floored = number.floor().min(f64::from(u32::MAX));
        Self::array_from_nonnegative_integer_to_usize(floored).map(|value| value.min(max))
    }

    fn array_from_iterable(
        &mut self,
        this_value: &Value,
        items: Value,
        map_fn: Option<&Value>,
        map_this: &Value,
    ) -> Result<Value> {
        let result = self.array_result_object(this_value, 0, false)?;
        let mut source = self.for_of_source(items)?;
        let mut index = 0usize;
        loop {
            self.step()?;
            let value = match self.for_of_step(&mut source)? {
                ForOfStep::Value(value) => value,
                ForOfStep::Done => {
                    self.set_array_result_length(&result, index)?;
                    return Ok(result);
                }
                ForOfStep::Abrupt(completion) => return completion.into_result(),
            };
            let mapped = self.array_from_mapped_value(value, index, map_fn, map_this);
            let mapped = match mapped {
                Ok(value) => value,
                Err(error) => {
                    self.close_for_of_source(&source);
                    return Err(error);
                }
            };
            if let Err(error) = self.set_array_like_index(&result, index, mapped) {
                self.close_for_of_source(&source);
                return Err(error);
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::limit(ARRAY_FROM_INDEX_LIMIT_ERROR))?;
        }
    }

    fn array_from_array_like(
        &mut self,
        this_value: &Value,
        items: &Value,
        map_fn: Option<&Value>,
        map_this: &Value,
    ) -> Result<Value> {
        let length = self.array_like_source_length(items)?;
        let result = self.array_result_object(this_value, length, true)?;
        for index in 0..length {
            self.step()?;
            let value = self.get_array_like_source_index(items, index)?;
            let value = self.array_from_mapped_value(value, index, map_fn, map_this)?;
            self.set_array_like_index(&result, index, value)?;
        }
        self.set_array_result_length(&result, length)?;
        Ok(result)
    }

    fn array_from_mapped_value(
        &mut self,
        value: Value,
        index: usize,
        map_fn: Option<&Value>,
        map_this: &Value,
    ) -> Result<Value> {
        let Some(map_fn) = map_fn else {
            return Ok(value);
        };
        let index = Self::array_like_index_value(index)?;
        let args = [value, index];
        match self.eval_call_completion(map_fn.clone(), &args, map_this.clone())? {
            Completion::Normal(value) => Ok(value),
            completion => completion.into_result(),
        }
    }

    fn array_from_map_function(args: &[Value]) -> Result<Option<Value>> {
        let Some(value) = args.get(1) else {
            return Ok(None);
        };
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        if Self::is_callable(value) {
            return Ok(Some(value.clone()));
        }
        Err(Error::type_error(ARRAY_FROM_CALLBACK_ERROR))
    }

    fn array_from_has_iterator(&mut self, value: &Value) -> Result<bool> {
        match value {
            Value::String(_) | Value::HeapString(_) => Ok(true),
            Value::Object(id) => {
                if self.objects.array_len_if_array(*id)?.is_some()
                    || self.string_object_primitive_value(*id)?.is_some()
                {
                    return Ok(true);
                }
                self.array_from_object_has_iterator(value)
            }
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::Symbol(_)
            | Value::Error(_) => Ok(false),
        }
    }

    fn array_from_object_has_iterator(&mut self, value: &Value) -> Result<bool> {
        let Some(symbol) = self.iterator_symbol() else {
            return Ok(false);
        };
        let lookup =
            PropertyLookup::from_key(ITERATOR_SYMBOL_DISPLAY_NAME, PropertyKey::symbol(symbol));
        let method = get_property(&self.objects, value, lookup)?;
        let method = self.runtime_property_value(method)?;
        if matches!(method, Value::Undefined | Value::Null) {
            return Ok(false);
        }
        if Self::is_callable(&method) {
            return Ok(true);
        }
        Err(Error::type_error(
            "Array.from iterator method is not callable",
        ))
    }

    fn array_result_object(
        &mut self,
        constructor: &Value,
        length: usize,
        pass_length: bool,
    ) -> Result<Value> {
        if self.array_constructor_is_constructable(constructor)? {
            if pass_length {
                let length_value = Self::array_like_length_value(length)?;
                return self.eval_new_value(constructor.clone(), &[length_value]);
            }
            return self.eval_new_value(constructor.clone(), &[]);
        }
        let prototype = self.array_constructor_prototype()?;
        self.objects
            .create_array_with_length(length, prototype, self.limits.max_objects)
    }

    fn set_array_result_length(&mut self, result: &Value, length: usize) -> Result<()> {
        if let Value::Object(id) = result
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            return Ok(());
        }
        self.set_array_like_length(result, length)
    }

    fn array_constructor_is_constructable(&self, value: &Value) -> Result<bool> {
        match value {
            Value::Function(id) => Ok(self
                .functions
                .get(id.index())
                .is_some_and(|function| function.constructable)),
            Value::NativeFunction(id) => {
                let kind = self.native_function(*id)?.kind();
                Ok(matches!(
                    kind,
                    NativeFunctionKind::Array
                        | NativeFunctionKind::AsyncFunction
                        | NativeFunctionKind::Boolean
                        | NativeFunctionKind::ErrorConstructor(_)
                        | NativeFunctionKind::Function
                        | NativeFunctionKind::Map
                        | NativeFunctionKind::Number
                        | NativeFunctionKind::Object
                        | NativeFunctionKind::Promise
                        | NativeFunctionKind::RegExp
                        | NativeFunctionKind::Set
                        | NativeFunctionKind::String
                ))
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::HostFunction(_)
            | Value::Object(_)
            | Value::Error(_) => Ok(false),
        }
    }

    fn array_like_source_length(&mut self, value: &Value) -> Result<usize> {
        let length = match value {
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::Object(_)
            | Value::String(_)
            | Value::HeapString(_) => self.get_property_value(value, LENGTH_PROPERTY)?,
            Value::Undefined | Value::Null => {
                return Err(Error::type_error(ARRAY_FROM_NULLISH_ERROR));
            }
            Value::Bool(_)
            | Value::Number(_)
            | Value::Symbol(_)
            | Value::HostFunction(_)
            | Value::Error(_) => Value::Undefined,
        };
        Self::array_to_length_value(&length)
    }

    fn get_array_like_source_index(&mut self, value: &Value, index: usize) -> Result<Value> {
        match value {
            Value::Object(_) | Value::String(_) | Value::HeapString(_) => {
                self.get_array_like_index(value, index)
            }
            Value::Function(_) | Value::NativeFunction(_) => {
                let property = Self::array_index_name(index)?;
                self.get_property_value(value, &property)
            }
            Value::Undefined | Value::Null => Err(Error::type_error(ARRAY_FROM_NULLISH_ERROR)),
            Value::Bool(_)
            | Value::Number(_)
            | Value::Symbol(_)
            | Value::HostFunction(_)
            | Value::Error(_) => Ok(Value::Undefined),
        }
    }

    fn array_length_number(value: &Value) -> f64 {
        match value {
            Value::Undefined
            | Value::Null
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_)
            | Value::Error(_)
            | Value::Symbol(_) => 0.0,
            Value::Bool(value) => {
                if *value {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Number(value) => *value,
            Value::String(value) => string_to_number(value),
            Value::HeapString(value) => string_to_number(value.as_str()),
        }
    }

    fn array_index_name(index: usize) -> Result<String> {
        let max = usize::try_from(u32::MAX - 1)
            .map_err(|_| Error::limit(ARRAY_FROM_INDEX_LIMIT_ERROR))?;
        if index > max {
            return Err(Error::limit(ARRAY_FROM_INDEX_LIMIT_ERROR));
        }
        Ok(index.to_string())
    }

    fn max_array_length() -> Result<usize> {
        usize::try_from(u32::MAX).map_err(|_| Error::limit(ARRAY_LENGTH_LIMIT_ERROR))
    }

    fn array_from_nonnegative_integer_to_usize(value: f64) -> Result<usize> {
        if value == 0.0 {
            return Ok(0);
        }
        format!("{value:.0}")
            .parse::<usize>()
            .map_err(|_| Error::limit(ARRAY_LENGTH_LIMIT_ERROR))
    }
}

fn string_to_number(value: &str) -> f64 {
    value.trim().parse::<f64>().unwrap_or(0.0)
}
