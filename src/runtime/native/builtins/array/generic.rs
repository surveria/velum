#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{SetFailureBehavior, same_value_zero, strict_equality, to_boolean},
        object::{PropertyKey, PropertyLookup},
        roots::VmRootKind,
    },
    value::{ErrorName, Value},
};

const ARRAY_LENGTH_PROPERTY: &str = "length";
const ARRAY_LIKE_RECEIVER_ERROR: &str = "Array.prototype method requires an object receiver";
const ARRAY_LIKE_LENGTH_LIMIT_ERROR: &str = "array-like length exceeded supported range";
const ARRAY_LIKE_INDEX_LIMIT_ERROR: &str = "array-like index exceeded supported range";
const ARRAY_CONCAT_LENGTH_ERROR: &str = "Array.prototype.concat result exceeds safe integer range";
const ARRAY_SPECIES_ERROR: &str = "Array species value must be a constructor";
const ARRAY_SPECIES_LENGTH_RANGE_ERROR: &str = "Invalid array length";
const ARRAY_DELETE_PROPERTY_ERROR: &str = "Cannot delete non-configurable array-like property";
const ARRAY_CONSTRUCTOR_PROPERTY: &str = "constructor";
const IS_CONCAT_SPREADABLE_PROPERTY: &str = "isConcatSpreadable";
const IS_CONCAT_SPREADABLE_DISPLAY: &str = "[Symbol.isConcatSpreadable]";
const SPECIES_PROPERTY: &str = "species";
const SPECIES_DISPLAY: &str = "[Symbol.species]";
const INDEX_NOT_FOUND: f64 = -1.0;

impl Context {
    pub(super) const fn array_search_bound_is_primitive(value: Option<&Value>) -> bool {
        match value {
            None => true,
            Some(value) => matches!(
                value,
                Value::Undefined
                    | Value::Null
                    | Value::Bool(_)
                    | Value::Number(_)
                    | Value::BigInt(_)
                    | Value::String(_)
                    | Value::Symbol(_)
            ),
        }
    }

    pub(super) fn array_join_separator(&mut self, value: Option<&Value>) -> Result<String> {
        match value {
            None | Some(Value::Undefined) => Ok(super::ARRAY_JOIN_DEFAULT_SEPARATOR.to_owned()),
            Some(value) => self.to_string(value),
        }
    }

    pub(super) fn generic_array_concat(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let result = self.array_species_create(this_value, 0)?;
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values(core::iter::once(&result))?;
        let spreadable_key = self.array_concat_spreadable_key()?;
        let mut next_index = 0_usize;
        for item in core::iter::once(this_value).chain(args.iter()) {
            if self.array_concat_is_spreadable(item, spreadable_key)? {
                let length = self.array_like_length(item)?;
                let end = concat_checked_length(next_index, length)?;
                for source_index in 0..length {
                    self.step()?;
                    if self.has_array_like_index(item, source_index)? {
                        let value = self.get_array_like_index(item, source_index)?;
                        self.array_from_create_data_property(&result, next_index, value)?;
                    }
                    next_index = next_index
                        .checked_add(1)
                        .ok_or_else(|| Error::type_error(ARRAY_CONCAT_LENGTH_ERROR))?;
                }
                if next_index != end {
                    return Err(Error::runtime("Array concat index accounting drifted"));
                }
            } else {
                let end = concat_checked_length(next_index, 1)?;
                self.array_from_create_data_property(&result, next_index, item.clone())?;
                next_index = end;
            }
        }
        self.set_array_like_length(&result, next_index)?;
        Ok(result)
    }

    pub(super) fn array_species_create(
        &mut self,
        original: &Value,
        length: usize,
    ) -> Result<Value> {
        let is_array = self.semantic_is_array(original)?;
        if !is_array {
            return self.create_intrinsic_array_with_length(length);
        }

        let mut constructor = self.get_named(original, ARRAY_CONSTRUCTOR_PROPERTY)?;
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values(core::iter::once(&constructor))?;
        if self.semantic_is_constructor(&constructor)?
            && self.is_foreign_intrinsic_array_constructor(&constructor)?
        {
            constructor = Value::Undefined;
        }
        if self.semantic_object_ref(&constructor)?.is_some() {
            let species_key = self.array_species_key()?;
            constructor = self.get(
                &constructor,
                PropertyLookup::from_key(SPECIES_DISPLAY, species_key),
            )?;
            roots.add_values(core::iter::once(&constructor))?;
            if matches!(constructor, Value::Null) {
                constructor = Value::Undefined;
            }
        }
        if matches!(constructor, Value::Undefined) {
            return self.create_intrinsic_array_with_length(length);
        }
        if !self.semantic_is_constructor(&constructor)? {
            return Err(Error::type_error(ARRAY_SPECIES_ERROR));
        }
        let length = Self::array_like_length_value(length)?;
        self.semantic_construct(
            &constructor,
            core::slice::from_ref(&length),
            constructor.clone(),
        )
    }

