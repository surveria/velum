use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{SetFailureBehavior, same_value_zero, strict_equality},
    },
    value::Value,
};

const ARRAY_LENGTH_PROPERTY: &str = "length";
const ARRAY_LIKE_RECEIVER_ERROR: &str = "Array.prototype method requires an object receiver";
const ARRAY_LIKE_LENGTH_LIMIT_ERROR: &str = "array-like length exceeded supported range";
const ARRAY_LIKE_INDEX_LIMIT_ERROR: &str = "array-like index exceeded supported range";
const INDEX_NOT_FOUND: f64 = -1.0;

impl Context {
    pub(super) fn generic_array_push(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let new_length = Self::checked_array_like_length(length, args.len())?;
        for (offset, value) in args.iter().enumerate() {
            self.step()?;
            let index = length
                .checked_add(offset)
                .ok_or_else(|| Error::limit(ARRAY_LIKE_INDEX_LIMIT_ERROR))?;
            self.set_array_like_index(this_value, index, value.clone())?;
        }
        self.set_array_like_length(this_value, new_length)?;
        Self::array_like_length_value(new_length)
    }

    pub(super) fn generic_array_pop(&mut self, this_value: &Value) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        if length == 0 {
            self.set_array_like_length(this_value, 0)?;
            return Ok(Value::Undefined);
        }
        let index = length
            .checked_sub(1)
            .ok_or_else(|| Error::limit(ARRAY_LIKE_INDEX_LIMIT_ERROR))?;
        self.step()?;
        let value = self.get_array_like_index(this_value, index)?;
        self.delete_array_like_index(this_value, index)?;
        self.set_array_like_length(this_value, index)?;
        Ok(value)
    }

    pub(super) fn generic_array_shift(&mut self, this_value: &Value) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        if length == 0 {
            self.set_array_like_length(this_value, 0)?;
            return Ok(Value::Undefined);
        }
        let first = self.get_array_like_index(this_value, 0)?;
        for source in 1..length {
            self.step()?;
            let target = source
                .checked_sub(1)
                .ok_or_else(|| Error::limit(ARRAY_LIKE_INDEX_LIMIT_ERROR))?;
            if self.has_array_like_index(this_value, source)? {
                let value = self.get_array_like_index(this_value, source)?;
                self.set_array_like_index(this_value, target, value)?;
            } else {
                self.delete_array_like_index(this_value, target)?;
            }
        }
        let new_length = length
            .checked_sub(1)
            .ok_or_else(|| Error::limit(ARRAY_LIKE_LENGTH_LIMIT_ERROR))?;
        self.delete_array_like_index(this_value, new_length)?;
        self.set_array_like_length(this_value, new_length)?;
        Ok(first)
    }

    pub(super) fn generic_array_unshift(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let new_length = Self::checked_array_like_length(length, args.len())?;
        if args.is_empty() {
            self.set_array_like_length(this_value, length)?;
            return Self::array_like_length_value(length);
        }

        for source in (0..length).rev() {
            self.step()?;
            let target = source
                .checked_add(args.len())
                .ok_or_else(|| Error::limit(ARRAY_LIKE_INDEX_LIMIT_ERROR))?;
            if self.has_array_like_index(this_value, source)? {
                let value = self.get_array_like_index(this_value, source)?;
                self.set_array_like_index(this_value, target, value)?;
            } else {
                self.delete_array_like_index(this_value, target)?;
            }
        }
        for (index, value) in args.iter().enumerate() {
            self.step()?;
            self.set_array_like_index(this_value, index, value.clone())?;
        }
        self.set_array_like_length(this_value, new_length)?;
        Self::array_like_length_value(new_length)
    }

    pub(super) fn generic_array_slice(
        &mut self,
        start: Option<&Value>,
        end: Option<&Value>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let start = self.array_slice_bound(start, length, 0)?;
        let end = self.array_slice_bound(end, length, length)?.max(start);
        let count = end
            .checked_sub(start)
            .ok_or_else(|| Error::limit(ARRAY_LIKE_LENGTH_LIMIT_ERROR))?;
        let prototype = self.existing_array_constructor_prototype()?;
        let result =
            self.objects
                .create_array_with_length(count, prototype, self.limits.max_objects)?;
        for offset in 0..count {
            self.step()?;
            let source = start
                .checked_add(offset)
                .ok_or_else(|| Error::limit(ARRAY_LIKE_INDEX_LIMIT_ERROR))?;
            if self.has_array_like_index(this_value, source)? {
                let value = self.get_array_like_index(this_value, source)?;
                self.set_array_like_index(&result, offset, value)?;
            }
        }
        Ok(result)
    }

    pub(super) fn generic_array_join(
        &mut self,
        separator: &str,
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let mut joined = self.join_string_with_separator_capacity(length, separator.len())?;
        for index in 0..length {
            self.step()?;
            if index > 0 {
                self.push_join_text(&mut joined, separator)?;
            }
            let value = self.get_array_like_index(this_value, index)?;
            self.push_join_value_text(&mut joined, &value)?;
        }
        self.heap_string_value(&joined)
    }

    pub(super) fn generic_array_index_of(
        &mut self,
        search: &Value,
        from_index: Option<&Value>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        if length == 0 {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        }
        let start = self.array_slice_bound(from_index, length, 0)?;
        if start >= length {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        }
        for index in start..length {
            self.step()?;
            if self.has_array_like_index(this_value, index)? {
                let value = self.get_array_like_index(this_value, index)?;
                if strict_equality(&value, search) {
                    return Self::array_like_index_value(index);
                }
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(super) fn generic_array_includes(
        &mut self,
        search: &Value,
        from_index: Option<&Value>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        if length == 0 {
            return Ok(Value::Bool(false));
        }
        let start = self.array_slice_bound(from_index, length, 0)?;
        if start >= length {
            return Ok(Value::Bool(false));
        }
        for index in start..length {
            self.step()?;
            let value = self.get_array_like_index(this_value, index)?;
            if same_value_zero(&value, search) {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    pub(super) fn generic_array_last_index_of(
        &mut self,
        search: &Value,
        from_index: Option<&Value>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        if length == 0 {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        }
        let Some(start) = self.array_last_index_of_start(from_index, length)? else {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        };
        for index in (0..=start).rev() {
            self.step()?;
            if self.has_array_like_index(this_value, index)? {
                let value = self.get_array_like_index(this_value, index)?;
                if strict_equality(&value, search) {
                    return Self::array_like_index_value(index);
                }
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(super) fn generic_array_reverse(&mut self, this_value: &Value) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        if length <= 1 {
            return Ok(this_value.clone());
        }
        let middle = length / 2;
        for lower in 0..middle {
            self.step()?;
            let upper = length
                .checked_sub(lower)
                .and_then(|index| index.checked_sub(1))
                .ok_or_else(|| Error::limit(ARRAY_LIKE_INDEX_LIMIT_ERROR))?;
            self.reverse_array_like_pair(this_value, lower, upper)?;
        }
        Ok(this_value.clone())
    }

    fn reverse_array_like_pair(
        &mut self,
        object: &Value,
        lower: usize,
        upper: usize,
    ) -> Result<()> {
        let lower_present = self.has_array_like_index(object, lower)?;
        let upper_present = self.has_array_like_index(object, upper)?;
        let lower_value = if lower_present {
            Some(self.get_array_like_index(object, lower)?)
        } else {
            None
        };
        let upper_value = if upper_present {
            Some(self.get_array_like_index(object, upper)?)
        } else {
            None
        };

        match (lower_value, upper_value) {
            (Some(lower_value), Some(upper_value)) => {
                self.set_array_like_index(object, lower, upper_value)?;
                self.set_array_like_index(object, upper, lower_value)?;
            }
            (None, Some(upper_value)) => {
                self.set_array_like_index(object, lower, upper_value)?;
                self.delete_array_like_index(object, upper)?;
            }
            (Some(lower_value), None) => {
                self.delete_array_like_index(object, lower)?;
                self.set_array_like_index(object, upper, lower_value)?;
            }
            (None, None) => {}
        }
        Ok(())
    }

    pub(in crate::runtime) fn array_like_length(&mut self, object: &Value) -> Result<usize> {
        let length = self.get_named(object, ARRAY_LENGTH_PROPERTY)?;
        self.length_value_to_usize(&length)
    }

    pub(in crate::runtime) fn set_array_like_length(
        &mut self,
        object: &Value,
        length: usize,
    ) -> Result<()> {
        let value = Self::array_like_length_value(length)?;
        let lookup = self.property_lookup(ARRAY_LENGTH_PROPERTY);
        self.set(object, lookup, value, object, SetFailureBehavior::Throw)
            .map(|_| ())
    }

    pub(in crate::runtime) fn get_array_like_index(
        &mut self,
        object: &Value,
        index: usize,
    ) -> Result<Value> {
        let property = Self::array_like_index_name(index)?;
        self.get_named(object, &property)
    }

    pub(super) fn has_array_like_index(&mut self, object: &Value, index: usize) -> Result<bool> {
        let property = Self::array_like_index_name(index)?;
        self.has_property_value_with_lookup(object, self.property_lookup(&property))
    }

    pub(super) fn set_array_like_index(
        &mut self,
        object: &Value,
        index: usize,
        value: Value,
    ) -> Result<()> {
        let property = Self::array_like_index_name(index)?;
        self.set_array_like_property(object, &property, value)
    }

    fn set_array_like_property(
        &mut self,
        object: &Value,
        property: &str,
        value: Value,
    ) -> Result<()> {
        let key = self.intern_property_key(property)?;
        self.set_property_value_with_accessors(object, key, property, value)
    }

    pub(super) fn delete_array_like_index(&mut self, object: &Value, index: usize) -> Result<()> {
        let property = Self::array_like_index_name(index)?;
        let lookup = self.property_lookup(&property);
        self.delete_property_value_with_lookup(object, lookup)
            .map(|_| ())
    }

    pub(super) fn ensure_array_like_object(object: &Value) -> Result<()> {
        if matches!(object, Value::Object(_)) {
            return Ok(());
        }
        Err(Error::runtime(ARRAY_LIKE_RECEIVER_ERROR))
    }

    /// Clamp a numeric index into `[0, length]`.
    pub(super) fn array_clamp_index(number: f64, length: usize) -> Result<usize> {
        if number <= 0.0 {
            return Ok(0);
        }
        if !number.is_finite() {
            return Ok(length);
        }
        let clamped = number.min(Self::array_length_as_f64(length)?);
        Self::nonnegative_integer_to_usize(clamped).map(|value| value.min(length))
    }

    fn array_length_as_f64(length: usize) -> Result<f64> {
        Self::usize_to_number(length, ARRAY_LIKE_LENGTH_LIMIT_ERROR)
    }

    fn checked_array_like_length(length: usize, additional: usize) -> Result<usize> {
        let length = length
            .checked_add(additional)
            .ok_or_else(|| Error::limit(ARRAY_LIKE_LENGTH_LIMIT_ERROR))?;
        let max = Self::max_array_like_length()?;
        if length > max {
            return Err(Error::limit(ARRAY_LIKE_LENGTH_LIMIT_ERROR));
        }
        Ok(length)
    }

    fn length_value_to_usize(&mut self, value: &Value) -> Result<usize> {
        let length = self.to_length(value)?;
        Self::length_to_usize(length, ARRAY_LIKE_LENGTH_LIMIT_ERROR)
    }

    pub(super) fn array_like_length_value(length: usize) -> Result<Value> {
        Self::usize_to_number(length, ARRAY_LIKE_LENGTH_LIMIT_ERROR).map(Value::Number)
    }

    pub(in crate::runtime) fn array_like_index_value(index: usize) -> Result<Value> {
        Self::usize_to_number(index, ARRAY_LIKE_INDEX_LIMIT_ERROR).map(Value::Number)
    }

    fn array_like_index_name(index: usize) -> Result<String> {
        let max = Self::max_array_like_index()?;
        if index > max {
            return Err(Error::limit(ARRAY_LIKE_INDEX_LIMIT_ERROR));
        }
        Ok(index.to_string())
    }

    fn max_array_like_length() -> Result<usize> {
        Self::length_to_usize(9_007_199_254_740_991, ARRAY_LIKE_LENGTH_LIMIT_ERROR)
    }

    fn max_array_like_index() -> Result<usize> {
        Self::length_to_usize(9_007_199_254_740_990, ARRAY_LIKE_INDEX_LIMIT_ERROR)
    }

    fn nonnegative_integer_to_usize(value: f64) -> Result<usize> {
        Self::finite_nonnegative_integer_to_usize(value, ARRAY_LIKE_LENGTH_LIMIT_ERROR)
    }
}
