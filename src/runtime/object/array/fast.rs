use crate::{
    error::{Error, Result},
    runtime::abstract_operations::{
        number_same_value_zero, number_strict_equality, same_value_zero, strict_equality,
    },
    value::{ObjectId, Value},
};

use super::super::property::PrototypeTraversalBudget;
use super::{ARRAY_INDEX_LIMIT_ERROR, ArrayIndex, Object, ObjectHeap, ObjectProperty};

impl ObjectHeap {
    pub(in crate::runtime::object) fn holey_array_includes_without_indexed_prototype(
        &self,
        id: ObjectId,
        length: usize,
        search: &Value,
        start: usize,
    ) -> Result<Option<Value>> {
        let Some(properties) = self.holey_properties_without_indexed_prototype(
            id,
            length,
            start,
            length
                .checked_sub(start)
                .ok_or_else(|| Error::runtime("array includes start exceeded array length"))?,
        )?
        else {
            return Ok(None);
        };

        if let Value::Number(search) = search {
            return Ok(Some(Self::holey_array_includes_number(
                properties, *search, start,
            )));
        }
        if matches!(search, Value::Undefined) {
            return Ok(Some(Self::holey_array_includes_undefined(
                properties, start,
            )));
        }

        for property in properties.iter().skip(start) {
            if property.as_ref().map_or_else(
                || same_value_zero(&Value::Undefined, search),
                |property| {
                    property
                        .data_value_ref()
                        .is_some_and(|value| same_value_zero(value, search))
                },
            ) {
                return Ok(Some(Value::Bool(true)));
            }
        }
        Ok(Some(Value::Bool(false)))
    }

    pub(in crate::runtime::object) fn holey_array_index_of_without_indexed_prototype(
        &self,
        id: ObjectId,
        length: usize,
        search: &Value,
        start: usize,
    ) -> Result<Option<Value>> {
        let Some(properties) = self.holey_properties_without_indexed_prototype(
            id,
            length,
            start,
            length
                .checked_sub(start)
                .ok_or_else(|| Error::runtime("array indexOf start exceeded array length"))?,
        )?
        else {
            return Ok(None);
        };

        if let Value::Number(search) = search {
            return Self::holey_array_index_of_number(properties, *search, start).map(Some);
        }

        for (position, property) in properties.iter().enumerate().skip(start) {
            if let Some(property) = property
                && property
                    .data_value_ref()
                    .is_some_and(|value| strict_equality(value, search))
            {
                return Self::array_index_value(position).map(Some);
            }
        }
        Ok(Some(Value::Number(INDEX_NOT_FOUND)))
    }

    pub(in crate::runtime::object) fn holey_array_last_index_of_without_indexed_prototype(
        &self,
        id: ObjectId,
        length: usize,
        search: &Value,
        start: usize,
    ) -> Result<Option<Value>> {
        let count = start
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
        let Some(properties) =
            self.holey_properties_without_indexed_prototype(id, length, 0, count)?
        else {
            return Ok(None);
        };

        if let Value::Number(search) = search {
            return Self::holey_array_last_index_of_number(properties, *search, count).map(Some);
        }

        for (position, property) in properties.iter().enumerate().take(count).rev() {
            if let Some(property) = property
                && property
                    .data_value_ref()
                    .is_some_and(|value| strict_equality(value, search))
            {
                return Self::array_index_value(position).map(Some);
            }
        }
        Ok(Some(Value::Number(INDEX_NOT_FOUND)))
    }

    fn holey_array_includes_number(
        properties: &[Option<ObjectProperty>],
        search: f64,
        start: usize,
    ) -> Value {
        for property in properties.iter().skip(start).flatten() {
            if let Some(Value::Number(value)) = property.data_value_ref()
                && number_same_value_zero(*value, search)
            {
                return Value::Bool(true);
            }
        }
        Value::Bool(false)
    }

    fn holey_array_includes_undefined(
        properties: &[Option<ObjectProperty>],
        start: usize,
    ) -> Value {
        for property in properties.iter().skip(start) {
            match property {
                None => return Value::Bool(true),
                Some(property) if matches!(property.data_value_ref(), Some(Value::Undefined)) => {
                    return Value::Bool(true);
                }
                Some(_) => {}
            }
        }
        Value::Bool(false)
    }

