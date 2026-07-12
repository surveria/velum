use crate::{
    error::Result,
    runtime::object::property::PrototypeTraversalBudget,
    value::{ObjectId, Value},
};

use super::{ArrayIndex, ObjectHeap};
use crate::runtime::object::typed_array_number;

impl ObjectHeap {
    fn array_index_has_accessor_in_chain(&self, id: ObjectId, index: ArrayIndex) -> Result<bool> {
        let mut current = Some(id);
        let mut budget = PrototypeTraversalBudget::from_object_count(self.object_count());
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if let Some(property) = object.array_storage.dense_property(index) {
                return Ok(property.accessor().is_some());
            }
            if let Some(key) = object.array_storage.sparse_key(index)
                && let Some(property) = object.named_property(&self.shapes, key)?
            {
                return Ok(property.accessor().is_some());
            }
            current = object.prototype;
        }
        Ok(false)
    }

    pub(crate) fn dynamic_array_index_if_array(
        &self,
        id: ObjectId,
        property: &Value,
    ) -> Result<Option<usize>> {
        if self.array_length_if_array(id)?.is_none() && self.typed_array(id)?.is_none() {
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
        if let Some(number) = self.typed_array_number(id, index)? {
            return Ok(Some(Value::Number(number)));
        }
        if self.array_length_if_array(id)?.is_none() {
            return Ok(None);
        }
        let index = ArrayIndex::from_usize(index)?;
        if self.array_index_has_accessor_in_chain(id, index)? {
            return Ok(None);
        }
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
        if self.typed_array(id)?.is_some() {
            self.set_typed_array_number(id, index, typed_array_number(&value)?)?;
            return Ok(true);
        }
        if self.array_length_if_array(id)?.is_none() {
            return Ok(false);
        }
        let index = ArrayIndex::from_usize(index)?;
        if self.array_index_has_accessor_in_chain(id, index)? {
            return Ok(false);
        }
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
        Value::Number(_) | Value::BigInt(_) => ArrayIndex::parse(&property.to_string()),
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
