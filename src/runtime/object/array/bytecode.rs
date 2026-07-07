use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{ArrayIndex, ObjectHeap};

impl ObjectHeap {
    pub(crate) fn dynamic_array_index_if_array(
        &self,
        id: ObjectId,
        property: &Value,
    ) -> Result<Option<usize>> {
        if self.array_length_if_array(id)?.is_none() {
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
        if self.array_length_if_array(id)?.is_none() {
            return Ok(None);
        }
        let index = ArrayIndex::from_usize(index)?;
        self.get_array_index(id, index).map(Some)
    }

    pub(crate) fn set_array_index_if_array(
        &mut self,
        id: ObjectId,
        index: usize,
        value: Value,
        max_properties: usize,
    ) -> Result<bool> {
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
        | Value::Symbol(_)
        | Value::Error(_) => None,
    }
}
