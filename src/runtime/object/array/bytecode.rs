use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{ArrayIndex, ObjectHeap};
use crate::runtime::object::byte_number;

impl ObjectHeap {
    pub(crate) fn dynamic_array_index_if_array(
        &self,
        id: ObjectId,
        property: &Value,
    ) -> Result<Option<usize>> {
        if self.array_length_if_array(id)?.is_none() && self.uint8_array(id)?.is_none() {
            return Ok(None);
        }
        let Some(index) = array_index_from_property_value(property) else {
            return Ok(None);
        };
        index.position().map(Some)
    }

    pub(crate) fn array_index_value_if_array(
        &self,
        id: ObjectId,
        index: usize,
    ) -> Result<Option<Value>> {
        if let Some(byte) = self.uint8_array_byte(id, index)? {
            return Ok(Some(Value::Number(f64::from(byte))));
        }
        if self.array_length_if_array(id)?.is_none() {
            return Ok(None);
        }
        let index = ArrayIndex::from_usize(index)?;
        self.get_array_index(id, index).map(Some)
    }

    pub(crate) fn has_own_array_index_if_array(
        &self,
        id: ObjectId,
        index: usize,
    ) -> Result<Option<bool>> {
        if self.array_length_if_array(id)?.is_none() {
            return Ok(None);
        }
        let index = ArrayIndex::from_usize(index)?;
        self.object(id)?
            .get_own_array_index(&self.shapes, index)
            .map(|value| value.is_some())
            .map(Some)
    }

    pub(crate) fn set_array_index_if_array(
        &mut self,
        id: ObjectId,
        index: usize,
        value: Value,
        max_properties: usize,
    ) -> Result<bool> {
        if self.uint8_array(id)?.is_some() {
            self.set_uint8_array_byte(id, index, byte_number(&value)?)?;
            return Ok(true);
        }
        if self.array_length_if_array(id)?.is_none() {
            return Ok(false);
        }
        let index = ArrayIndex::from_usize(index)?;
        if index.dense_position(max_properties)?.is_none() {
            return Ok(false);
        }
        self.set_array_index(id, index, value, max_properties)?;
        Ok(true)
    }
}

fn array_index_from_property_value(property: &Value) -> Option<ArrayIndex> {
    match property {
        Value::String(value) => ArrayIndex::parse(value),
        Value::HeapString(value) => ArrayIndex::parse(value.as_str()),
        Value::Number(_) => ArrayIndex::parse(&property.to_string()),
        Value::Undefined
        | Value::Null
        | Value::Bool(_)
        | Value::Function(_)
        | Value::NativeFunction(_)
        | Value::HostFunction(_)
        | Value::Object(_)
        | Value::Symbol(_) => None,
    }
}
