use crate::{
    error::{Error, Result},
    storage::string_heap::JsString,
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap, PropertyEnumerable, PropertyKey};

const STRING_LENGTH_PROPERTY: &str = "length";
const STRING_LENGTH_LIMIT_ERROR: &str = "string length exceeded supported object range";
const STRING_OBJECT_FIXED_PROPERTY_CAPACITY: usize = 1;

impl ObjectHeap {
    pub(crate) fn create_string_object(
        &mut self,
        value: JsString,
        prototype: ObjectId,
        length_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let length = string_character_count(value.as_str())?;
        let mut object = Object::string(value);
        object.prototype = Some(prototype);
        object.define(
            length_key,
            STRING_LENGTH_PROPERTY,
            Value::Number(length_to_value(length)?),
            PropertyEnumerable::No,
            &mut self.shapes,
            max_properties,
        )?;
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(crate) fn string_object_character(
        &self,
        id: ObjectId,
        property: &str,
    ) -> Result<Option<char>> {
        self.object(id)?.virtual_string_character(property)
    }
}

impl Object {
    pub(super) fn string(value: JsString) -> Self {
        Self {
            named_properties: Vec::with_capacity(STRING_OBJECT_FIXED_PROPERTY_CAPACITY),
            array_storage: super::ArrayStorage::new(),
            shape: super::ShapeId::root(),
            enumerable_property_count: 0,
            array_length: None,
            string_value: Some(value),
            prototype: None,
        }
    }

    pub(super) fn has_virtual_string_property(
        &self,
        property: super::PropertyLookup<'_>,
    ) -> Result<bool> {
        self.has_virtual_string_property_name(property.name())
    }

    pub(super) fn has_virtual_string_property_name(&self, property: &str) -> Result<bool> {
        self.virtual_string_character(property)
            .map(|value| value.is_some())
    }

    pub(super) fn virtual_string_character(&self, property: &str) -> Result<Option<char>> {
        let Some(value) = self.string_value.as_ref() else {
            return Ok(None);
        };
        let Some(index) = super::ArrayIndex::parse(property) else {
            return Ok(None);
        };
        let position = index.position()?;
        Ok(value.as_str().chars().nth(position))
    }

    pub(super) fn virtual_string_key_count(&self) -> usize {
        self.string_value
            .as_ref()
            .map_or(0, |value| value.as_str().chars().count())
    }

    pub(super) fn has_virtual_string_keys(&self) -> bool {
        self.string_value
            .as_ref()
            .is_some_and(|value| !value.as_str().is_empty())
    }

    pub(super) fn extend_virtual_string_keys(&self, keys: &mut Vec<String>) -> Result<()> {
        let Some(value) = self.string_value.as_ref() else {
            return Ok(());
        };
        let len = string_character_count(value.as_str())?;
        for index in 0..len {
            super::keys::push_unique_key(keys, index.to_string());
        }
        Ok(())
    }
}

fn string_character_count(value: &str) -> Result<usize> {
    let count = value.chars().count();
    u32::try_from(count)
        .map_err(|_| Error::limit(STRING_LENGTH_LIMIT_ERROR))
        .map(|_| count)
}

fn length_to_value(length: usize) -> Result<f64> {
    let length = u32::try_from(length).map_err(|_| Error::limit(STRING_LENGTH_LIMIT_ERROR))?;
    Ok(f64::from(length))
}
