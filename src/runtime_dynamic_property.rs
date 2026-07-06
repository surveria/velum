use crate::{
    ast::Expr,
    error::Result,
    runtime_object::PropertyKey,
    runtime_property::{
        DynamicPropertyKey, delete_property, get_property, has_property, property_key, set_property,
    },
    value::Value,
};

use super::Context;

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

    pub(crate) fn get_property_value(&self, object: &Value, property: &str) -> Result<Value> {
        let lookup = self.property_lookup(property);
        if let Value::Function(id) = object {
            return self.get_function_property_lookup(*id, lookup);
        }
        if let Value::NativeFunction(id) = object {
            return self.get_native_function_property_lookup(*id, lookup);
        }
        self.checked_value(get_property(&self.objects, object, lookup)?)
    }

    pub(crate) fn get_dynamic_property_value(
        &self,
        object: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<Value> {
        if let Value::Function(id) = object {
            return self.get_function_property_lookup(*id, property.lookup());
        }
        if let Value::NativeFunction(id) = object {
            return self.get_native_function_property_lookup(*id, property.lookup());
        }
        self.checked_value(get_property(&self.objects, object, property.lookup())?)
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
