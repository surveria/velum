use crate::{
    error::{Error, Result},
    value::{ObjectId, Value},
};

mod bulk;
mod bytecode;
mod fast;
mod front;
mod index;
mod storage;

pub(super) use index::{ArrayIndex, ArrayLength};
pub(super) use storage::ArrayStorage;

use super::{ARRAY_INDEX_LIMIT_ERROR, Object, ObjectHeap, ObjectProperty, ShapeTable};
use fast::ArrayCopyLimits;

const ARRAY_CONCAT_RECEIVER_ERROR: &str = "Array.prototype.concat requires an array receiver";
const ARRAY_INCLUDES_RECEIVER_ERROR: &str = "Array.prototype.includes requires an array receiver";
const ARRAY_INDEX_OF_RECEIVER_ERROR: &str = "Array.prototype.indexOf requires an array receiver";
const ARRAY_JOIN_RECEIVER_ERROR: &str = "Array.prototype.join requires an array receiver";
const ARRAY_LAST_INDEX_OF_RECEIVER_ERROR: &str =
    "Array.prototype.lastIndexOf requires an array receiver";
const ARRAY_POP_RECEIVER_ERROR: &str = "Array.prototype.pop requires an array receiver";
const ARRAY_PUSH_RECEIVER_ERROR: &str = "Array.prototype.push requires an array receiver";
const ARRAY_REVERSE_RECEIVER_ERROR: &str = "Array.prototype.reverse requires an array receiver";
const ARRAY_SLICE_RECEIVER_ERROR: &str = "Array.prototype.slice requires an array receiver";
const INDEX_NOT_FOUND: f64 = -1.0;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ArrayCopyProgress {
    pub(super) next_index: usize,
    pub(super) source_index: usize,
}

impl ObjectHeap {
    pub(crate) fn array_push(
        &mut self,
        id: ObjectId,
        values: Vec<Value>,
        max_properties: usize,
    ) -> Result<Value> {
        let mut length = self.array_length_for_method(id, ARRAY_PUSH_RECEIVER_ERROR)?;

        for value in values {
            let index = length.index()?;
            self.set_array_index(id, index, value, max_properties)?;
            length = index.next_length()?;
        }
        self.object_mut(id)?.array_length = Some(length);
        Ok(length.value())
    }

    pub(crate) fn array_pop(&mut self, id: ObjectId) -> Result<Value> {
        let length = self.array_length_for_method(id, ARRAY_POP_RECEIVER_ERROR)?;
        let Some(index) = length.previous_index() else {
            return Ok(Value::Undefined);
        };

        let value = self
            .object(id)?
            .get_own_array_index(&self.shapes, index)?
            .unwrap_or(Value::Undefined);
        self.delete_array_index(id, index)?;
        self.object_mut(id)?.array_length = Some(index.length());
        Ok(value)
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
        let length = self
            .array_length_for_method(id, ARRAY_SLICE_RECEIVER_ERROR)?
            .to_usize()?;
        let count = end
            .checked_sub(start)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        if let Some(values) = self.packed_array_value_range(id, length, start, count)? {
            return self.create_array(values, prototype, max_objects, max_properties);
        }
        if let Some(value) = self.holey_array_slice_without_indexed_prototype(
            id,
            length,
            start,
            count,
            prototype,
            ArrayCopyLimits::new(max_objects, max_properties),
        )? {
            return Ok(value);
        }

        let result = self.create_array_with_length(count, prototype, max_objects)?;
        let Value::Object(result_id) = result else {
            return Err(Error::runtime("array slice result is not an object"));
        };

        for offset in 0..count {
            let source_index = start
                .checked_add(offset)
                .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
            let source_index = ArrayIndex::from_usize(source_index)?;
            if let Some(value) = self.array_property_value_by_index(id, source_index)? {
                let target_index = ArrayIndex::from_usize(offset)?;
                self.set_array_index(result_id, target_index, value, max_properties)?;
            }
        }
        Ok(Value::Object(result_id))
    }

    pub(crate) fn array_reverse(&mut self, id: ObjectId, max_properties: usize) -> Result<Value> {
        let length = self
            .array_length_for_method(id, ARRAY_REVERSE_RECEIVER_ERROR)?
            .to_usize()?;
        if length <= 1 {
            return Ok(Value::Object(id));
        }
        if self
            .object_mut(id)?
            .array_storage
            .reverse_packed_for_len_if_default(length)
        {
            return Ok(Value::Object(id));
        }

        let middle = length / 2;
        for lower_index in 0..middle {
            let upper_index = length
                .checked_sub(lower_index)
                .and_then(|index| index.checked_sub(1))
                .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
            self.reverse_array_pair(id, lower_index, upper_index, max_properties)?;
        }
        Ok(Value::Object(id))
    }

