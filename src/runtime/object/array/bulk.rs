use super::{ARRAY_INDEX_LIMIT_ERROR, ArrayLength, Object, ObjectHeap, ObjectId};
use crate::error::{Error, Result};
use crate::value::Value;

impl ObjectHeap {
    pub(in crate::runtime::object) fn packed_array_value_range(
        &self,
        id: ObjectId,
        length: usize,
        start: usize,
        count: usize,
    ) -> Result<Option<Vec<Value>>> {
        let Some(properties) = self.object(id)?.packed_array_properties(length) else {
            return Ok(None);
        };
        let mut values = Vec::with_capacity(count);
        for property in properties.iter().skip(start).take(count) {
            values.push(property.value());
        }
        Ok(Some(values))
    }

    pub(in crate::runtime::object) fn packed_concat_values(
        &self,
        id: ObjectId,
        this_length: ArrayLength,
        values: &[Value],
    ) -> Result<Option<Vec<Value>>> {
        let mut result_len = this_length.to_usize()?;
        for value in values {
            let value_len = self.concat_value_length(value)?;
            result_len = result_len
                .checked_add(value_len)
                .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        }

        let mut result = Vec::with_capacity(result_len);
        if !self.extend_packed_concat_values(&mut result, id, this_length.to_usize()?)? {
            return Ok(None);
        }
        for value in values {
            if let Value::Object(source_id) = value
                && let Some(length) = self.array_length_if_array(*source_id)?
            {
                if !self.extend_packed_concat_values(&mut result, *source_id, length.to_usize()?)? {
                    return Ok(None);
                }
            } else {
                result.push(value.clone());
            }
        }
        Ok(Some(result))
    }

    fn concat_value_length(&self, value: &Value) -> Result<usize> {
        if let Value::Object(id) = value
            && let Some(length) = self.array_length_if_array(*id)?
        {
            return length.to_usize();
        }
        Ok(1)
    }

    fn extend_packed_concat_values(
        &self,
        result: &mut Vec<Value>,
        source_id: ObjectId,
        length: usize,
    ) -> Result<bool> {
        let Some(values) = self.packed_array_value_range(source_id, length, 0, length)? else {
            return Ok(false);
        };
        result.extend(values);
        Ok(true)
    }
}

impl Object {
    pub(in crate::runtime::object) fn append_packed_default_values(
        &mut self,
        values: Vec<Value>,
        max_properties: usize,
    ) -> Result<()> {
        let count = self
            .array_storage
            .append_packed_default_values(values, max_properties)?;
        self.enumerable_property_count = self
            .enumerable_property_count
            .checked_add(count)
            .ok_or_else(|| Error::limit("object enumerable property count overflowed"))?;
        Ok(())
    }
}
