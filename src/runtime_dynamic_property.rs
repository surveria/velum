use crate::{
    ast::Expr,
    error::Result,
    runtime_assertions::error_property_text,
    runtime_object::PropertyKey,
    runtime_property::{
        DynamicPropertyKey, StringPropertyValue, delete_property, get_property, has_property,
        property_key, set_property, string_property_value,
    },
    value::{ObjectId, Value},
};

use super::Context;

const MAX_UTF8_CHAR_BYTES: usize = 4;

impl Context {
    pub(crate) fn eval_property_key(&mut self, property: &Expr) -> Result<DynamicPropertyKey> {
        let value = self.eval_expr(property)?;
        self.dynamic_property_key(&value)
    }

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
        self.checked_value(get_property(&self.objects, object, lookup)?)
    }

    pub(crate) fn get_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<Value> {
        if let Value::Function(id) = object {
            return self.get_function_property_lookup(*id, property.lookup());
        }
        if let Value::NativeFunction(id) = object {
            return self.get_native_function_property_lookup(*id, property.lookup());
        }
        if let Value::Error(error) = object {
            return self.get_error_property_value(error, property.name());
        }
        if let Value::String(value) = object {
            return self.get_string_property_value(value, property.name());
        }
        if let Value::HeapString(value) = object {
            return self.get_string_property_value(value.as_str(), property.name());
        }
        if let Value::Object(id) = object
            && let Some(value) = self.get_string_object_property_value(*id, property.name())?
        {
            return Ok(value);
        }
        self.checked_value(get_property(&self.objects, object, property.lookup())?)
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

    pub(super) fn get_string_property_value(
        &mut self,
        value: &str,
        property: &str,
    ) -> Result<Value> {
        match string_property_value(value, property)? {
            StringPropertyValue::Length(value) => Ok(Value::Number(value)),
            StringPropertyValue::Character(ch) => {
                let mut buffer = [0_u8; MAX_UTF8_CHAR_BYTES];
                self.heap_string_value(ch.encode_utf8(&mut buffer))
            }
            StringPropertyValue::Missing => Ok(Value::Undefined),
        }
    }

    pub(super) fn get_string_object_property_value(
        &mut self,
        id: ObjectId,
        property: &str,
    ) -> Result<Option<Value>> {
        let Some(ch) = self.objects.string_object_character(id, property)? else {
            return Ok(None);
        };
        let mut buffer = [0_u8; MAX_UTF8_CHAR_BYTES];
        self.heap_string_value(ch.encode_utf8(&mut buffer))
            .map(Some)
    }

    pub(crate) fn set_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &mut DynamicPropertyKey,
        value: Value,
    ) -> Result<()> {
        self.checked_value(value.clone())?;
        if let Value::Function(id) = object {
            let key = self.intern_dynamic_property_key(property)?;
            return self.set_function_property_key(*id, property.name(), key, value);
        }
        if let Value::NativeFunction(id) = object {
            let key = self.intern_dynamic_property_key(property)?;
            return self.set_native_function_property_key(*id, property.name(), key, value);
        }
        let key = self.intern_dynamic_property_key(property)?;
        set_property(
            &mut self.objects,
            object,
            key,
            property.name(),
            value,
            self.limits.max_object_properties,
        )
    }

    pub(crate) fn delete_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<Value> {
        if let Value::Function(id) = object {
            return self
                .delete_function_property_lookup(*id, property.lookup())
                .map(Value::Bool);
        }
        if let Value::NativeFunction(id) = object {
            return self
                .delete_native_function_property_lookup(*id, property.lookup())
                .map(Value::Bool);
        }
        delete_property(&mut self.objects, object, property.lookup()).map(Value::Bool)
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

    fn intern_dynamic_property_key(
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
