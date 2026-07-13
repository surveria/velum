use crate::{
    error::{Error, Result},
    storage::string_heap::JsString,
    value::{ObjectId, Value},
};

use super::{
    DataPropertyUpdate, Object, ObjectHeap, PropertyConfigurable, PropertyEnumerable, PropertyKey,
    PropertyUpdate, PropertyWritable,
};

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
        let length = string_code_unit_count(value.as_utf16())?;
        let mut object = Object::string(value);
        object.prototype = Some(prototype);
        object.define_property(
            length_key,
            STRING_LENGTH_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Number(length_to_value(length)?)),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            &mut self.shapes,
            max_properties,
        )?;
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(crate) fn string_object_code_unit(
        &self,
        id: ObjectId,
        property: &str,
    ) -> Result<Option<u16>> {
        self.object(id)?.virtual_string_code_unit(property)
    }

    pub(crate) fn string_object_value(&self, id: ObjectId) -> Result<Option<&str>> {
        Ok(self
            .object(id)?
            .string_value
            .as_ref()
            .map(crate::storage::string_heap::JsString::as_str))
    }

    pub(crate) fn string_object_utf16_value(&self, id: ObjectId) -> Result<Option<&[u16]>> {
        Ok(self
            .object(id)?
            .string_value
            .as_ref()
            .map(crate::storage::string_heap::JsString::as_utf16))
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
            array_length_writable: super::PropertyWritable::Yes,
            string_value: Some(value),
            primitive_value: None,
            error_metadata: None,
            date_value: None,
            intl_value: None,
            temporal_value: None,
            regexp_value: None,
            proxy_value: None,
            byte_buffer: None,
            data_view: None,
            typed_array: None,
            is_raw_json: false,
            arguments_brand: false,
            argument_parameter_map: Vec::new(),
            function_prototype_brand: super::FunctionPrototypeBrand::Absent,
            module_namespace: false,
            shadow_realm: None,
            prototype: None,
            extensibility: super::ObjectExtensibility::Extensible,
            storage_ledger: None,
        }
    }

    pub(super) fn has_virtual_string_property(
        &self,
        property: super::PropertyLookup<'_>,
    ) -> Result<bool> {
        self.has_virtual_string_property_name(property.name())
    }

    pub(super) fn has_virtual_string_property_name(&self, property: &str) -> Result<bool> {
        self.virtual_string_code_unit(property)
            .map(|value| value.is_some())
    }

    pub(super) fn virtual_string_code_unit(&self, property: &str) -> Result<Option<u16>> {
        let Some(value) = self.string_value.as_ref() else {
            return Ok(None);
        };
        let Some(index) = super::ArrayIndex::parse(property) else {
            return Ok(None);
        };
        let position = index.position()?;
        Ok(value.as_utf16().get(position).copied())
    }

    pub(super) fn virtual_string_key_count(&self) -> usize {
        self.string_value
            .as_ref()
            .map_or(0, |value| value.as_utf16().len())
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
        let len = string_code_unit_count(value.as_utf16())?;
        for index in 0..len {
            super::property::push_unique_key(keys, index.to_string());
        }
        Ok(())
    }
}

fn string_code_unit_count(value: &[u16]) -> Result<usize> {
    let count = value.len();
    u32::try_from(count)
        .map_err(|_| Error::limit(STRING_LENGTH_LIMIT_ERROR))
        .map(|_| count)
}

fn length_to_value(length: usize) -> Result<f64> {
    let length = u32::try_from(length).map_err(|_| Error::limit(STRING_LENGTH_LIMIT_ERROR))?;
    Ok(f64::from(length))
}
