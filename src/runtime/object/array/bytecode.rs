use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{ArrayIndex, ObjectHeap};

impl ObjectHeap {
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
