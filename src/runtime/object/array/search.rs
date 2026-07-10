use std::fmt::Write as _;

use crate::{
    error::{Error, Result},
    runtime::abstract_operations::{
        number_same_value_zero, number_strict_equality, same_value_zero, strict_equality,
    },
    value::Value,
};

use super::{ARRAY_INDEX_LIMIT_ERROR, INDEX_NOT_FOUND, ObjectHeap, ObjectProperty};

impl ObjectHeap {
    pub(super) fn array_index_value(index: usize) -> Result<Value> {
        let index = u32::try_from(index).map_err(|_| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        Ok(Value::Number(f64::from(index)))
    }

    pub(super) fn push_join_text(
        joined: &mut String,
        text: &str,
        max_string_len: usize,
    ) -> Result<()> {
        let length = joined
            .len()
            .checked_add(text.len())
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
        if length > max_string_len {
            return Err(Error::limit(format!(
                "string length {length} exceeded {max_string_len}"
            )));
        }
        joined.push_str(text);
        Ok(())
    }

    pub(super) fn push_join_value_text(
        joined: &mut String,
        value: &Value,
        max_string_len: usize,
    ) -> Result<()> {
        match value {
            Value::Undefined | Value::Null => Ok(()),
            Value::String(value) => Self::push_join_text(joined, value, max_string_len),
            Value::HeapString(value) => {
                Self::push_join_text(joined, value.as_str(), max_string_len)
            }
            _ => Self::write_join_display(joined, value, max_string_len),
        }
    }

    fn write_join_display(joined: &mut String, value: &Value, max_string_len: usize) -> Result<()> {
        joined.write_fmt(format_args!("{value}")).map_err(|error| {
            Error::runtime(format!("failed to format array join value: {error}"))
        })?;
        if joined.len() > max_string_len {
            return Err(Error::limit(format!(
                "string length {} exceeded {}",
                joined.len(),
                max_string_len
            )));
        }
        Ok(())
    }

    pub(super) fn join_string_with_separator_capacity(
        length: usize,
        separator_len: usize,
        max_string_len: usize,
    ) -> Result<String> {
        let separator_count = length.saturating_sub(1);
        let separator_bytes = separator_count
            .checked_mul(separator_len)
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
        if separator_bytes > max_string_len {
            return Err(Error::limit(format!(
                "string length {separator_bytes} exceeded {max_string_len}"
            )));
        }
        Ok(String::with_capacity(separator_bytes))
    }

    pub(super) fn packed_array_index_of(
        properties: &[ObjectProperty],
        search: &Value,
        start: usize,
    ) -> Result<Value> {
        for (position, property) in properties.iter().enumerate().skip(start) {
            if property
                .data_value_ref()
                .is_some_and(|value| strict_equality(value, search))
            {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(super) fn packed_array_index_of_number(
        properties: &[ObjectProperty],
        search: f64,
        start: usize,
    ) -> Result<Value> {
        for (position, property) in properties.iter().enumerate().skip(start) {
            if let Some(Value::Number(value)) = property.data_value_ref()
                && number_strict_equality(*value, search)
            {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(super) fn packed_array_includes(
        properties: &[ObjectProperty],
        search: &Value,
        start: usize,
    ) -> Value {
        for property in properties.iter().skip(start) {
            if property
                .data_value_ref()
                .is_some_and(|value| same_value_zero(value, search))
            {
                return Value::Bool(true);
            }
        }
        Value::Bool(false)
    }

    pub(super) fn packed_array_includes_number(
        properties: &[ObjectProperty],
        search: f64,
        start: usize,
    ) -> Value {
        for property in properties.iter().skip(start) {
            if let Some(Value::Number(value)) = property.data_value_ref()
                && number_same_value_zero(*value, search)
            {
                return Value::Bool(true);
            }
        }
        Value::Bool(false)
    }

    pub(super) fn packed_array_last_index_of(
        properties: &[ObjectProperty],
        search: &Value,
        start: usize,
    ) -> Result<Value> {
        if properties.is_empty() {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        }
        let upper = start.min(properties.len().saturating_sub(1));
        let count = upper
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        for (position, property) in properties.iter().enumerate().take(count).rev() {
            if property
                .data_value_ref()
                .is_some_and(|value| strict_equality(value, search))
            {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(super) fn packed_array_last_index_of_number(
        properties: &[ObjectProperty],
        search: f64,
        start: usize,
    ) -> Result<Value> {
        if properties.is_empty() {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        }
        let upper = start.min(properties.len().saturating_sub(1));
        let count = upper
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        for (position, property) in properties.iter().enumerate().take(count).rev() {
            if let Some(Value::Number(value)) = property.data_value_ref()
                && number_strict_equality(*value, search)
            {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }
}
