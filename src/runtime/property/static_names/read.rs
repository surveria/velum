use crate::{
    error::Result,
    runtime::Context,
    runtime::object::{
        CacheablePropertyPresence, CacheablePropertyValue, PropertyKey, PropertyLookup,
    },
    runtime::property::{DynamicPropertyKey, get_property, has_property},
    runtime::semantic_object::{SemanticPropertyPresence, SemanticPropertyRead},
    syntax::{StaticName, StaticPropertyAccessId},
    value::{ObjectId, Value},
};

use super::PROTOTYPE_PROPERTY;

impl Context {
    /// Validated per-site cache hit for a plain-object read. The cache is
    /// only ever filled from the plain-object tail of the lookup chain and
    /// its guard pins the receiver, so a validated hit can skip the exotic
    /// receiver probes entirely.
    fn cached_static_property_fast_read(
        &mut self,
        object: &Value,
        access: StaticPropertyAccessId,
        lookup: PropertyLookup<'_>,
    ) -> Result<Option<Value>> {
        let Value::Object(id) = object else {
            return Ok(None);
        };
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(None);
        };
        if let Some(cached_lookup) = cache.property_lookup(access)?
            && cached_lookup.matches_property(lookup)
            && let CacheablePropertyValue::Hit(value) = self
                .objects
                .read_cacheable_property_value_for(*id, cached_lookup)?
        {
            return self.runtime_value(value).map(Some);
        }
        Ok(None)
    }

    pub(crate) fn get_static_property_value(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
    ) -> Result<Value> {
        let lookup = self.static_property_lookup(property)?;
        if let Some(value) = self.cached_static_property_fast_read(object, access, lookup)? {
            return Ok(value);
        }
        if let Some(read) = self.semantic_property_read(object, lookup)? {
            return match read {
                SemanticPropertyRead::Resolved(value) => Ok(value),
                SemanticPropertyRead::ObjectTail(id) if property.as_str() != PROTOTYPE_PROPERTY => {
                    self.get_cached_object_property_value(id, access, lookup)
                }
                SemanticPropertyRead::ObjectTail(id) => self.finish_semantic_property_read(
                    SemanticPropertyRead::ObjectTail(id),
                    object,
                    lookup,
                ),
            };
        }
        if let Value::String(value) = object {
            return self.get_string_property_value(object, value, property.as_str());
        }
        if let Value::HeapString(value) = object {
            return self.get_utf16_string_property_value(
                object,
                value.as_utf16(),
                property.as_str(),
            );
        }
        if let Some(value) = self.primitive_prototype_property_value(object, property.as_str())? {
            return Ok(value);
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
        if let Some(value) =
            self.cached_static_property_fast_read(object, access, property.lookup())?
        {
            return Ok(value);
        }
        if let Some(read) = self.semantic_property_read(object, property.lookup())? {
            return match read {
                SemanticPropertyRead::Resolved(value) => Ok(value),
                SemanticPropertyRead::ObjectTail(id)
                    if property.name() != PROTOTYPE_PROPERTY
                        && self.objects.array_len_if_array(id)?.is_none() =>
                {
                    self.get_cached_object_property_value(id, access, property.lookup())
                }
                SemanticPropertyRead::ObjectTail(id) => self.finish_semantic_property_read(
                    SemanticPropertyRead::ObjectTail(id),
                    object,
                    property.lookup(),
                ),
            };
        }
        if matches!(property.lookup().key(), Some(PropertyKey::Symbol(_)))
            && matches!(
                object,
                Value::Bool(_)
                    | Value::Number(_)
                    | Value::String(_)
                    | Value::HeapString(_)
                    | Value::Symbol(_)
            )
        {
            return self.get(object, property.lookup());
        }
        if let Value::String(value) = object {
            return self.get_string_property_value(object, value, property.name());
        }
        if let Value::HeapString(value) = object {
            return self.get_utf16_string_property_value(object, value.as_utf16(), property.name());
        }
        if let Some(value) = self.primitive_prototype_property_value(object, property.name())? {
            return Ok(value);
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
        self.has_cached_property_lookup_value(object, property.lookup(), access)
    }

    pub(crate) fn has_cached_property_name_value(
        &mut self,
        object: &Value,
        property: &str,
        access: StaticPropertyAccessId,
    ) -> Result<bool> {
        let lookup = self.property_lookup(property);
        self.has_cached_property_lookup_value(object, lookup, access)
    }

    fn has_cached_property_lookup_value(
        &mut self,
        object: &Value,
        lookup: PropertyLookup<'_>,
        access: StaticPropertyAccessId,
    ) -> Result<bool> {
        let Some(presence) = self.semantic_property_presence(object, lookup)? else {
            return has_property(&self.objects, object, lookup);
        };
        match presence {
            SemanticPropertyPresence::Resolved(value) => Ok(value),
            SemanticPropertyPresence::ObjectTail(id) => {
                self.has_cached_object_property_lookup(id, lookup, access)
            }
        }
    }

    fn get_cached_object_property_value(
        &mut self,
        object: ObjectId,
        access: StaticPropertyAccessId,
        lookup: PropertyLookup<'_>,
    ) -> Result<Value> {
        self.ensure_object_prototype_intrinsic_for_ordinary_lookup(object, lookup.name())?;
        let lookup = if lookup.key().is_none() {
            self.property_lookup(lookup.name())
        } else {
            lookup
        };
        let Some(cache) = self.current_static_name_atom_cache() else {
            return self.finish_semantic_property_read(
                SemanticPropertyRead::ObjectTail(object),
                &Value::Object(object),
                lookup,
            );
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
            CacheablePropertyValue::Uncacheable => self.finish_semantic_property_read(
                SemanticPropertyRead::ObjectTail(object),
                &Value::Object(object),
                lookup,
            ),
        }
    }

    fn has_cached_object_property_lookup(
        &mut self,
        object: ObjectId,
        lookup: PropertyLookup<'_>,
        access: StaticPropertyAccessId,
    ) -> Result<bool> {
        self.ensure_object_prototype_intrinsic_for_ordinary_lookup(object, lookup.name())?;
        let lookup = if lookup.key().is_none() {
            self.property_lookup(lookup.name())
        } else {
            lookup
        };
        let Some(cache) = self.current_static_name_atom_cache() else {
            return self.finish_semantic_property_presence(
                SemanticPropertyPresence::ObjectTail(object),
                lookup,
            );
        };
        if let Some(cached_lookup) = cache.property_lookup(access)?
            && cached_lookup.matches_property(lookup)
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

        let candidate = self.objects.cacheable_property_lookup(object, lookup)?;
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
            CacheablePropertyPresence::Uncacheable => self.finish_semantic_property_presence(
                SemanticPropertyPresence::ObjectTail(object),
                lookup,
            ),
        }
    }
}
