use crate::{
    error::Result,
    runtime::Context,
    runtime::control::error_property_text,
    runtime::object::{OBJECT_CONSTRUCTOR_PROPERTY, PropertyKey},
    runtime::property::{
        DynamicPropertyKey, PropertyValue, StringPropertyValue, get_property,
        get_property_with_receiver, has_property, property_key, string_property_value,
    },
    value::{ObjectId, Value},
};

const MAX_UTF8_CHAR_BYTES: usize = 4;
const STRING_CONSTRUCTOR_PROPERTY: &str = "constructor";

impl Context {
    pub(in crate::runtime) fn dynamic_property_key(
        &self,
        value: &Value,
    ) -> Result<DynamicPropertyKey> {
        let name = property_key(value);
        self.check_string_len(&name)?;
        let key = match value {
            Value::Symbol(symbol) => Some(PropertyKey::symbol(symbol.id())),
            _ => self.known_property_key(&name),
        };
        Ok(DynamicPropertyKey::new(name, key))
    }

    pub(crate) fn get_property_value(&mut self, object: &Value, property: &str) -> Result<Value> {
        let lookup = self.property_lookup(property);
        if let Some(read) = self.semantic_property_read(object, lookup)? {
            return self.finish_semantic_property_read(read, object, lookup);
        }
        if let Value::String(value) = object {
            return self.get_string_property_value(object, value, property);
        }
        if let Value::HeapString(value) = object {
            return self.get_string_property_value(object, value.as_str(), property);
        }
        if let Some(value) = self.primitive_prototype_property_value(object, property)? {
            return Ok(value);
        }
        let value = get_property(&self.objects, object, lookup)?;
        self.runtime_property_value(value)
    }

    pub(in crate::runtime) fn get_property_value_with_lookup(
        &mut self,
        object: &Value,
        property: crate::runtime::object::PropertyLookup<'_>,
    ) -> Result<Value> {
        if let Some(read) = self.semantic_property_read(object, property)? {
            return self.finish_semantic_property_read(read, object, property);
        }
        let value = get_property(&self.objects, object, property)?;
        self.runtime_property_value(value)
    }

    pub(in crate::runtime) fn get_error_property_value(
        &mut self,
        error: &crate::value::ErrorObject,
        property: &str,
    ) -> Result<Value> {
        if property == OBJECT_CONSTRUCTOR_PROPERTY {
            return self.error_constructor_value(error.name());
        }
        if let Some(value) = error_property_text(error, property) {
            return self.heap_string_value(value);
        }
        self.error_prototype_property_value(error.name(), property)
    }

    pub(in crate::runtime) fn runtime_property_value(
        &mut self,
        value: PropertyValue<'_>,
    ) -> Result<Value> {
        match value {
            PropertyValue::Value(value) => self.runtime_value(value),
            PropertyValue::Text(value) => self.heap_string_value(value),
            PropertyValue::Character(ch) => self.heap_string_char_value(ch),
            PropertyValue::Getter { getter, receiver } => {
                let value = self.call_accessor_getter(&getter, receiver)?;
                self.runtime_value(value)
            }
        }
    }

    pub(in crate::runtime) fn get_string_property_value(
        &mut self,
        receiver: &Value,
        value: &str,
        property: &str,
    ) -> Result<Value> {
        if property == STRING_CONSTRUCTOR_PROPERTY {
            return self.string_constructor_value();
        }
        match string_property_value(value, property)? {
            StringPropertyValue::Length(value) => Ok(Value::Number(value)),
            StringPropertyValue::Character(ch) => self.heap_string_char_value(ch),
            StringPropertyValue::Missing => {
                self.string_prototype_property_value(receiver, property)
            }
        }
    }

    pub(in crate::runtime) fn heap_string_char_value(&mut self, ch: char) -> Result<Value> {
        let mut buffer = [0_u8; MAX_UTF8_CHAR_BYTES];
        self.heap_string_value(ch.encode_utf8(&mut buffer))
    }

    pub(in crate::runtime) fn get_string_object_property_value(
        &mut self,
        id: ObjectId,
        property: &str,
    ) -> Result<Option<Value>> {
        let Some(ch) = self.objects.string_object_character(id, property)? else {
            return Ok(None);
        };
        self.heap_string_char_value(ch).map(Some)
    }

    pub(in crate::runtime) fn string_object_primitive_value(
        &self,
        id: ObjectId,
    ) -> Result<Option<&str>> {
        self.objects.string_object_value(id)
    }

    pub(in crate::runtime) fn primitive_prototype_property_value(
        &mut self,
        object: &Value,
        property: &str,
    ) -> Result<Option<Value>> {
        match object {
            Value::Bool(_) => self
                .boolean_prototype_property_value(object, property)
                .map(Some),
            Value::Number(_) => self
                .number_prototype_property_value(object, property)
                .map(Some),
            Value::Symbol(_) => self
                .symbol_prototype_property_value(object, property)
                .map(Some),
            _ => Ok(None),
        }
    }

    pub(in crate::runtime) fn get_prototype_property_value_with_receiver(
        &mut self,
        prototype: ObjectId,
        receiver: &Value,
        property: &str,
    ) -> Result<Value> {
        let lookup = self.property_lookup(property);
        let value = get_property_with_receiver(&self.objects, prototype, receiver, lookup)?;
        self.runtime_property_value(value)
    }

    pub(in crate::runtime) fn has_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<bool> {
        self.has_property_value_with_lookup(object, property.lookup())
    }

    pub(in crate::runtime) fn has_property_value_with_lookup(
        &mut self,
        object: &Value,
        property: crate::runtime::object::PropertyLookup<'_>,
    ) -> Result<bool> {
        if let Some(presence) = self.semantic_property_presence(object, property)? {
            return self.finish_semantic_property_presence(presence, property);
        }
        has_property(&self.objects, object, property)
    }

    pub(in crate::runtime) fn intern_dynamic_property_key(
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