    fn holey_array_index_of_number(
        properties: &[Option<ObjectProperty>],
        search: f64,
        start: usize,
    ) -> Result<Value> {
        for (position, property) in properties.iter().enumerate().skip(start) {
            if let Some(property) = property
                && let Some(Value::Number(value)) = property.data_value_ref()
                && number_strict_equality(*value, search)
            {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    fn holey_array_last_index_of_number(
        properties: &[Option<ObjectProperty>],
        search: f64,
        count: usize,
    ) -> Result<Value> {
        for (position, property) in properties.iter().enumerate().take(count).rev() {
            if let Some(property) = property
                && let Some(Value::Number(value)) = property.data_value_ref()
                && number_strict_equality(*value, search)
            {
                return Self::array_index_value(position);
            }
        }
        Ok(Value::Number(INDEX_NOT_FOUND))
    }

    pub(in crate::runtime::object) fn holey_array_join_without_indexed_prototype(
        &self,
        id: ObjectId,
        length: usize,
        separator: &str,
        max_string_len: usize,
    ) -> Result<Option<String>> {
        let Some(properties) =
            self.holey_properties_without_indexed_prototype(id, length, 0, length)?
        else {
            return Ok(None);
        };
        if properties.iter().flatten().any(|property| {
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
            let Some(property) = property else {
                continue;
            };
            let value = property.data_value_ref().unwrap_or(&Value::Undefined);
            Self::push_join_value_text(&mut joined, value, max_string_len)?;
        }
        Ok(Some(joined))
    }

    pub(in crate::runtime::object) fn holey_array_slice_without_indexed_prototype(
        &mut self,
        id: ObjectId,
        length: usize,
        start: usize,
        count: usize,
        prototype: ObjectId,
        limits: ArrayCopyLimits,
    ) -> Result<Option<Value>> {
        let Some(values) = self.holey_values_without_indexed_prototype(id, length, start, count)?
        else {
            return Ok(None);
        };
        let result = self.create_array_with_length(count, prototype, limits.max_objects)?;
        let Value::Object(result_id) = result else {
            return Err(Error::runtime("array slice result is not an object"));
        };
        for (offset, value) in values {
            let target_index = ArrayIndex::from_usize(offset)?;
            self.object_mut(result_id)?.set_array_index(
                target_index,
                value,
                limits.max_properties,
            )?;
        }
        Ok(Some(Value::Object(result_id)))
    }

    fn holey_values_without_indexed_prototype(
        &self,
        id: ObjectId,
        length: usize,
        start: usize,
        count: usize,
    ) -> Result<Option<Vec<(usize, Value)>>> {
        let Some(properties) =
            self.holey_properties_without_indexed_prototype(id, length, start, count)?
        else {
            return Ok(None);
        };

        let mut values = Vec::new();
        for (offset, property) in properties.iter().skip(start).take(count).enumerate() {
            if let Some(property) = property {
                values.push((offset, property.value()));
            }
        }
        Ok(Some(values))
    }

    fn holey_properties_without_indexed_prototype(
        &self,
        id: ObjectId,
        length: usize,
        start: usize,
        count: usize,
    ) -> Result<Option<&[Option<ObjectProperty>]>> {
        if self.prototype_chain_has_array_index_in_range(id, start, count)? {
            return Ok(None);
        }
        Ok(self.object(id)?.holey_array_properties(length))
    }

    pub(in crate::runtime::object) fn prototype_chain_has_array_index_in_range(
        &self,
        id: ObjectId,
        start: usize,
        count: usize,
    ) -> Result<bool> {
        let Some(end) = start.checked_add(count) else {
            return Ok(true);
        };
        let object = self.object(id)?;
        let mut current = object.prototype;
        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if object.has_own_array_index_in_range(start, end)? {
                return Ok(true);
            }
            current = object.prototype;
        }
        Ok(false)
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::runtime::object) struct ArrayCopyLimits {
    pub(in crate::runtime::object) max_objects: usize,
    pub(in crate::runtime::object) max_properties: usize,
}

impl ArrayCopyLimits {
    pub(in crate::runtime::object) const fn new(max_objects: usize, max_properties: usize) -> Self {
        Self {
            max_objects,
            max_properties,
        }
    }
}

impl Object {
    fn holey_array_properties(&self, length: usize) -> Option<&[Option<ObjectProperty>]> {
        self.array_length?;
        self.array_storage.holey_properties_for_len(length)
    }

    fn has_own_array_index_in_range(&self, start: usize, end: usize) -> Result<bool> {
        if self.array_storage.has_sparse_key_in_range(start, end)? {
            return Ok(true);
        }
        if self.array_length.is_some() && self.array_storage.has_dense_property_in_range(start, end)
        {
            return Ok(true);
        }
        Ok(false)
    }
}

const INDEX_NOT_FOUND: f64 = -1.0;
