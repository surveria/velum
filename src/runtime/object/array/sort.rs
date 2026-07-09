use super::{ArrayLength, ObjectHeap, ObjectId, ObjectProperty};
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
        if !properties
            .iter()
            .all(ObjectProperty::has_default_array_attributes)
        {
            return Ok(None);
        }
        Ok(Some(properties.iter().map(ObjectProperty::value).collect()))
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

    pub(crate) fn append_packed_default_numbers_if_array(
        &mut self,
        id: ObjectId,
        expected_length: usize,
        values: &[f64],
        max_properties: usize,
    ) -> Result<bool> {
        let Some(length) = self.array_len_if_array(id)? else {
            return Ok(false);
        };
        if length != expected_length || self.object(id)?.packed_array_properties(length).is_none() {
            return Ok(false);
        }
        let Some(new_length) = length.checked_add(values.len()) else {
            return Ok(false);
        };
        let Ok(new_length) = ArrayLength::from_usize(new_length) else {
            return Ok(false);
        };
        let before = self.object(id)?.structure_snapshot();
        self.object_mut(id)?.append_packed_default_value_iter(
            values.iter().copied().map(Value::Number),
            values.len(),
            max_properties,
        )?;
        self.object_mut(id)?.array_length = Some(new_length);
        self.bump_if_structure_changed(id, before)?;
        Ok(true)
    }

    pub(crate) fn splice_packed_default_array_if_array(
        &mut self,
        id: ObjectId,
        start: usize,
        delete_count: usize,
        items: &[Value],
        max_properties: usize,
    ) -> Result<Option<Vec<Value>>> {
        let Some(length) = self.array_len_if_array(id)? else {
            return Ok(None);
        };
        let Some(new_length) = length.checked_sub(delete_count).and_then(|value| {
            value
                .checked_add(items.len())
                .and_then(|length| ArrayLength::from_usize(length).ok())
        }) else {
            return Ok(None);
        };
        let before = self.object(id)?.structure_snapshot();
        let removed = self
            .object_mut(id)?
            .array_storage
            .splice_packed_default_for_len(length, start, delete_count, items, max_properties);
        if removed.is_some() {
            self.object_mut(id)?.array_length = Some(new_length);
            self.bump_if_structure_changed(id, before)?;
        }
        Ok(removed)
    }
}
