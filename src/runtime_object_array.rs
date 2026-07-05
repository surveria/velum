use crate::{
    error::{Error, Result},
    value::{ObjectId, Value},
};

use super::{ARRAY_INDEX_LIMIT_ERROR, ArrayIndex, ArrayLength, ObjectHeap};

const ARRAY_INDEX_OF_RECEIVER_ERROR: &str = "Array.prototype.indexOf requires an array receiver";
const ARRAY_JOIN_RECEIVER_ERROR: &str = "Array.prototype.join requires an array receiver";
const ARRAY_LAST_INDEX_OF_RECEIVER_ERROR: &str =
    "Array.prototype.lastIndexOf requires an array receiver";
const ARRAY_POP_RECEIVER_ERROR: &str = "Array.prototype.pop requires an array receiver";
const ARRAY_PUSH_RECEIVER_ERROR: &str = "Array.prototype.push requires an array receiver";
const ARRAY_SHIFT_RECEIVER_ERROR: &str = "Array.prototype.shift requires an array receiver";
const ARRAY_SLICE_RECEIVER_ERROR: &str = "Array.prototype.slice requires an array receiver";
const ARRAY_UNSHIFT_RECEIVER_ERROR: &str = "Array.prototype.unshift requires an array receiver";
const INDEX_NOT_FOUND: f64 = -1.0;

impl ObjectHeap {
    pub(crate) fn array_push(
        &mut self,
        id: ObjectId,
        values: Vec<Value>,
        max_properties: usize,
    ) -> Result<Value> {
        let object = self.object_mut(id)?;
        let mut length = object
            .array_length
            .ok_or_else(|| Error::runtime(ARRAY_PUSH_RECEIVER_ERROR))?;

        for value in values {
            let index = length.index()?;
            object.set_ordinary(index.key(), value, max_properties)?;
            length = index.next_length()?;
        }
        object.array_length = Some(length);
        Ok(length.value())
    }

    pub(crate) fn array_pop(&mut self, id: ObjectId) -> Result<Value> {
        let object = self.object_mut(id)?;
        let Some(length) = object.array_length else {
            return Err(Error::runtime(ARRAY_POP_RECEIVER_ERROR));
        };
        let Some(index) = length.previous_index() else {
            return Ok(Value::Undefined);
        };

        let key = index.key();
        let value = object.get_own(&key).unwrap_or(Value::Undefined);
        object.delete(&key);
        object.array_length = Some(index.length());
        Ok(value)
    }

    pub(crate) fn array_shift(&mut self, id: ObjectId, max_properties: usize) -> Result<Value> {
        let length = self.array_length_for_method(id, ARRAY_SHIFT_RECEIVER_ERROR)?;
        let Some(first_index) = length.first_index() else {
            return Ok(Value::Undefined);
        };

        let first_value = self.get(id, &first_index.key())?;
        let length_usize = length.to_usize()?;
        for index in 1..length_usize {
            self.move_array_index(id, index, index.saturating_sub(1), max_properties)?;
        }

        let Some(last_index) = length.previous_index() else {
            return Ok(first_value);
        };
        self.delete(id, &last_index.key())?;
        self.object_mut(id)?.array_length = Some(last_index.length());
        Ok(first_value)
    }

    pub(crate) fn array_unshift(
        &mut self,
        id: ObjectId,
        values: Vec<Value>,
        max_properties: usize,
    ) -> Result<Value> {
        let length = self.array_length_for_method(id, ARRAY_UNSHIFT_RECEIVER_ERROR)?;
        let value_count = values.len();
        let new_length = length.add_usize(value_count)?;
        if value_count == 0 {
            return Ok(new_length.value());
        }

        let length_usize = length.to_usize()?;
        for offset in 0..length_usize {
            let from_index = length_usize.saturating_sub(offset).saturating_sub(1);
            let to_index = from_index
                .checked_add(value_count)
                .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
            self.move_array_index(id, from_index, to_index, max_properties)?;
        }

        for (index, value) in values.into_iter().enumerate() {
            let key = ArrayIndex::from_usize(index)?.key();
            self.set(id, key, value, max_properties)?;
        }
        self.object_mut(id)?.array_length = Some(new_length);
        Ok(new_length.value())
    }

