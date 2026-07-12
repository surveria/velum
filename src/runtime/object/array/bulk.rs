use super::{
    ARRAY_INDEX_LIMIT_ERROR, ArrayCopyLimits, Object, ObjectHeap, ObjectId, ObjectProperty,
};
use crate::error::{Error, Result};
use crate::value::Value;

impl ObjectHeap {
    pub(in crate::runtime::object) fn create_packed_array_slice(
        &mut self,
        id: ObjectId,
        length: usize,
        start: usize,
        count: usize,
        prototype: ObjectId,
        limits: ArrayCopyLimits,
    ) -> Result<Option<Value>> {
        let Some(properties) = self.object(id)?.packed_array_properties(length) else {
            return Ok(None);
        };
        Self::packed_property_range(properties, start, count)?;
        Self::ensure_packed_result_property_limit(count, limits.max_properties)?;
        let result = self.create_array_with_length(count, prototype, limits.max_objects)?;
        let Value::Object(result_id) = result else {
            return Err(Error::runtime("array slice result is not an object"));
        };
        self.append_packed_array_range(result_id, id, length, start, count, limits.max_properties)?;
        Ok(Some(Value::Object(result_id)))
    }

    fn packed_property_range(
        properties: &[ObjectProperty],
        start: usize,
        count: usize,
    ) -> Result<&[ObjectProperty]> {
        let end = start
            .checked_add(count)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        properties
            .get(start..end)
            .ok_or_else(|| Error::runtime("packed array property range is unavailable"))
    }

    fn ensure_packed_result_property_limit(count: usize, max_properties: usize) -> Result<()> {
        if count <= max_properties {
            return Ok(());
        }
        Err(Error::limit(format!(
            "object property count exceeded {max_properties}"
        )))
    }

    fn append_packed_array_range(
        &mut self,
        result_id: ObjectId,
        source_id: ObjectId,
        length: usize,
        start: usize,
        count: usize,
        max_properties: usize,
    ) -> Result<()> {
        let (source, result) =
            Self::object_pair_for_concat(&mut self.objects, source_id, result_id)?;
        let Some(properties) = source.packed_array_properties(length) else {
            return Err(Error::runtime("packed array source is no longer packed"));
        };
        Self::packed_property_range(properties, start, count)?;
        result.append_packed_default_property_values(properties, start, count, max_properties)
    }
}

impl Object {
    pub(in crate::runtime::object) fn append_packed_default_value_iter(
        &mut self,
        values: impl IntoIterator<Item = Value>,
        value_count: usize,
        max_properties: usize,
    ) -> Result<()> {
        let reservation = self.reserve_property_growth_by(value_count)?;
        let count = self.array_storage.append_packed_default_value_iter(
            values,
            value_count,
            max_properties,
        )?;
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
        self.add_enumerable_properties(count)
    }

    pub(in crate::runtime::object) fn append_packed_default_property_values(
        &mut self,
        properties: &[ObjectProperty],
        start: usize,
        count: usize,
        max_properties: usize,
    ) -> Result<()> {
        let reservation = self.reserve_property_growth_by(count)?;
        let count = self.array_storage.append_packed_default_property_values(
            properties,
            start,
            count,
            max_properties,
        )?;
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
        self.add_enumerable_properties(count)
    }

    pub(in crate::runtime::object) fn pop_packed_for_len_if_configurable(
        &mut self,
        len: usize,
    ) -> Result<Option<ObjectProperty>> {
        let Some(property) = self.array_storage.pop_packed_for_len_if_configurable(len) else {
            return Ok(None);
        };
        if property.is_enumerable() {
            self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
        }
        self.release_property()?;
        Ok(Some(property))
    }

    fn add_enumerable_properties(&mut self, count: usize) -> Result<()> {
        self.enumerable_property_count = self
            .enumerable_property_count
            .checked_add(count)
            .ok_or_else(|| Error::limit("object enumerable property count overflowed"))?;
        Ok(())
    }
}
