use super::{ObjectHeap, ObjectId, ObjectProperty};
use crate::error::Result;
use crate::value::Value;

impl ObjectHeap {
    pub(crate) fn packed_default_array_values_if_array(
        &self,
        id: ObjectId,
    ) -> Result<Option<Vec<Value>>> {
        let Some(length) = self.array_len_if_array(id)? else {
            return Ok(None);
        };
        let Some(properties) = self.object(id)?.packed_array_properties(length) else {
            return Ok(None);
        };
        if !properties.iter().all(|property| {
            property.has_default_array_attributes() && property.data_value_ref().is_some()
        }) {
            return Ok(None);
        }
        Ok(Some(
            properties
                .iter()
                .filter_map(ObjectProperty::data_value_ref)
                .cloned()
                .collect(),
        ))
    }

    pub(crate) fn sort_packed_default_numeric_array_if_array(
        &mut self,
        id: ObjectId,
        descending: bool,
    ) -> Result<bool> {
        let Some(length) = self.array_len_if_array(id)? else {
            return Ok(false);
        };
        Ok(self
            .object_mut(id)?
            .array_storage
            .sort_packed_default_numbers_for_len(length, descending))
    }
}