    pub(crate) fn array_slice(
        &mut self,
        id: ObjectId,
        start: usize,
        end: usize,
        prototype: ObjectId,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        self.array_length_for_method(id, ARRAY_SLICE_RECEIVER_ERROR)?;
        let count = end
            .checked_sub(start)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        let result = self.create_array_with_length(count, prototype, max_objects)?;
        let Value::Object(result_id) = result else {
            return Err(Error::runtime("array slice result is not an object"));
        };

        for offset in 0..count {
            let source_index = start
                .checked_add(offset)
                .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
            let source_key = ArrayIndex::from_usize(source_index)?.key();
            if self.has(id, &source_key)? {
                let value = self.get(id, &source_key)?;
                let target_key = ArrayIndex::from_usize(offset)?.key();
                self.set(result_id, target_key, value, max_properties)?;
            }
        }
        Ok(Value::Object(result_id))
    }

    pub(crate) fn array_len(&self, id: ObjectId) -> Result<usize> {
        self.array_length_for_method(id, ARRAY_JOIN_RECEIVER_ERROR)?
            .to_usize()
    }

    pub(crate) fn array_len_for_slice(&self, id: ObjectId) -> Result<usize> {
        self.array_length_for_method(id, ARRAY_SLICE_RECEIVER_ERROR)?
            .to_usize()
    }

    pub(crate) fn array_len_for_index_of(&self, id: ObjectId) -> Result<usize> {
        self.array_length_for_method(id, ARRAY_INDEX_OF_RECEIVER_ERROR)?
            .to_usize()
    }

    pub(crate) fn array_len_for_last_index_of(&self, id: ObjectId) -> Result<usize> {
        self.array_length_for_method(id, ARRAY_LAST_INDEX_OF_RECEIVER_ERROR)?
            .to_usize()
    }

    pub(crate) fn array_index_of(
        &self,
        id: ObjectId,
        search: &Value,
        start: usize,
    ) -> Result<Value> {
        let length = self
            .array_length_for_method(id, ARRAY_INDEX_OF_RECEIVER_ERROR)?
            .to_usize()?;
        if start >= length {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        }

        for index in start..length {
            let key = ArrayIndex::from_usize(index)?.key();
            if self.has(id, &key)? {
                let value = self.get(id, &key)?;
                if &value == search {
                    return Self::array_index_value(index);
                }
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(crate) fn array_last_index_of(
        &self,
        id: ObjectId,
        search: &Value,
        start: Option<usize>,
    ) -> Result<Value> {
        self.array_length_for_method(id, ARRAY_LAST_INDEX_OF_RECEIVER_ERROR)?;
        let Some(start) = start else {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        };

        for index in (0..=start).rev() {
            let key = ArrayIndex::from_usize(index)?.key();
            if self.has(id, &key)? {
                let value = self.get(id, &key)?;
                if &value == search {
                    return Self::array_index_value(index);
                }
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(crate) fn array_get_index(&self, id: ObjectId, index: usize) -> Result<Value> {
        let object = self.object(id)?;
        if object.array_length.is_none() {
            return Err(Error::runtime(ARRAY_JOIN_RECEIVER_ERROR));
        }
        let index = ArrayIndex::from_usize(index)?;
        self.get(id, &index.key())
    }

    fn array_length_for_method(&self, id: ObjectId, error: &str) -> Result<ArrayLength> {
        let object = self.object(id)?;
        object.array_length.ok_or_else(|| Error::runtime(error))
    }

    fn move_array_index(
        &mut self,
        id: ObjectId,
        from_index: usize,
        to_index: usize,
        max_properties: usize,
    ) -> Result<()> {
        let from_key = ArrayIndex::from_usize(from_index)?.key();
        let to_key = ArrayIndex::from_usize(to_index)?.key();
        if self.has(id, &from_key)? {
            let value = self.get(id, &from_key)?;
            return self.set(id, to_key, value, max_properties);
        }
        self.delete(id, &to_key)?;
        Ok(())
    }

    fn array_index_value(index: usize) -> Result<Value> {
        let index = u32::try_from(index).map_err(|_| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        Ok(Value::Number(f64::from(index)))
    }
}

impl ArrayLength {
    fn to_usize(self) -> Result<usize> {
        usize::try_from(self.0).map_err(|_| Error::limit("array length exceeded supported range"))
    }

    fn add_usize(self, value: usize) -> Result<Self> {
        let value = u32::try_from(value)
            .map_err(|_| Error::limit("array length exceeded supported range"))?;
        self.0
            .checked_add(value)
            .map(Self)
            .ok_or_else(|| Error::limit("array length exceeded supported range"))
    }

    fn index(self) -> Result<ArrayIndex> {
        ArrayIndex::from_u32(self.0)
    }

    const fn first_index(self) -> Option<ArrayIndex> {
        if self.0 == 0 {
            return None;
        }
        Some(ArrayIndex(0))
    }

    const fn previous_index(self) -> Option<ArrayIndex> {
        if self.0 == 0 {
            return None;
        }
        Some(ArrayIndex(self.0 - 1))
    }
}
