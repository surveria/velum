use std::cell::Cell;
use std::rc::Rc;

use crate::{
    ast::{StaticName, StaticPropertyAccessId},
    atom::AtomId,
    binding_layout::BindingLayout,
    error::{Error, Result},
    runtime::Context,
    runtime::object::{
        CacheablePropertyDelete, CacheablePropertyLookup, CacheablePropertyPresence,
        CacheablePropertyValue, CacheablePropertyWrite, PropertyKey, PropertyLookup,
    },
    runtime::property::{
        DynamicPropertyKey, delete_property, get_property, has_property, set_property,
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use super::native::NativeFunctionKind;
use super::static_bindings::StaticBindingCacheHandle;

#[derive(Debug, Clone)]
pub struct StaticNameAtomCacheHandle {
    atoms: Rc<[Cell<Option<AtomId>>]>,
    property_lookups: Rc<[Cell<Option<CacheablePropertyLookup>>]>,
    native_calls: Rc<[Cell<Option<StaticPropertyNativeCallCache>>]>,
}

impl StaticNameAtomCacheHandle {
    pub(super) fn new(static_name_count: usize, static_property_access_count: usize) -> Self {
        let mut atoms = Vec::with_capacity(static_name_count);
        for _ in 0..static_name_count {
            atoms.push(Cell::new(None));
        }
        let mut property_lookups = Vec::with_capacity(static_property_access_count);
        for _ in 0..static_property_access_count {
            property_lookups.push(Cell::new(None));
        }
        let mut native_calls = Vec::with_capacity(static_property_access_count);
        for _ in 0..static_property_access_count {
            native_calls.push(Cell::new(None));
        }
        Self {
            atoms: Rc::from(atoms.into_boxed_slice()),
            property_lookups: Rc::from(property_lookups.into_boxed_slice()),
            native_calls: Rc::from(native_calls.into_boxed_slice()),
        }
    }

    fn atom(&self, name: &StaticName) -> Result<Option<AtomId>> {
        self.atoms
            .get(name.id().index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static name atom cache slot is not defined"))
    }

    fn remember(&self, name: &StaticName, atom: AtomId) -> Result<()> {
        let slot = self
            .atoms
            .get(name.id().index()?)
            .ok_or_else(|| Error::runtime("static name atom cache slot is not defined"))?;
        slot.set(Some(atom));
        Ok(())
    }

    fn property_lookup(
        &self,
        access: StaticPropertyAccessId,
    ) -> Result<Option<CacheablePropertyLookup>> {
        self.property_lookups
            .get(access.index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static property cache slot is not defined"))
    }

    fn remember_property_lookup(
        &self,
        access: StaticPropertyAccessId,
        lookup: CacheablePropertyLookup,
    ) -> Result<()> {
        let slot = self
            .property_lookups
            .get(access.index()?)
            .ok_or_else(|| Error::runtime("static property cache slot is not defined"))?;
        slot.set(Some(lookup));
        Ok(())
    }

    fn native_call(
        &self,
        access: StaticPropertyAccessId,
    ) -> Result<Option<StaticPropertyNativeCallCache>> {
        self.native_calls
            .get(access.index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static property native call cache slot is not defined"))
    }

    fn remember_native_call(
        &self,
        access: StaticPropertyAccessId,
        function: NativeFunctionId,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let slot = self.native_calls.get(access.index()?).ok_or_else(|| {
            Error::runtime("static property native call cache slot is not defined")
        })?;
        slot.set(Some(StaticPropertyNativeCallCache::new(function, kind)));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct StaticPropertyNativeCallCache {
    function: NativeFunctionId,
    kind: NativeFunctionKind,
}

impl StaticPropertyNativeCallCache {
    const fn new(function: NativeFunctionId, kind: NativeFunctionKind) -> Self {
        Self { function, kind }
    }

    fn kind_if_current(self, function: NativeFunctionId) -> Option<NativeFunctionKind> {
        if self.function == function {
            return Some(self.kind);
        }
        None
    }
}

const PROTOTYPE_PROPERTY: &str = "__proto__";

impl Context {
    pub(crate) fn with_static_name_atom_cache<T>(
        &mut self,
        cache: StaticNameAtomCacheHandle,
        evaluate: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.static_name_atom_caches.push(cache);
        let result = evaluate(self);
        self.pop_static_name_atom_cache()?;
        result
    }

    pub(crate) fn with_static_name_caches<T>(
        &mut self,
        atom_cache: StaticNameAtomCacheHandle,
        binding_cache: StaticBindingCacheHandle,
        binding_layout: BindingLayout,
        evaluate: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.static_name_atom_caches.push(atom_cache);
        self.static_binding_caches.push(binding_cache);
        self.static_binding_layouts.push(binding_layout);
        let result = evaluate(self);
        self.pop_static_binding_layout()?;
        self.pop_static_binding_cache()?;
        self.pop_static_name_atom_cache()?;
        result
    }

    pub(crate) fn current_static_name_atom_cache(&self) -> Option<StaticNameAtomCacheHandle> {
        self.static_name_atom_caches.last().cloned()
    }

    pub(super) fn cached_static_property_native_call_kind(
        &self,
        access: StaticPropertyAccessId,
        function: NativeFunctionId,
    ) -> Result<Option<NativeFunctionKind>> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(None);
        };
        Ok(cache
            .native_call(access)?
            .and_then(|cached| cached.kind_if_current(function)))
    }

    pub(super) fn remember_static_property_native_call_kind(
        &self,
        access: StaticPropertyAccessId,
        function: NativeFunctionId,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(());
        };
        cache.remember_native_call(access, function, kind)
    }

    pub(crate) fn lookup_static_name_atom(&self, name: &StaticName) -> Result<Option<AtomId>> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(self.atom(name));
        };
        if let Some(atom) = cache.atom(name)? {
            return Ok(Some(atom));
        }
        let Some(atom) = self.atom(name) else {
            return Ok(None);
        };
        cache.remember(name, atom)?;
        Ok(Some(atom))
    }

    pub(crate) fn intern_static_name_atom(&mut self, name: &StaticName) -> Result<AtomId> {
        if let Some(atom) = self.lookup_static_name_atom(name)? {
            return Ok(atom);
        }
        let atom = self.intern_atom(name)?;
        self.remember_static_name_atom(name, atom)?;
        Ok(atom)
    }

    pub(crate) fn intern_static_property_key(&mut self, name: &StaticName) -> Result<PropertyKey> {
        if let Some(key) = self.well_known_properties.lookup(name) {
            return Ok(key);
        }
        self.intern_static_name_atom(name).map(PropertyKey::new)
    }

    pub(crate) fn static_property_lookup<'a>(
        &self,
        name: &'a StaticName,
    ) -> Result<PropertyLookup<'a>> {
        if let Some(key) = self.well_known_properties.lookup(name) {
            return Ok(PropertyLookup::from_key(name.as_str(), key));
        }
        let key = self.lookup_static_name_atom(name)?.map(PropertyKey::new);
        Ok(PropertyLookup::new(name.as_str(), key))
    }

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
            return self.get_string_property_value(value, property.as_str());
        }
        if let Value::HeapString(value) = object {
            return self.get_string_property_value(value.as_str(), property.as_str());
        }
        if let Value::Object(id) = object
            && let Some(value) = self.get_string_object_property_value(*id, property.as_str())?
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
        &self,
        object: &Value,
        property: &DynamicPropertyKey,
        access: StaticPropertyAccessId,
    ) -> Result<bool> {
        match object {
            Value::Function(id) => self.has_function_property_lookup(*id, property.lookup()),
            Value::NativeFunction(id) => {
                self.has_native_function_property_lookup(*id, property.lookup())
            }
            Value::Object(id) => self.has_cached_object_property_value(*id, property, access),
            _ => has_property(&self.objects, object, property.lookup()),
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

    pub(crate) fn set_static_property_value(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        value: Value,
    ) -> Result<()> {
        let value = self.runtime_value(value)?;
        let key = self.intern_static_property_key(property)?;
        if let Value::Function(id) = object {
            return self.set_function_property_key(*id, property, key, value);
        }
        if let Value::NativeFunction(id) = object {
            return self.set_native_function_property_key(*id, property, key, value);
        }
        if let Value::Object(id) = object
            && property.as_str() != PROTOTYPE_PROPERTY
            && self.set_cached_object_property_value(
                *id,
                access,
                PropertyLookup::from_key(property.as_str(), key),
                value.clone(),
            )?
        {
            return Ok(());
        }
        set_property(
            &mut self.objects,
            object,
            key,
            property,
            value,
            self.limits.max_object_properties,
        )
    }

    pub(crate) fn set_cached_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &mut DynamicPropertyKey,
        access: StaticPropertyAccessId,
        value: Value,
    ) -> Result<()> {
        let value = self.runtime_value(value)?;
        if let Value::Function(id) = object {
            let key = self.intern_dynamic_property_key(property)?;
            return self.set_function_property_key(*id, property.name(), key, value);
        }
        if let Value::NativeFunction(id) = object {
            let key = self.intern_dynamic_property_key(property)?;
            return self.set_native_function_property_key(*id, property.name(), key, value);
        }
        let key = self.intern_dynamic_property_key(property)?;
        if let Value::Object(id) = object
            && property.name() != PROTOTYPE_PROPERTY
            && self.objects.array_len_if_array(*id)?.is_none()
            && self.set_cached_object_property_value(
                *id,
                access,
                PropertyLookup::from_key(property.name(), key),
                value.clone(),
            )?
        {
            return Ok(());
        }
        set_property(
            &mut self.objects,
            object,
            key,
            property.name(),
            value,
            self.limits.max_object_properties,
        )
    }

    fn set_cached_object_property_value(
        &mut self,
        object: ObjectId,
        access: StaticPropertyAccessId,
        lookup: PropertyLookup<'_>,
        value: Value,
    ) -> Result<bool> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(false);
        };
        if let Some(cached_lookup) = cache.property_lookup(access)?
            && cached_lookup.matches_property(lookup)
            && self.objects.write_cacheable_own_property_value_for(
                object,
                cached_lookup,
                value.clone(),
            )? == CacheablePropertyWrite::Updated
        {
            return Ok(true);
        }

        let candidate = self.objects.cacheable_property_lookup(object, lookup)?;
        if self
            .objects
            .write_cacheable_own_property_value_for(object, candidate, value)?
            == CacheablePropertyWrite::Updated
        {
            cache.remember_property_lookup(access, candidate)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn delete_static_property_value(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
    ) -> Result<Value> {
        let lookup = self.static_property_lookup(property)?;
        if let Value::Function(id) = object {
            return self
                .delete_function_property_lookup(*id, lookup)
                .map(Value::Bool);
        }
        if let Value::NativeFunction(id) = object {
            return self
                .delete_native_function_property_lookup(*id, lookup)
                .map(Value::Bool);
        }
        if let Value::Object(id) = object
            && property.as_str() != PROTOTYPE_PROPERTY
            && self.objects.array_len_if_array(*id)?.is_none()
        {
            return self
                .delete_cached_object_property_value(*id, access, lookup)
                .map(Value::Bool);
        }
        delete_property(&mut self.objects, object, lookup).map(Value::Bool)
    }

    pub(crate) fn delete_cached_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &DynamicPropertyKey,
        access: StaticPropertyAccessId,
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
        if let Value::Object(id) = object
            && property.name() != PROTOTYPE_PROPERTY
            && self.objects.array_len_if_array(*id)?.is_none()
        {
            return self
                .delete_cached_object_property_value(*id, access, property.lookup())
                .map(Value::Bool);
        }
        delete_property(&mut self.objects, object, property.lookup()).map(Value::Bool)
    }

    fn delete_cached_object_property_value(
        &mut self,
        object: ObjectId,
        access: StaticPropertyAccessId,
        lookup: PropertyLookup<'_>,
    ) -> Result<bool> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return delete_property(&mut self.objects, &Value::Object(object), lookup);
        };
        if let Some(cached_lookup) = cache.property_lookup(access)?
            && cached_lookup.matches_property(lookup)
        {
            match self
                .objects
                .delete_cacheable_own_property_for(object, cached_lookup)?
            {
                CacheablePropertyDelete::Deleted | CacheablePropertyDelete::Missing => {
                    return Ok(true);
                }
                CacheablePropertyDelete::NotConfigurable => return Ok(false),
                CacheablePropertyDelete::Uncacheable => {}
            }
        }

        let candidate = self.objects.cacheable_property_lookup(object, lookup)?;
        match self
            .objects
            .delete_cacheable_own_property_for(object, candidate)?
        {
            CacheablePropertyDelete::Deleted => Ok(true),
            CacheablePropertyDelete::Missing => {
                cache.remember_property_lookup(access, candidate)?;
                Ok(true)
            }
            CacheablePropertyDelete::NotConfigurable => {
                cache.remember_property_lookup(access, candidate)?;
                Ok(false)
            }
            CacheablePropertyDelete::Uncacheable => {
                delete_property(&mut self.objects, &Value::Object(object), lookup)
            }
        }
    }

    fn remember_static_name_atom(&self, name: &StaticName, atom: AtomId) -> Result<()> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(());
        };
        cache.remember(name, atom)
    }

    fn pop_static_name_atom_cache(&mut self) -> Result<()> {
        if self.static_name_atom_caches.pop().is_some() {
            return Ok(());
        }
        Err(Error::runtime("static name atom cache disappeared"))
    }

    fn pop_static_binding_cache(&mut self) -> Result<()> {
        if self.static_binding_caches.pop().is_some() {
            return Ok(());
        }
        Err(Error::runtime("static binding cache disappeared"))
    }

    fn pop_static_binding_layout(&mut self) -> Result<()> {
        if self.static_binding_layouts.pop().is_some() {
            return Ok(());
        }
        Err(Error::runtime("static binding layout disappeared"))
    }
}