    pub(super) fn create_intrinsic_array_with_length(&mut self, length: usize) -> Result<Value> {
        if u64::try_from(length).map_or(true, |length| length > u64::from(u32::MAX)) {
            return Err(Error::exception(
                ErrorName::RangeError,
                ARRAY_SPECIES_LENGTH_RANGE_ERROR,
            ));
        }
        let prototype = self.existing_array_constructor_prototype()?;
        self.objects
            .create_array_with_length(length, prototype, self.limits.max_objects)
    }

    fn array_species_key(&mut self) -> Result<PropertyKey> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&symbol_constructor, SPECIES_PROPERTY)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime("Symbol.species is not initialized"));
        };
        Ok(PropertyKey::symbol(symbol.id()))
    }

    fn array_concat_spreadable_key(&mut self) -> Result<PropertyKey> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&symbol_constructor, IS_CONCAT_SPREADABLE_PROPERTY)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime(
                "Symbol.isConcatSpreadable is not initialized",
            ));
        };
        Ok(PropertyKey::symbol(symbol.id()))
    }

    fn array_concat_is_spreadable(&mut self, value: &Value, key: PropertyKey) -> Result<bool> {
        if self.semantic_object_ref(value)?.is_none() {
            return Ok(false);
        }
        let spreadable = self.get(
            value,
            PropertyLookup::from_key(IS_CONCAT_SPREADABLE_DISPLAY, key),
        )?;
        if !matches!(spreadable, Value::Undefined) {
            return to_boolean(self, &spreadable);
        }
        self.semantic_is_array(value)
    }

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
        let result = self.array_species_create(this_value, count)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, core::iter::once(&result))?;
        for offset in 0..count {
            self.step()?;
            let source = start
                .checked_add(offset)
                .ok_or_else(|| Error::limit(ARRAY_LIKE_INDEX_LIMIT_ERROR))?;
            if self.has_array_like_index(this_value, source)? {
                let value = self.get_array_like_index(this_value, source)?;
                self.array_from_create_data_property(&result, offset, value)?;
            }
        }
        self.set_array_like_length(&result, count)?;
        Ok(result)
    }

    pub(super) fn generic_array_join_with_length(
        &mut self,
        separator: &str,
        this_value: &Value,
        length: usize,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
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
        let lower_value = if self.has_array_like_index(object, lower)? {
            Some(self.get_array_like_index(object, lower)?)
        } else {
            None
        };
        let upper_value = if self.has_array_like_index(object, upper)? {
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
        if let Value::Object(id) = object
            && self.objects.typed_array(*id)?.is_some()
        {
            return self.set_property_value_with_accessors(object, key, property, value);
        }
        let lookup = PropertyLookup::from_key(property, key);
        self.set(object, lookup, value, object, SetFailureBehavior::Throw)
            .map(|_| ())
    }

    pub(super) fn delete_array_like_index(&mut self, object: &Value, index: usize) -> Result<()> {
        let property = Self::array_like_index_name(index)?;
        let lookup = self.property_lookup(&property);
        if self.delete_property_value_with_lookup(object, lookup)? {
            return Ok(());
        }
        Err(Error::type_error(ARRAY_DELETE_PROPERTY_ERROR))
    }

    pub(super) fn ensure_array_like_object(object: &Value) -> Result<()> {
        if matches!(
            object,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        ) {
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
            .ok_or_else(|| Error::type_error(ARRAY_LIKE_LENGTH_LIMIT_ERROR))?;
        let max = Self::max_array_like_length()?;
        if length > max {
            return Err(Error::type_error(ARRAY_LIKE_LENGTH_LIMIT_ERROR));
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

fn concat_checked_length(current: usize, additional: usize) -> Result<usize> {
    let length = current
        .checked_add(additional)
        .ok_or_else(|| Error::type_error(ARRAY_CONCAT_LENGTH_ERROR))?;
    let max = usize::try_from(9_007_199_254_740_991_u64)
        .map_err(|_| Error::limit(ARRAY_LIKE_LENGTH_LIMIT_ERROR))?;
    if length > max {
        return Err(Error::type_error(ARRAY_CONCAT_LENGTH_ERROR));
    }
    Ok(length)
}
