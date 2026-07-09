use std::fmt::Write as _;

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, object::PropertyEnumerable},
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{ARRAY_NAME, NativeFunctionKind};

mod callbacks;
mod copy;
mod find_last;
mod flatten;
mod generic;
mod mutate;
mod prototype_registry;
mod sort;

const ARRAY_JOIN_DEFAULT_SEPARATOR: &str = ",";
const ARRAY_IS_ARRAY_PROPERTY: &str = "isArray";

impl Context {
    pub(in crate::runtime::native) fn array_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Array) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.array_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::Array)?;
        self.push_native_function_with_id(id, NativeFunctionKind::Array, prototype, name)?;
        self.install_array_static_methods(id)?;
        self.install_array_prototype_methods(prototype_id)?;
        self.insert_global_builtin(ARRAY_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_array_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let args = args.as_slice();
        if let Some(length) = Self::array_constructor_length(args)? {
            let prototype = self.array_constructor_prototype()?;
            return self.objects.create_array_with_length(
                length,
                prototype,
                self.limits.max_objects,
            );
        }
        self.create_array_from_element_iter(args.iter().cloned(), args.len())
    }

    pub(in crate::runtime::native) fn eval_direct_array_constructor(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        if let Some(length) = Self::array_constructor_length(args)? {
            let prototype = self.array_constructor_prototype()?;
            return self.objects.create_array_with_length(
                length,
                prototype,
                self.limits.max_objects,
            );
        }
        self.create_array_from_element_iter(args.iter().cloned(), args.len())
    }

    pub(in crate::runtime::native) fn eval_array_is_array(
        &self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_array_is_array(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_array_is_array(
        &self,
        args: &[Value],
    ) -> Result<Value> {
        let is_array = match args.first() {
            Some(Value::Object(id)) => self.objects.array_len_if_array(*id)?.is_some(),
            Some(
                Value::Undefined
                | Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::String(_)
                | Value::HeapString(_)
                | Value::Symbol(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
                | Value::Error(_),
            )
            | None => false,
        };
        Ok(Value::Bool(is_array))
    }

    pub(in crate::runtime::native) fn eval_array_push(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_push(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_push(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            return self
                .objects
                .array_push(*id, args, self.limits.max_object_properties);
        }
        self.generic_array_push(args, this_value)
    }

    pub(in crate::runtime::native) fn eval_array_concat(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_concat(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_concat(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let Value::Object(id) = this_value else {
            return Err(Error::runtime(
                "Array.prototype.concat requires an array receiver",
            ));
        };
        let prototype = self.existing_array_constructor_prototype()?;
        self.objects.array_concat(
            *id,
            args,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime::native) fn eval_array_reverse(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_reverse(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_reverse(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::eval_array_discard_args(args);
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            return self
                .objects
                .array_reverse(*id, self.limits.max_object_properties);
        }
        self.generic_array_reverse(this_value)
    }

    pub(in crate::runtime::native) fn eval_array_pop(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_pop(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_pop(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::eval_array_discard_args(args);
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            return self.objects.array_pop(*id);
        }
        self.generic_array_pop(this_value)
    }

    pub(in crate::runtime::native) fn eval_array_includes(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_includes(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_includes(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (search, from_index) = Self::eval_array_binary_values(args);
        let default_search = Value::Undefined;
        let search = search.unwrap_or(&default_search);
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            let length = self.objects.array_len_for_includes(*id)?;
            let from_index = Self::array_slice_bound(from_index, length, 0)?;
            return self.objects.array_includes(*id, length, search, from_index);
        }
        self.generic_array_includes(search, from_index, this_value)
    }

    pub(in crate::runtime::native) fn eval_array_index_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_index_of(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_index_of(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (search, from_index) = Self::eval_array_binary_values(args);
        let default_search = Value::Undefined;
        let search = search.unwrap_or(&default_search);
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            let length = self.objects.array_len_for_index_of(*id)?;
            let from_index = Self::array_slice_bound(from_index, length, 0)?;
            return self.objects.array_index_of(*id, length, search, from_index);
        }
        self.generic_array_index_of(search, from_index, this_value)
    }

    pub(in crate::runtime::native) fn eval_array_last_index_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_last_index_of(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_last_index_of(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (search, from_index) = Self::eval_array_binary_values(args);
        let default_search = Value::Undefined;
        let search = search.unwrap_or(&default_search);
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            let length = self.objects.array_len_for_last_index_of(*id)?;
            let from_index = Self::array_last_index_of_start(from_index, length)?;
            return self
                .objects
                .array_last_index_of(*id, length, search, from_index);
        }
        self.generic_array_last_index_of(search, from_index, this_value)
    }

    pub(in crate::runtime::native) fn eval_array_join(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_join(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_join(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let separator = Self::eval_array_unary_value(args);
        let separator = Self::array_join_separator(separator);
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            if let Some(joined) =
                self.objects
                    .packed_array_join(*id, &separator, self.limits.max_string_len)?
            {
                return self.heap_string_value(&joined);
            }

            let length = self.objects.array_len(*id)?;
            let mut joined = self.join_string_with_separator_capacity(length, separator.len())?;
            for index in 0..length {
                if index > 0 {
                    self.push_join_text(&mut joined, &separator)?;
                }
                let value = self.objects.array_get_index(*id, index)?;
                self.push_join_value_text(&mut joined, &value)?;
            }
            return self.heap_string_value(&joined);
        }
        self.generic_array_join(&separator, this_value)
    }

    pub(in crate::runtime::native) fn eval_array_shift(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_shift(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_shift(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::eval_array_discard_args(args);
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            return self
                .objects
                .array_shift(*id, self.limits.max_object_properties);
        }
        self.generic_array_shift(this_value)
    }

    pub(in crate::runtime::native) fn eval_array_slice(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_slice(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_slice(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let (start, end) = Self::eval_array_binary_values(args);
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            let length = self.objects.array_len_for_slice(*id)?;
            let start = Self::array_slice_bound(start, length, 0)?;
            let end = Self::array_slice_bound(end, length, length)?.max(start);
            let prototype = self.existing_array_constructor_prototype()?;
            return self.objects.array_slice(
                *id,
                length,
                start..end,
                prototype,
                self.limits.max_objects,
                self.limits.max_object_properties,
            );
        }
        self.generic_array_slice(start, end, this_value)
    }

    pub(in crate::runtime::native) fn eval_array_unshift(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_unshift(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_unshift(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        if let Value::Object(id) = this_value
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            return self
                .objects
                .array_unshift(*id, args, self.limits.max_object_properties);
        }
        self.generic_array_unshift(args, this_value)
    }

    pub(crate) fn create_array_from_elements(&mut self, elements: Vec<Value>) -> Result<Value> {
        let element_count = elements.len();
        self.create_array_from_element_iter(elements, element_count)
    }

    pub(crate) fn create_array_from_element_iter(
        &mut self,
        elements: impl IntoIterator<Item = Value>,
        element_count: usize,
    ) -> Result<Value> {
        let prototype = self.array_constructor_prototype()?;
        self.objects.create_array_from_iter(
            elements,
            element_count,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    pub(crate) fn create_array_literal_from_elements(
        &mut self,
        elements: impl IntoIterator<Item = Value>,
        element_count: usize,
        holes: &[bool],
    ) -> Result<Value> {
        if holes.len() != element_count {
            return Err(Error::runtime(
                "array literal hole metadata length mismatch",
            ));
        }
        if holes.iter().all(|hole| !*hole) {
            return self.create_array_from_element_iter(elements, element_count);
        }

        let prototype = self.array_constructor_prototype()?;
        let array = self.objects.create_array_with_length(
            element_count,
            prototype,
            self.limits.max_objects,
        )?;
        let mut values = elements.into_iter();
        for (index, hole) in holes.iter().copied().enumerate() {
            if hole {
                continue;
            }
            let Some(value) = values.next() else {
                return Err(Error::runtime("array literal value metadata mismatch"));
            };
            let property_name = index.to_string();
            let key = self.intern_property_key(&property_name)?;
            crate::runtime::property::set_property(
                &mut self.objects,
                &array,
                key,
                &property_name,
                value,
                self.limits.max_object_properties,
            )?;
        }
        if values.next().is_some() {
            return Err(Error::runtime("array literal value metadata mismatch"));
        }
        Ok(array)
    }

    pub(crate) fn create_array_literal_from_options(
        &mut self,
        elements: Vec<Option<Value>>,
    ) -> Result<Value> {
        let element_count = elements.len();
        let prototype = self.array_constructor_prototype()?;
        let array = self.objects.create_array_with_length(
            element_count,
            prototype,
            self.limits.max_objects,
        )?;
        for (index, value) in elements.into_iter().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let property_name = index.to_string();
            let key = self.intern_property_key(&property_name)?;
            crate::runtime::property::set_property(
                &mut self.objects,
                &array,
                key,
                &property_name,
                value,
                self.limits.max_object_properties,
            )?;
        }
        Ok(array)
    }

    fn array_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.array_prototype_id_with_constructor(
            constructor,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn install_array_static_methods(&mut self, constructor: NativeFunctionId) -> Result<()> {
        let is_array =
            self.create_native_function(NativeFunctionKind::ArrayIsArray, Value::Undefined)?;
        let key = self.intern_property_key(ARRAY_IS_ARRAY_PROPERTY)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, is_array, PropertyEnumerable::No);
        Ok(())
    }

    fn array_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.array_constructor_value()? else {
            return Err(Error::runtime("Array constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("Array prototype is not an object")),
        }
    }

    pub(super) fn existing_array_constructor_prototype(&self) -> Result<ObjectId> {
        self.objects.existing_array_prototype_id()
    }

    fn array_constructor_length(values: &[Value]) -> Result<Option<usize>> {
        let Some(Value::Number(value)) = values.first() else {
            return Ok(None);
        };
        if values.len() != 1 {
            return Ok(None);
        }
        Self::array_length_from_number(*value).map(Some)
    }

    fn array_length_from_number(value: f64) -> Result<usize> {
        if value == 0.0 {
            return Ok(0);
        }
        if !value.is_finite() || value.is_sign_negative() || value.fract() != 0.0 {
            return Err(Error::runtime("invalid array length"));
        }
        format!("{value:.0}")
            .parse::<usize>()
            .map_err(|_| Error::limit("array length exceeded supported range"))
    }

    fn array_join_separator(value: Option<&Value>) -> String {
        match value {
            None | Some(Value::Undefined) => ARRAY_JOIN_DEFAULT_SEPARATOR.to_owned(),
            Some(value) => value.display_for_concat(),
        }
    }

    const fn eval_array_unary_value(args: &[Value]) -> Option<&Value> {
        args.first()
    }

    fn eval_array_binary_values(args: &[Value]) -> (Option<&Value>, Option<&Value>) {
        (args.first(), args.get(1))
    }

    const fn eval_array_discard_args(_args: &[Value]) {}

    fn push_join_value_text(&self, joined: &mut String, value: &Value) -> Result<()> {
        match value {
            Value::Undefined | Value::Null => Ok(()),
            Value::String(value) => self.push_join_text(joined, value),
            Value::HeapString(value) => self.push_join_text(joined, value.as_str()),
            _ => self.write_join_display(joined, value),
        }
    }

    fn push_join_text(&self, joined: &mut String, text: &str) -> Result<()> {
        let length = joined
            .len()
            .checked_add(text.len())
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
        if length > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {} exceeded {}",
                length, self.limits.max_string_len
            )));
        }
        joined.push_str(text);
        Ok(())
    }

    fn write_join_display(&self, joined: &mut String, value: &Value) -> Result<()> {
        joined.write_fmt(format_args!("{value}")).map_err(|error| {
            Error::runtime(format!("failed to format array join value: {error}"))
        })?;
        if joined.len() > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {} exceeded {}",
                joined.len(),
                self.limits.max_string_len
            )));
        }
        Ok(())
    }

    fn join_string_with_separator_capacity(
        &self,
        length: usize,
        separator_len: usize,
    ) -> Result<String> {
        let separator_count = length.saturating_sub(1);
        let separator_bytes = separator_count
            .checked_mul(separator_len)
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
        if separator_bytes > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {} exceeded {}",
                separator_bytes, self.limits.max_string_len
            )));
        }
        Ok(String::with_capacity(separator_bytes))
    }

    fn array_slice_bound(value: Option<&Value>, length: usize, default: usize) -> Result<usize> {
        let Some(value) = value else {
            return Ok(default);
        };
        if matches!(value, Value::Undefined) {
            return Ok(default);
        }

        let number = Self::array_slice_bound_number(value);
        Self::array_slice_bound_from_number(number, length)
    }

    fn array_slice_bound_number(value: &Value) -> f64 {
        match value {
            Value::Undefined
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_)
            | Value::Error(_)
            | Value::Symbol(_)
            | Value::Null => 0.0,
            Value::Bool(value) => {
                if *value {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Number(value) => *value,
            Value::String(value) => value.trim().parse::<f64>().unwrap_or(0.0),
            Value::HeapString(value) => value.as_str().trim().parse::<f64>().unwrap_or(0.0),
        }
    }

    fn array_slice_bound_from_number(number: f64, length: usize) -> Result<usize> {
        if number.is_nan() || number == 0.0 {
            return Ok(0);
        }
        if !number.is_finite() {
            return if number.is_sign_negative() {
                Ok(0)
            } else {
                Ok(length)
            };
        }

        let length_f64 = Self::array_slice_length_as_f64(length)?;
        let integer = if number.is_sign_negative() {
            number.ceil()
        } else {
            number.floor()
        };
        let clamped = if integer < 0.0 {
            (length_f64 + integer).clamp(0.0, length_f64)
        } else {
            integer.min(length_f64)
        };
        Self::array_slice_nonnegative_usize(clamped)
    }

    fn array_slice_length_as_f64(length: usize) -> Result<f64> {
        let length = u32::try_from(length)
            .map_err(|_| Error::limit("array length exceeded supported range"))?;
        Ok(f64::from(length))
    }

    fn array_slice_nonnegative_usize(value: f64) -> Result<usize> {
        if value == 0.0 {
            return Ok(0);
        }
        format!("{value:.0}")
            .parse::<usize>()
            .map_err(|_| Error::limit("array index exceeded supported range"))
    }

    fn array_last_index_of_start(value: Option<&Value>, length: usize) -> Result<Option<usize>> {
        if length == 0 {
            return Ok(None);
        }
        let Some(value) = value else {
            return Ok(Some(length.saturating_sub(1)));
        };

        let number = Self::array_slice_bound_number(value);
        Self::array_last_index_of_start_from_number(number, length)
    }

    fn array_last_index_of_start_from_number(number: f64, length: usize) -> Result<Option<usize>> {
        if number.is_nan() || number == 0.0 {
            return Ok(Some(0));
        }
        if !number.is_finite() {
            return if number.is_sign_negative() {
                Ok(None)
            } else {
                Ok(Some(length.saturating_sub(1)))
            };
        }

        let length_f64 = Self::array_slice_length_as_f64(length)?;
        let integer = if number.is_sign_negative() {
            number.ceil()
        } else {
            number.floor()
        };
        if integer < 0.0 {
            let index = length_f64 + integer;
            if index < 0.0 {
                return Ok(None);
            }
            return Self::array_slice_nonnegative_usize(index).map(Some);
        }

        let clamped = integer.min(length_f64 - 1.0);
        Self::array_slice_nonnegative_usize(clamped).map(Some)
    }
}
