use crate::{
    error::Result,
    runtime::Context,
    runtime::object::{PropertyKey, PropertyLookup},
    runtime::property::{
        DynamicPropertyKey, PropertyValue, StringPropertyValue, get_property_with_receiver,
        has_property, string_property_value, utf16_string_property_value,
    },
    value::{ObjectId, Value},
};

const STRING_CONSTRUCTOR_PROPERTY: &str = "constructor";

impl Context {
    pub(in crate::runtime) fn dynamic_property_key(
        &mut self,
        value: &Value,
    ) -> Result<DynamicPropertyKey> {
        self.to_property_key(value)
    }

    pub(in crate::runtime) fn runtime_property_value(
        &mut self,
        value: PropertyValue,
    ) -> Result<Value> {
        match value {
            PropertyValue::Value(value) => self.runtime_value(value),
            PropertyValue::CodeUnit(unit) => self.heap_string_code_unit_value(unit),
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
            StringPropertyValue::CodeUnit(unit) => self.heap_string_code_unit_value(unit),
            StringPropertyValue::Missing => {
                self.string_prototype_property_value(receiver, property)
            }
        }
    }

    pub(in crate::runtime) fn heap_string_code_unit_value(&mut self, unit: u16) -> Result<Value> {
        self.heap_utf16_string_value(&[unit])
    }

    pub(in crate::runtime) fn get_utf16_string_property_value(
        &mut self,
        receiver: &Value,
        value: &[u16],
        property: &str,
    ) -> Result<Value> {
        if property == STRING_CONSTRUCTOR_PROPERTY {
            return self.string_constructor_value();
        }
        match utf16_string_property_value(value, property)? {
            StringPropertyValue::Length(value) => Ok(Value::Number(value)),
            StringPropertyValue::CodeUnit(unit) => self.heap_string_code_unit_value(unit),
            StringPropertyValue::Missing => {
                self.string_prototype_property_value(receiver, property)
            }
        }
    }

    pub(in crate::runtime) fn get_string_object_property_value(
        &mut self,
        id: ObjectId,
        property: &str,
    ) -> Result<Option<Value>> {
        let Some(unit) = self.objects.string_object_code_unit(id, property)? else {
            return Ok(None);
        };
        self.heap_string_code_unit_value(unit).map(Some)
    }

    pub(in crate::runtime) fn string_object_utf16_primitive_value(
        &self,
        id: ObjectId,
    ) -> Result<Option<&[u16]>> {
        self.objects.string_object_utf16_value(id)
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

    pub(in crate::runtime) fn primitive_prototype_property_value_with_lookup(
        &mut self,
        object: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Option<Value>> {
        match object {
            Value::Bool(_) => self
                .boolean_prototype_property_value_with_lookup(object, property)
                .map(Some),
            Value::Number(_) => self
                .number_prototype_property_value_with_lookup(object, property)
                .map(Some),
            Value::Symbol(_) => self
                .symbol_prototype_property_value_with_lookup(object, property)
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
        self.get_prototype_property_value_with_lookup(prototype, receiver, lookup)
    }

    pub(in crate::runtime) fn get_prototype_property_value_with_lookup(
        &mut self,
        prototype: ObjectId,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let value = get_property_with_receiver(&self.objects, prototype, receiver, property)?;
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
