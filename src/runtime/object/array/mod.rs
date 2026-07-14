use crate::{
    error::{Error, Result},
    runtime::abstract_operations::{same_value_zero, strict_equality},
    value::{ObjectId, Value},
};

mod bulk;
mod bytecode;
mod fast;
mod front;
mod index;
mod length;
mod search;
mod sort;
mod storage;

pub(in crate::runtime) use index::ArrayIndex;
pub(super) use index::ArrayLength;
pub(super) use storage::ArrayStorage;

use super::{ARRAY_INDEX_LIMIT_ERROR, Object, ObjectHeap, ObjectProperty, ShapeTable};
const ARRAY_JOIN_RECEIVER_ERROR: &str = "Array.prototype.join requires an array receiver";
const ARRAY_POP_RECEIVER_ERROR: &str = "Array.prototype.pop requires an array receiver";
const ARRAY_PUSH_RECEIVER_ERROR: &str = "Array.prototype.push requires an array receiver";
const ARRAY_REVERSE_RECEIVER_ERROR: &str = "Array.prototype.reverse requires an array receiver";
const INDEX_NOT_FOUND: f64 = -1.0;

impl ObjectHeap {
    pub(crate) fn array_push(
        &mut self,
        id: ObjectId,
        values: &[Value],
        max_properties: usize,
    ) -> Result<Value> {
        let mut length = self.array_length_for_method(id, ARRAY_PUSH_RECEIVER_ERROR)?;
        let new_length = length.add_usize(values.len())?;
        if values.is_empty() {
            return Ok(new_length.value());
        }

        let length_usize = length.to_usize()?;
        if self
            .object(id)?
            .packed_array_properties(length_usize)
            .is_some()
        {
            let before = self.object(id)?.structure_snapshot();
            {
                let object = self.object_mut(id)?;
                object.append_packed_default_value_iter(
                    values.iter().cloned(),
                    values.len(),
                    max_properties,
                )?;
                object.array_length = Some(new_length);
            }
            self.bump_if_structure_changed(id, &before)?;
            return Ok(new_length.value());
        }

        for value in values {
            let index = length.index()?;
            self.set_array_index(id, index, value.clone(), max_properties)?;
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
        let length_usize = length.to_usize()?;

        if self
            .object(id)?
            .packed_array_properties(length_usize)
            .is_some()
        {
            let before = self.object(id)?.structure_snapshot();
            if let Some(property) = self
                .object_mut(id)?
                .pop_packed_for_len_if_configurable(length_usize)?
            {
                self.object_mut(id)?.array_length = Some(index.length());
                self.bump_if_structure_changed(id, &before)?;
                return Ok(property.value());
            }
        }

        let value = self
            .object(id)?
            .get_own_array_index(&self.shapes, index)?
            .unwrap_or(Value::Undefined);
        self.delete_array_index(id, index)?;
        self.object_mut(id)?.array_length = Some(index.length());
        Ok(value)
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
            .reverse_dense_for_len_if_default(length)
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

    pub(crate) fn array_len_if_array(&self, id: ObjectId) -> Result<Option<usize>> {
        let Some(length) = self.array_length_if_array(id)? else {
            return Ok(None);
        };
        length.to_usize().map(Some)
    }

    pub(crate) fn array_length_value_if_array(&self, id: ObjectId) -> Result<Option<Value>> {
        Ok(self.array_length_if_array(id)?.map(ArrayLength::value))
    }

    pub(crate) fn array_index_of(
        &self,
        id: ObjectId,
        length: usize,
        search: &Value,
        start: usize,
    ) -> Result<Value> {
        if start >= length {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        }
        if let Some(properties) = self.packed_array_properties(id, length)? {
            if let Value::Number(search) = search {
                return Self::packed_array_index_of_number(properties, *search, start);
            }
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
                && strict_equality(&value, search)
            {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(crate) fn array_includes(
        &self,
        id: ObjectId,
        length: usize,
        search: &Value,
        start: usize,
    ) -> Result<Value> {
        if start >= length {
            return Ok(Value::Bool(false));
        }
        if let Some(properties) = self.packed_array_properties(id, length)? {
            if let Value::Number(search) = search {
                return Ok(Self::packed_array_includes_number(
                    properties, *search, start,
                ));
            }
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
            if same_value_zero(&value, search) {
                return Ok(Value::Bool(true));
            }
        }
        Ok(Value::Bool(false))
    }

    pub(crate) fn array_last_index_of(
        &self,
        id: ObjectId,
        length: usize,
        search: &Value,
        start: Option<usize>,
    ) -> Result<Value> {
        let Some(start) = start else {
            return Ok(Value::Number(INDEX_NOT_FOUND));
        };
        if let Some(properties) = self.packed_array_properties(id, length)? {
            if let Value::Number(search) = search {
                return Self::packed_array_last_index_of_number(properties, *search, start);
            }
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
                && strict_equality(&value, search)
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
        if properties.iter().any(|property| {
            property
                .data_value_ref()
                .is_some_and(|value| !crate::runtime::abstract_operations::is_primitive(value))
        }) {
            return Ok(None);
        }
        let mut joined =
            Self::join_string_with_separator_capacity(length, separator.len(), max_string_len)?;
        for (index, property) in properties.iter().enumerate() {
            if index > 0 {
                Self::push_join_text(&mut joined, separator, max_string_len)?;
            }
            let value = property.data_value_ref().unwrap_or(&Value::Undefined);
            Self::push_join_value_text(&mut joined, value, max_string_len)?;
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
        self.bump_if_structure_changed(id, &before)
    }

    pub(super) fn delete_array_index(&mut self, id: ObjectId, index: ArrayIndex) -> Result<bool> {
        let before = self.object(id)?.structure_snapshot();
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        let deleted = object.delete_array_index(index, shapes)?;
        self.bump_if_structure_changed(id, &before)?;
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
            current = object.ordinary_prototype_id();
        }
        Ok(None)
    }

    fn packed_array_properties(
        &self,
        id: ObjectId,
        length: usize,
    ) -> Result<Option<&[ObjectProperty]>> {
        let Some(properties) = self.object(id)?.packed_array_properties(length) else {
            return Ok(None);
        };
        if properties
            .iter()
            .any(|property| property.accessor().is_some())
        {
            return Ok(None);
        }
        Ok(Some(properties))
    }

    pub(crate) fn packed_array_values_if_array(&self, id: ObjectId) -> Result<Option<Vec<Value>>> {
        let Some(length) = self.array_len_if_array(id)? else {
            return Ok(None);
        };
        let Some(properties) = self.object(id)?.packed_array_properties(length) else {
            return Ok(None);
        };
        let Some(values) = properties
            .iter()
            .map(ObjectProperty::data_value_ref)
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(None);
        };
        Ok(Some(values.into_iter().cloned().collect()))
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
        if self.array_length.is_some() && self.has_array_element(index) {
            return self.delete_array_element(index);
        }
        let Some(key) = self.array_storage.sparse_key(index) else {
            return Ok(true);
        };
        if self
            .named_property(shapes, key)?
            .is_some_and(|property| !property.is_configurable())
        {
            return Ok(false);
        }
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
