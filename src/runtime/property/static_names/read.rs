use crate::{
    error::Result,
    runtime::Context,
    runtime::object::{
        CacheablePropertyPresence, CacheablePropertyValue, OBJECT_CONSTRUCTOR_PROPERTY,
        PropertyLookup,
    },
    runtime::property::{DynamicPropertyKey, get_property, has_property},
    syntax::{StaticName, StaticPropertyAccessId},
    value::{ObjectId, Value},
};

use super::PROTOTYPE_PROPERTY;

impl Context {
    pub(crate) fn get_static_property_value(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
    ) -> Result<Value> {
        let lookup = self.static_property_lookup(property)?;
        if let Value::Function(id) = object {
            return self.get_function_property_lookup(*id, lookup);
        }
        if let Value::NativeFunction(id) = object {
            return self.get_native_function_property_lookup(*id, lookup);
        }
        if let Value::Error(error) = object {
            return self.get_error_property_value(error, property.as_str());
        }
        if let Value::String(value) = object {
            return self.get_string_property_value(object, value, property.as_str());
        }
        if let Value::HeapString(value) = object {
            return self.get_string_property_value(object, value.as_str(), property.as_str());
        }
        if let Some(value) = self.primitive_prototype_property_value(object, property.as_str())? {
            return Ok(value);
        }
        if let Value::Object(id) = object
            && let Some(value) = self.get_string_object_property_value(*id, property.as_str())?
        {
            return Ok(value);
        }
        if let Value::Object(id) = object
            && let Some(value) = self.global_object_property_value(*id, lookup)?
        {
            return Ok(value);
        }
        if let Value::Object(id) = object
            && property.as_str() != PROTOTYPE_PROPERTY
        {
            return self.get_cached_object_property_value(*id, access, lookup);
        }
        let value = get_property(&self.objects, object, lookup)?;
        self.runtime_property_value(value)
    }

    pub(crate) fn get_array_length_property_value(&self, object: &Value) -> Result<Option<Value>> {
        let Value::Object(id) = object else {
            return Ok(None);
        };
        self.objects.array_length_value_if_array(*id)
    }

    pub(crate) fn get_cached_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &DynamicPropertyKey,
        access: StaticPropertyAccessId,
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
            return self.get_string_property_value(object, value, property.name());
        }
        if let Value::HeapString(value) = object {
            return self.get_string_property_value(object, value.as_str(), property.name());
        }
        if let Some(value) = self.primitive_prototype_property_value(object, property.name())? {
            return Ok(value);
        }
        if let Value::Object(id) = object
            && let Some(value) = self.get_string_object_property_value(*id, property.name())?
        {
            return Ok(value);
        }
        if let Value::Object(id) = object
            && let Some(value) = self.global_object_property_value(*id, property.lookup())?
        {
            return Ok(value);
        }
        if let Value::Object(id) = object
            && property.name() != PROTOTYPE_PROPERTY
            && self.objects.array_len_if_array(*id)?.is_none()
        {
            return self.get_cached_object_property_value(*id, access, property.lookup());
        }
        let value = get_property(&self.objects, object, property.lookup())?;
        self.runtime_property_value(value)
    }

    pub(crate) fn has_cached_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &DynamicPropertyKey,
        access: StaticPropertyAccessId,
    ) -> Result<bool> {
        match object {
            Value::Function(id) => self.has_function_property_lookup(*id, property.lookup()),
            Value::NativeFunction(id) => {
                self.has_native_function_property_lookup(*id, property.lookup())
            }
            Value::Error(error) => {
                if matches!(
                    property.name(),
                    "name" | "message" | OBJECT_CONSTRUCTOR_PROPERTY
                ) {
                    return Ok(true);
                }
                self.error_prototype_has_property(error.name(), property.lookup())
            }
            Value::Object(id) => {
                if let Some(has_property) =
                    self.global_object_has_property(*id, property.lookup())?
                {
                    return Ok(has_property);
                }
                self.has_cached_object_property_value(*id, property, access)
            }
            _ => has_property(&self.objects, object, property.lookup()),
        }
    }

    fn get_cached_object_property_value(
        &mut self,
        object: ObjectId,
        access: StaticPropertyAccessId,
        lookup: PropertyLookup<'_>,
    ) -> Result<Value> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            let object_value = Value::Object(object);
            let value = get_property(&self.objects, &object_value, lookup)?;
            return self.runtime_property_value(value);
        };
        if let Some(cached_lookup) = cache.property_lookup(access)?
            && cached_lookup.matches_property(lookup)
        {
            match self
                .objects
                .read_cacheable_property_value_for(object, cached_lookup)?
            {
                CacheablePropertyValue::Hit(value) => return self.runtime_value(value),
                CacheablePropertyValue::Missing => return Ok(Value::Undefined),
                CacheablePropertyValue::Uncacheable => {}
            }
        }

        let candidate = self.objects.cacheable_property_lookup(object, lookup)?;
        match self
            .objects
            .read_cacheable_property_value_for(object, candidate)?
        {
            CacheablePropertyValue::Hit(value) => {
                cache.remember_property_lookup(access, candidate)?;
                self.runtime_value(value)
            }
            CacheablePropertyValue::Missing => {
                cache.remember_property_lookup(access, candidate)?;
                Ok(Value::Undefined)
            }
            CacheablePropertyValue::Uncacheable => {
                let object_value = Value::Object(object);
                let value = get_property(&self.objects, &object_value, lookup)?;
                self.runtime_property_value(value)
            }
        }
    }

    fn has_cached_object_property_value(
        &self,
        object: ObjectId,
        property: &DynamicPropertyKey,
        access: StaticPropertyAccessId,
    ) -> Result<bool> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return has_property(&self.objects, &Value::Object(object), property.lookup());
        };
        if let Some(cached_lookup) = cache.property_lookup(access)?
            && cached_lookup.matches_property(property.lookup())
        {
            match self
                .objects
                .read_cacheable_property_presence_for(object, cached_lookup)?
            {
                CacheablePropertyPresence::Hit => return Ok(true),
                CacheablePropertyPresence::Missing => return Ok(false),
                CacheablePropertyPresence::Uncacheable => {}
            }
        }

        let candidate = self
            .objects
            .cacheable_property_lookup(object, property.lookup())?;
        match self
            .objects
            .read_cacheable_property_presence_for(object, candidate)?
        {
            CacheablePropertyPresence::Hit => {
                cache.remember_property_lookup(access, candidate)?;
                Ok(true)
            }
            CacheablePropertyPresence::Missing => {
                cache.remember_property_lookup(access, candidate)?;
                Ok(false)
            }
            CacheablePropertyPresence::Uncacheable => {
                has_property(&self.objects, &Value::Object(object), property.lookup())
            }
        }
    }
}