    pub(crate) fn array_concat(
        &mut self,
        id: ObjectId,
        values: Vec<Value>,
        prototype: ObjectId,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let this_length = self.array_length_for_method(id, ARRAY_CONCAT_RECEIVER_ERROR)?;
        if let Some(values) = self.packed_concat_values(id, this_length, &values)? {
            return self.create_array(values, prototype, max_objects, max_properties);
        }
        let result = self.create_array_with_length(0, prototype, max_objects)?;
        let Value::Object(result_id) = result else {
            return Err(Error::runtime("array concat result is not an object"));
        };

        let mut next_index = 0;
        self.concat_array_source(result_id, &mut next_index, id, this_length, max_properties)?;

        for value in values {
            if let Value::Object(source_id) = &value
                && let Some(length) = self.array_length_if_array(*source_id)?
            {
                self.concat_array_source(
                    result_id,
                    &mut next_index,
                    *source_id,
                    length,
                    max_properties,
                )?;
            } else {
                self.concat_single_value(result_id, &mut next_index, value, max_properties)?;
            }
        }
        self.object_mut(result_id)?.array_length = Some(ArrayLength::from_usize(next_index)?);
        Ok(Value::Object(result_id))
    }

    pub(crate) fn array_len(&self, id: ObjectId) -> Result<usize> {
        self.array_length_for_method(id, ARRAY_JOIN_RECEIVER_ERROR)?
            .to_usize()
    }

    pub(crate) fn array_len_if_array(&self, id: ObjectId) -> Result<Option<usize>> {
        let Some(length) = self.array_length_if_array(id)? else {
            return Ok(None);
        };
        length.to_usize().map(Some)
    }

    pub(crate) fn array_length_value_if_array(&self, id: ObjectId) -> Result<Option<Value>> {
        Ok(self.array_length_if_array(id)?.map(ArrayLength::value))
    }

    pub(crate) fn array_len_for_slice(&self, id: ObjectId) -> Result<usize> {
        self.array_length_for_method(id, ARRAY_SLICE_RECEIVER_ERROR)?
            .to_usize()
    }

    pub(crate) fn array_len_for_index_of(&self, id: ObjectId) -> Result<usize> {
        self.array_length_for_method(id, ARRAY_INDEX_OF_RECEIVER_ERROR)?
            .to_usize()
    }

