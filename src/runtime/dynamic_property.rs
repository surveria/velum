use crate::{
    error::Result,
    runtime::assertions::error_property_text,
    runtime::object::PropertyKey,
    runtime::property::{
        DynamicPropertyKey, PropertyValue, StringPropertyValue, get_property, has_property,
        property_key, string_property_value,
    },
    value::{ObjectId, Value},
};

use super::Context;

const MAX_UTF8_CHAR_BYTES: usize = 4;

impl Context {
    pub(super) fn dynamic_property_key(&self, value: &Value) -> Result<DynamicPropertyKey> {
        let name = property_key(value);
        self.check_string_len(&name)?;
        let key = self.known_property_key(&name);
        Ok(DynamicPropertyKey::new(name, key))
    }

    pub(crate) fn get_property_value(&mut self, object: &Value, property: &str) -> Result<Value> {
        let lookup = self.property_lookup(property);
        if let Value::Function(id) = object {
            return self.get_function_property_lookup(*id, lookup);
        }
        if let Value::NativeFunction(id) = object {
            return self.get_native_function_property_lookup(*id, lookup);
        }
        if let Value::Error(error) = object {
            return self.get_error_property_value(error, property);
        }
        if let Value::String(value) = object {
            return self.get_string_property_value(value, property);
        }
        if let Value::HeapString(value) = object {
            return self.get_string_property_value(value.as_str(), property);
        }
        if let Value::Object(id) = object
            && let Some(value) = self.get_string_object_property_value(*id, property)?
        {
            return Ok(value);
        }
        let value = get_property(&self.objects, object, lookup)?;
        self.runtime_property_value(value)
    }

    pub(super) fn get_error_property_value(
        &mut self,
        error: &crate::value::ErrorObject,
        property: &str,
    ) -> Result<Value> {
        if let Some(value) = error_property_text(error, property) {
            return self.heap_string_value(value);
        }
        Ok(Value::Undefined)
    }

    pub(super) fn runtime_property_value(&mut self, value: PropertyValue<'_>) -> Result<Value> {
        match value {
            PropertyValue::Value(value) => self.runtime_value(value),
            PropertyValue::Text(value) => self.heap_string_value(value),
            PropertyValue::Character(ch) => self.heap_string_char_value(ch),
        }
    }

    pub(super) fn get_string_property_value(
        &mut self,
        value: &str,
        property: &str,
    ) -> Result<Value> {
        match string_property_value(value, property)? {
            StringPropertyValue::Length(value) => Ok(Value::Number(value)),
            StringPropertyValue::Character(ch) => self.heap_string_char_value(ch),
            StringPropertyValue::Missing => Ok(Value::Undefined),
        }
    }

    pub(super) fn heap_string_char_value(&mut self, ch: char) -> Result<Value> {
        let mut buffer = [0_u8; MAX_UTF8_CHAR_BYTES];
        self.heap_string_value(ch.encode_utf8(&mut buffer))
    }

    pub(super) fn get_string_object_property_value(
        &mut self,
        id: ObjectId,
        property: &str,
    ) -> Result<Option<Value>> {
        let Some(ch) = self.objects.string_object_character(id, property)? else {
            return Ok(None);
        };
        self.heap_string_char_value(ch).map(Some)
    }

    pub(super) fn has_dynamic_property_value(
        &self,
        object: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<bool> {
        match object {
            Value::Function(id) => self.has_function_property_lookup(*id, property.lookup()),
            Value::NativeFunction(id) => {
                self.has_native_function_property_lookup(*id, property.lookup())
            }
            _ => has_property(&self.objects, object, property.lookup()),
        }
    }

    pub(super) fn intern_dynamic_property_key(
        &mut self,
        property: &mut DynamicPropertyKey,
    ) -> Result<PropertyKey> {
        if let Some(key) = property.key() {
            return Ok(key);
        }
        let key = self.intern_property_key(property.name())?;
        property.remember_key(key);
        Ok(key)
    }
}
