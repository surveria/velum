use std::cell::Cell;
use std::rc::Rc;

use crate::{
    ast::{StaticName, StaticPropertyAccessId},
    atom::AtomId,
    binding_layout::BindingLayout,
    error::{Error, Result},
    runtime::Context,
    runtime_object::{
        CacheablePropertyLookup, CacheablePropertyPresence, CacheablePropertyValue, PropertyKey,
        PropertyLookup,
    },
    runtime_property::{
        DynamicPropertyKey, delete_property, get_property, has_property, set_property,
    },
    value::{ObjectId, Value},
};

use super::runtime_static_bindings::StaticBindingCacheHandle;

#[derive(Debug, Clone)]
pub struct StaticNameAtomCacheHandle {
    atoms: Rc<[Cell<Option<AtomId>>]>,
    property_lookups: Rc<[Cell<Option<CacheablePropertyLookup>>]>,
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
        Self {
            atoms: Rc::from(atoms.into_boxed_slice()),
            property_lookups: Rc::from(property_lookups.into_boxed_slice()),
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
        &self,
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
        if let Value::Object(id) = object
            && property.as_str() != PROTOTYPE_PROPERTY
        {
            return self.get_cached_static_object_property_value(*id, access, lookup);
        }
        self.checked_value(get_property(&self.objects, object, lookup)?)
    }

    fn get_cached_static_object_property_value(
        &self,
        object: ObjectId,
        access: StaticPropertyAccessId,
        lookup: PropertyLookup<'_>,
    ) -> Result<Value> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return self.checked_value(get_property(
                &self.objects,
                &Value::Object(object),
                lookup,
            )?);
        };
        if let Some(cached_lookup) = cache.property_lookup(access)? {
            match self
                .objects
                .read_cacheable_property_value_for(object, cached_lookup)?
            {
                CacheablePropertyValue::Hit(value) => return self.checked_value(value),
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
                self.checked_value(value)
            }
            CacheablePropertyValue::Missing => {
                cache.remember_property_lookup(access, candidate)?;
                Ok(Value::Undefined)
            }
            CacheablePropertyValue::Uncacheable => {
                self.checked_value(get_property(&self.objects, &Value::Object(object), lookup)?)
            }
        }
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
        if let Some(cached_lookup) = cache.property_lookup(access)? {
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
        value: Value,
    ) -> Result<()> {
        self.checked_value(value.clone())?;
        let key = self.intern_static_property_key(property)?;
        if let Value::Function(id) = object {
            return self.set_function_property_key(*id, property, key, value);
        }
        if let Value::NativeFunction(id) = object {
            return self.set_native_function_property_key(*id, property, key, value);
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

    pub(crate) fn delete_static_property_value(
        &mut self,
        object: &Value,
        property: &StaticName,
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
        delete_property(&mut self.objects, object, lookup).map(Value::Bool)
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
