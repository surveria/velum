use crate::{
    error::{Error, Result},
    value::{ObjectId, Value},
};

use super::{ARRAY_INDEX_LIMIT_ERROR, ArrayIndex, ObjectHeap};

const ARRAY_SHIFT_RECEIVER_ERROR: &str = "Array.prototype.shift requires an array receiver";
const ARRAY_UNSHIFT_RECEIVER_ERROR: &str = "Array.prototype.unshift requires an array receiver";

impl ObjectHeap {
    pub(crate) fn array_shift(&mut self, id: ObjectId, max_properties: usize) -> Result<Value> {
        let length = self.array_length_for_method(id, ARRAY_SHIFT_RECEIVER_ERROR)?;
        let Some(first_index) = length.first_index() else {
            return Ok(Value::Undefined);
        };
        let length_usize = length.to_usize()?;
        let Some(last_index) = length.previous_index() else {
            return Ok(Value::Undefined);
        };
        if let Some(first_property) = self
            .object_mut(id)?
            .array_storage
            .shift_packed_for_len_if_default(length_usize)
        {
            self.object_mut(id)?.array_length = Some(last_index.length());
            self.bump_prototype_lookup_version()?;
            return Ok(first_property.value());
        }

        let first_value = self.get_array_index(id, first_index)?;
        for index in 1..length_usize {
            self.move_array_index(id, index, index.saturating_sub(1), max_properties)?;
        }

        self.delete_array_index(id, last_index)?;
        self.object_mut(id)?.array_length = Some(last_index.length());
        Ok(first_value)
    }

    pub(crate) fn array_unshift(
        &mut self,
        id: ObjectId,
        values: &[Value],
        max_properties: usize,
    ) -> Result<Value> {
        let length = self.array_length_for_method(id, ARRAY_UNSHIFT_RECEIVER_ERROR)?;
        let value_count = values.len();
        let new_length = length.add_usize(value_count)?;
        if value_count == 0 {
            return Ok(new_length.value());
        }

        let length_usize = length.to_usize()?;
        if self
            .object_mut(id)?
            .array_storage
            .unshift_packed_for_len_if_default(length_usize, values, max_properties)
        {
            self.object_mut(id)?.array_length = Some(new_length);
            self.bump_prototype_lookup_version()?;
            return Ok(new_length.value());
        }

        for offset in 0..length_usize {
            let from_index = length_usize.saturating_sub(offset).saturating_sub(1);
            let to_index = from_index
                .checked_add(value_count)
                .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
            self.move_array_index(id, from_index, to_index, max_properties)?;
        }

        for (index, value) in values.iter().enumerate() {
            let index = ArrayIndex::from_usize(index)?;
            self.set_array_index(id, index, value.clone(), max_properties)?;
        }
        self.object_mut(id)?.array_length = Some(new_length);
        Ok(new_length.value())
    }
}