    pub(crate) fn array_len_for_includes(&self, id: ObjectId) -> Result<usize> {
        self.array_length_for_method(id, ARRAY_INCLUDES_RECEIVER_ERROR)?
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
        if let Some(properties) = self.packed_array_properties(id, length)? {
            return Self::packed_array_index_of(properties, search, start);
        }
        if let Some(value) =
            self.holey_array_index_of_without_indexed_prototype(id, length, search, start)?
        {
            return Ok(value);
        }

        for position in start..length {
            let index = ArrayIndex::from_usize(position)?;
            if let Some(value) = self.array_property_value_by_index(id, index)?
                && &value == search
            {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(crate) fn array_includes(
        &self,
        id: ObjectId,
        search: &Value,
        start: usize,
    ) -> Result<Value> {
        let length = self
            .array_length_for_method(id, ARRAY_INCLUDES_RECEIVER_ERROR)?
            .to_usize()?;
        if start >= length {
            return Ok(Value::Bool(false));
        }
        if let Some(properties) = self.packed_array_properties(id, length)? {
            return Ok(Self::packed_array_includes(properties, search, start));
        }
        if let Some(value) =
            self.holey_array_includes_without_indexed_prototype(id, length, search, start)?
        {
            return Ok(value);
        }

        for index in start..length {
            let index = ArrayIndex::from_usize(index)?;
            let value = self.get_array_index(id, index)?;
            if Self::same_value_zero(&value, search) {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    pub(crate) fn array_last_index_of(
        &self,
        id: ObjectId,
        search: &Value,
        start: Option<usize>,
    ) -> Result<Value> {
        let length = self
            .array_length_for_method(id, ARRAY_LAST_INDEX_OF_RECEIVER_ERROR)?
            .to_usize()?;
        let Some(start) = start else {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        };
        if let Some(properties) = self.packed_array_properties(id, length)? {
            return Self::packed_array_last_index_of(properties, search, start);
        }
        if let Some(value) =
            self.holey_array_last_index_of_without_indexed_prototype(id, length, search, start)?
        {
            return Ok(value);
        }

        for position in (0..=start).rev() {
            let index = ArrayIndex::from_usize(position)?;
            if let Some(value) = self.array_property_value_by_index(id, index)?
                && &value == search
            {
                return Self::array_index_value(position);
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
        self.get_array_index(id, index)
    }

    pub(crate) fn packed_array_join(
        &self,
        id: ObjectId,
        separator: &str,
        max_string_len: usize,
    ) -> Result<Option<String>> {
        let length = self.array_length_for_method(id, ARRAY_JOIN_RECEIVER_ERROR)?;
        let length = length.to_usize()?;
        let Some(properties) = self.packed_array_properties(id, length)? else {
            return self.holey_array_join_without_indexed_prototype(
                id,
                length,
                separator,
                max_string_len,
            );
        };
        let mut joined = String::new();
        for (index, property) in properties.iter().enumerate() {
            if index > 0 {
                Self::push_join_text(&mut joined, separator, max_string_len)?;
            }
            let value = property.value();
            let text = Self::array_join_element_text(&value);
            Self::push_join_text(&mut joined, &text, max_string_len)?;
        }
        Ok(Some(joined))
    }

    pub(super) fn get_array_index(&self, id: ObjectId, index: ArrayIndex) -> Result<Value> {
        if let Some(value) = self.array_property_value_by_index(id, index)? {
            return Ok(value);
        }
        Ok(Value::Undefined)
    }

    pub(super) fn set_array_index(
        &mut self,
        id: ObjectId,
        index: ArrayIndex,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        let before = self.object(id)?.structure_snapshot();
        let object = self.object_mut(id)?;
        if object.array_length.is_none() {
            return Err(Error::runtime("array index receiver is not an array"));
        }
        object.set_array_index(index, value, max_properties)?;
        self.bump_if_structure_changed(id, before)
    }

    pub(super) fn delete_array_index(&mut self, id: ObjectId, index: ArrayIndex) -> Result<bool> {
        let before = self.object(id)?.structure_snapshot();
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        let deleted = object.delete_array_index(index, shapes)?;
        self.bump_if_structure_changed(id, before)?;
        Ok(deleted)
    }

    fn array_property_value_by_index(
        &self,
        id: ObjectId,
        index: ArrayIndex,
    ) -> Result<Option<Value>> {
        let mut current = Some(id);
        let mut visited = Vec::new();
        while let Some(current_id) = current {
            if visited.contains(&current_id) {
                return Err(Error::runtime("prototype cycle detected"));
            }
            visited.push(current_id);
            let object = self.object(current_id)?;
            if let Some(value) = object.get_own_array_index(&self.shapes, index)? {
                return Ok(Some(value));
            }
            current = object.prototype;
        }
        Ok(None)
    }

    fn packed_array_properties(
        &self,
        id: ObjectId,
        length: usize,
    ) -> Result<Option<&[ObjectProperty]>> {
        Ok(self.object(id)?.packed_array_properties(length))
    }

    pub(super) fn array_length_for_method(&self, id: ObjectId, error: &str) -> Result<ArrayLength> {
        let object = self.object(id)?;
        object.array_length.ok_or_else(|| Error::runtime(error))
    }

    pub(super) fn move_array_index(
        &mut self,
        id: ObjectId,
        from_index: usize,
        to_index: usize,
        max_properties: usize,
    ) -> Result<()> {
        let from_index = ArrayIndex::from_usize(from_index)?;
        let to_index = ArrayIndex::from_usize(to_index)?;
        if let Some(value) = self.array_property_value_by_index(id, from_index)? {
            return self.set_array_index(id, to_index, value, max_properties);
        }
        self.delete_array_index(id, to_index)?;
        Ok(())
    }

    fn reverse_array_pair(
        &mut self,
        id: ObjectId,
        lower_index: usize,
        upper_index: usize,
        max_properties: usize,
    ) -> Result<()> {
        let lower_index = ArrayIndex::from_usize(lower_index)?;
        let upper_index = ArrayIndex::from_usize(upper_index)?;
        let lower_value = self.array_property_value_by_index(id, lower_index)?;
        let upper_value = self.array_property_value_by_index(id, upper_index)?;

        match (lower_value, upper_value) {
            (Some(lower_value), Some(upper_value)) => {
                self.set_array_index(id, lower_index, upper_value, max_properties)?;
                self.set_array_index(id, upper_index, lower_value, max_properties)?;
            }
            (None, Some(upper_value)) => {
                self.set_array_index(id, lower_index, upper_value, max_properties)?;
                self.delete_array_index(id, upper_index)?;
            }
            (Some(lower_value), None) => {
                self.delete_array_index(id, lower_index)?;
                self.set_array_index(id, upper_index, lower_value, max_properties)?;
            }
            (None, None) => {}
        }
        Ok(())
    }

    pub(super) fn array_length_if_array(&self, id: ObjectId) -> Result<Option<ArrayLength>> {
        let object = self.object(id)?;
        Ok(object.array_length)
    }

    fn concat_array_source(
        &mut self,
        result_id: ObjectId,
        next_index: &mut usize,
        source_id: ObjectId,
        length: ArrayLength,
        max_properties: usize,
    ) -> Result<()> {
        let length = length.to_usize()?;
        let progress = self.concat_own_array_prefix(
            result_id,
            *next_index,
            source_id,
            length,
            max_properties,
        )?;
        *next_index = progress.next_index;

        for source_index in progress.source_index..length {
            let source_index = ArrayIndex::from_usize(source_index)?;
            if let Some(value) = self.array_property_value_by_index(source_id, source_index)? {
                self.set_concat_result_index(result_id, *next_index, value, max_properties)?;
            }
            *next_index = Self::next_concat_index(*next_index)?;
        }
        Ok(())
    }

    fn concat_own_array_prefix(
        &mut self,
        result_id: ObjectId,
        start_index: usize,
        source_id: ObjectId,
        length: usize,
        max_properties: usize,
    ) -> Result<ArrayCopyProgress> {
        if let Some(progress) = self.holey_concat_array_prefix_without_indexed_prototype(
            result_id,
            start_index,
            source_id,
            length,
            max_properties,
        )? {
            return Ok(progress);
        }

        let shapes = &self.shapes;
        let (source, result) =
            Self::object_pair_for_concat(self.objects.as_mut_slice(), source_id, result_id)?;
        if result.array_length.is_none() {
            return Err(Error::runtime("array concat result is not an array"));
        }
        if let Some(properties) = source.packed_array_properties(length) {
            let mut next_index = start_index;
            for property in properties {
                let target_index = ArrayIndex::from_usize(next_index)?;
                result.set_array_index(target_index, property.value(), max_properties)?;
                next_index = Self::next_concat_index(next_index)?;
            }
            return Ok(ArrayCopyProgress {
                next_index,
                source_index: length,
            });
        }
        let mut next_index = start_index;
        for source_position in 0..length {
            let source_index = ArrayIndex::from_usize(source_position)?;
            let Some(value) = source.get_own_array_index(shapes, source_index)? else {
                return Ok(ArrayCopyProgress {
                    next_index,
                    source_index: source_position,
                });
            };
            let target_index = ArrayIndex::from_usize(next_index)?;
            result.set_array_index(target_index, value, max_properties)?;
            next_index = Self::next_concat_index(next_index)?;
        }
        Ok(ArrayCopyProgress {
            next_index,
            source_index: length,
        })
    }

    fn object_pair_for_concat(
        objects: &mut [Object],
        source_id: ObjectId,
        result_id: ObjectId,
    ) -> Result<(&Object, &mut Object)> {
        if source_id == result_id {
            return Err(Error::runtime("array concat source and result alias"));
        }
        let source_index = source_id.index();
        let result_index = result_id.index();
        if source_index >= objects.len() || result_index >= objects.len() {
            return Err(Error::runtime("object id is not defined"));
        }
        if source_index < result_index {
            let (left, right) = objects.split_at_mut(result_index);
            let source = left
                .get(source_index)
                .ok_or_else(|| Error::runtime("object id is not defined"))?;
            let result = right
                .first_mut()
                .ok_or_else(|| Error::runtime("object id is not defined"))?;
            return Ok((source, result));
        }

        let (left, right) = objects.split_at_mut(source_index);
        let result = left
            .get_mut(result_index)
            .ok_or_else(|| Error::runtime("object id is not defined"))?;
        let source = right
            .first()
            .ok_or_else(|| Error::runtime("object id is not defined"))?;
        Ok((source, result))
    }

    fn concat_single_value(
        &mut self,
        result_id: ObjectId,
        next_index: &mut usize,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        self.set_concat_result_index(result_id, *next_index, value, max_properties)?;
        *next_index = Self::next_concat_index(*next_index)?;
        Ok(())
    }

    fn set_concat_result_index(
        &mut self,
        result_id: ObjectId,
        index: usize,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        let index = ArrayIndex::from_usize(index)?;
        let object = self.object_mut(result_id)?;
        if object.array_length.is_none() {
            return Err(Error::runtime("array concat result is not an array"));
        }
        object.set_array_index(index, value, max_properties)
    }

    fn next_concat_index(index: usize) -> Result<usize> {
        index
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))
    }

    pub(super) fn array_index_value(index: usize) -> Result<Value> {
        let index = u32::try_from(index).map_err(|_| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        Ok(Value::Number(f64::from(index)))
    }

    pub(super) fn same_value_zero(left: &Value, right: &Value) -> bool {
        match (left, right) {
            (Value::Number(left), Value::Number(right)) => {
                (left.to_bits() == right.to_bits())
                    || (left.is_nan() && right.is_nan())
                    || (Self::number_is_zero(*left) && Self::number_is_zero(*right))
            }
            _ => left == right,
        }
    }

    const fn number_is_zero(value: f64) -> bool {
        matches!(value.classify(), std::num::FpCategory::Zero)
    }

    pub(super) fn array_join_element_text(value: &Value) -> String {
        match value {
            Value::Undefined | Value::Null => String::new(),
            _ => value.display_for_concat(),
        }
    }

    pub(super) fn push_join_text(
        joined: &mut String,
        text: &str,
        max_string_len: usize,
    ) -> Result<()> {
        let length = joined
            .len()
            .checked_add(text.len())
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
        if length > max_string_len {
            return Err(Error::limit(format!(
                "string length {length} exceeded {max_string_len}"
            )));
        }
        joined.push_str(text);
        Ok(())
    }

    fn packed_array_index_of(
        properties: &[ObjectProperty],
        search: &Value,
        start: usize,
    ) -> Result<Value> {
        for (position, property) in properties.iter().enumerate().skip(start) {
            let value = property.value();
            if &value == search {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    fn packed_array_includes(properties: &[ObjectProperty], search: &Value, start: usize) -> Value {
        for property in properties.iter().skip(start) {
            if Self::same_value_zero(&property.value(), search) {
                return Value::Bool(true);
            }
        }
        Value::Bool(false)
    }

    fn packed_array_last_index_of(
        properties: &[ObjectProperty],
        search: &Value,
        start: usize,
    ) -> Result<Value> {
        if properties.is_empty() {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        }
        let upper = start.min(properties.len().saturating_sub(1));
        let count = upper
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        for (position, property) in properties.iter().enumerate().take(count).rev() {
            let value = property.value();
            if &value == search {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }
}

impl Object {
    pub(super) fn packed_array_properties(&self, length: usize) -> Option<&[ObjectProperty]> {
        self.array_length?;
        self.array_storage.packed_properties_for_len(length)
    }

    fn get_own_array_index(&self, shapes: &ShapeTable, index: ArrayIndex) -> Result<Option<Value>> {
        if self.array_length.is_some()
            && let Some(value) = self.array_element_value(index)
        {
            return Ok(Some(value));
        }
        let Some(key) = self.array_storage.sparse_key(index) else {
            return Ok(None);
        };
        self.named_property(shapes, key)
            .map(|property| property.map(super::ObjectProperty::value))
    }

    pub(super) fn set_array_index(
        &mut self,
        index: ArrayIndex,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        self.set_array_property_value(index, None, value, None, None, max_properties)?;
        self.extend_array_length(index)
    }

    fn delete_array_index(&mut self, index: ArrayIndex, shapes: &mut ShapeTable) -> Result<bool> {
        if self.array_length.is_some() && self.delete_array_element(index) {
            return Ok(true);
        }
        let Some(key) = self.array_storage.sparse_key(index) else {
            return Ok(true);
        };
        let removed = self.remove_named_property(shapes, key)?;
        self.array_storage.remove_sparse_key(index);
        if let Some(property) = removed
            && property.is_enumerable()
        {
            self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
        }
        Ok(true)
    }
}

impl ArrayLength {
    pub(super) fn to_usize(self) -> Result<usize> {
        usize::try_from(self.0).map_err(|_| Error::limit("array length exceeded supported range"))
    }

    pub(super) fn add_usize(self, value: usize) -> Result<Self> {
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

    pub(super) const fn first_index(self) -> Option<ArrayIndex> {
        if self.0 == 0 {
            return None;
        }
        Some(ArrayIndex(0))
    }

    pub(super) const fn previous_index(self) -> Option<ArrayIndex> {
        if self.0 == 0 {
            return None;
        }
        Some(ArrayIndex(self.0 - 1))
    }
}
