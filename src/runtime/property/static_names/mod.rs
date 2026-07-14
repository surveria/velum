use crate::{
    binding_metadata::BindingLayout,
    error::{Error, Result},
    runtime::Context,
    runtime::binding::static_bindings::StaticBindingCacheHandle,
    runtime::native::NativeFunctionKind,
    runtime::object::{
        CacheablePropertyDelete, CacheablePropertyLookup, CacheablePropertyValue,
        CacheablePropertyWrite, OwnPropertyDescriptor, PropertyKey, PropertyLookup,
    },
    runtime::property::{DynamicPropertyKey, delete_property},
    runtime::semantic_object::{SemanticPropertyDelete, SemanticPropertyWrite},
    storage::atom::AtomId,
    syntax::{StaticCallSiteId, StaticName, StaticPropertyAccessId},
    value::{NativeFunctionId, ObjectId, Value},
};

mod cache;
mod native_call_cache;
mod read;

pub(in crate::runtime) use cache::{CallValueCache, StaticNameAtomCacheHandle};

enum CachedPropertyMutation {
    Updated { old_value: Value, new_value: Value },
    NeedsGenericSet { old_value: Value, new_value: Value },
}

impl Context {
    pub(crate) fn with_static_name_atom_cache<T>(
        &mut self,
        cache: StaticNameAtomCacheHandle,
        evaluate: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let cache_entries = cache.storage_entry_count()?;
        let reservation = self
            .storage_ledger
            .reserve_count(crate::runtime::VmStorageKind::CacheEntry, cache_entries)?;
        reservation.commit()?;
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
        let cache_entries = atom_cache
            .storage_entry_count()?
            .checked_add(binding_cache.storage_entry_count()?)
            .and_then(|count| count.checked_add(binding_layout.storage_entry_count().ok()?))
            .ok_or_else(|| Error::limit("static cache entry count overflowed"))?;
        let reservation = self
            .storage_ledger
            .reserve_count(crate::runtime::VmStorageKind::CacheEntry, cache_entries)?;
        reservation.commit()?;
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
        if !self.optional_optimizations_enabled() {
            return None;
        }
        self.static_name_atom_caches.last().cloned()
    }

    pub(crate) fn current_static_name_atom_cache_owner(&self) -> Option<StaticNameAtomCacheHandle> {
        self.static_name_atom_caches.last().cloned()
    }

    pub(in crate::runtime) fn cached_static_property_native_call_kind(
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

    pub(in crate::runtime) fn cached_static_object_property_native_call_kind(
        &self,
        access: StaticPropertyAccessId,
        object: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<NativeFunctionKind>> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(None);
        };
        let Some(cached) = cache.native_call(access)? else {
            return Ok(None);
        };
        let Some(object_property) = cached.object_property else {
            return Ok(None);
        };
        if !object_property.lookup.matches_property(property) {
            return Ok(None);
        }
        if self.objects.cacheable_native_property_is_current_for(
            object,
            object_property.lookup,
            cached.function,
            object_property.version,
        )? {
            return Ok(Some(cached.kind));
        }
        Ok(None)
    }

    pub(in crate::runtime) fn cached_static_object_property_native_call_kind_for_access(
        &self,
        access: StaticPropertyAccessId,
        object: ObjectId,
    ) -> Result<Option<NativeFunctionKind>> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(None);
        };
        let Some(cached) = cache.native_call(access)? else {
            return Ok(None);
        };
        let Some(object_property) = cached.object_property else {
            return Ok(None);
        };
        if self.objects.cacheable_native_property_is_current_for(
            object,
            object_property.lookup,
            cached.function,
            object_property.version,
        )? {
            return Ok(Some(cached.kind));
        }
        Ok(None)
    }

    pub(in crate::runtime) fn remember_static_property_native_call_kind(
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

    pub(in crate::runtime) fn remember_static_object_property_native_call_kind(
        &self,
        access: StaticPropertyAccessId,
        lookup: CacheablePropertyLookup,
        version: u64,
        function: NativeFunctionId,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(());
        };
        cache.remember_object_property_native_call(access, lookup, version, function, kind)
    }

    pub(in crate::runtime) fn cached_call_value(
        &self,
        site: StaticCallSiteId,
    ) -> Result<Option<CallValueCache>> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(None);
        };
        cache.call_value(site)
    }

    pub(in crate::runtime) fn remember_call_value(
        &self,
        site: StaticCallSiteId,
        cache_value: CallValueCache,
    ) -> Result<()> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(());
        };
        cache.remember_call_value(site, cache_value)
    }

    pub(in crate::runtime) fn cached_template_object(
        &self,
        site: StaticCallSiteId,
    ) -> Result<Option<Value>> {
        let cache = self
            .current_static_name_atom_cache_owner()
            .ok_or_else(|| Error::runtime("static template object cache is unavailable"))?;
        cache.template_object(site)
    }

    pub(in crate::runtime) fn remember_template_object(
        &self,
        site: StaticCallSiteId,
        value: Value,
    ) -> Result<()> {
        let cache = self
            .current_static_name_atom_cache_owner()
            .ok_or_else(|| Error::runtime("static template object cache is unavailable"))?;
        cache.remember_template_object(site, value)
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
        // Per-site atom cache first: an indexed Cell read replaces the
        // well-known string match on hot paths.
        if let Some(cache) = self.current_static_name_atom_cache()
            && let Some(atom) = cache.atom(name)?
        {
            return Ok(PropertyLookup::from_key(
                name.as_str(),
                PropertyKey::new(atom),
            ));
        }
        if let Some(key) = self.well_known_properties.lookup(name) {
            // Remember atom-backed well-known keys in the per-site cache so
            // later accesses skip the string match entirely.
            if let PropertyKey::Atom(atom) = key
                && let Some(cache) = self.current_static_name_atom_cache()
            {
                cache.remember(name, atom)?;
            }
            return Ok(PropertyLookup::from_key(name.as_str(), key));
        }
        let key = self.lookup_static_name_atom(name)?.map(PropertyKey::new);
        Ok(PropertyLookup::new(name.as_str(), key))
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
        let lookup = PropertyLookup::from_key(property.as_str(), key);
        let Some(write) = self.semantic_property_write(object, lookup, value.clone())? else {
            self.set_property_value_with_accessors(object, key, property, value)?;
            return Ok(());
        };
        if let SemanticPropertyWrite::ObjectTail(id) = write
            && !self.is_global_object_id(id)
            && self.set_cached_object_property_value(id, access, lookup, value.clone())?
        {
            return Ok(());
        }
        self.finish_semantic_property_write(write, lookup, value)?;
        Ok(())
    }

    pub(in crate::runtime) fn try_set_cached_static_own_property_value(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        value: Value,
    ) -> Result<bool> {
        let Value::Object(id) = object else {
            return Ok(false);
        };
        if self.objects.is_proxy(*id)
            || self.objects.is_module_namespace(*id)?
            || self.is_global_object_id(*id)
        {
            return Ok(false);
        }
        let value = self.runtime_value(value)?;
        let key = self.intern_static_property_key(property)?;
        let lookup = PropertyLookup::from_key(property.as_str(), key);
        let dynamic = DynamicPropertyKey::new(property.as_str().to_owned(), Some(key));
        if !matches!(
            self.semantic_own_property_descriptor(object, &dynamic)?,
            Some(OwnPropertyDescriptor::Data(descriptor)) if descriptor.writable().is_yes()
        ) {
            return Ok(false);
        }
        if !self.set_cached_object_property_value(*id, access, lookup, value)? {
            return Ok(false);
        }
        Ok(true)
    }

    pub(in crate::runtime) fn try_cached_static_property_read_modify_write(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
        update: impl FnOnce(&mut Self, &Value) -> Result<(Value, Value)>,
    ) -> Result<Option<(Value, Value)>> {
        let Value::Object(object_id) = object else {
            return Ok(None);
        };
        if self.is_global_object_id(*object_id) {
            return Ok(None);
        }

        let key = self.intern_static_property_key(property)?;
        let lookup = PropertyLookup::from_key(property.as_str(), key);
        let Some(mutation) =
            self.try_cached_object_property_read_modify_write(*object_id, access, lookup, update)?
        else {
            return Ok(None);
        };
        match mutation {
            CachedPropertyMutation::Updated {
                old_value,
                new_value,
            } => Ok(Some((old_value, new_value))),
            CachedPropertyMutation::NeedsGenericSet {
                old_value,
                new_value,
            } => {
                self.set_static_property_value(object, property, access, new_value.clone())?;
                Ok(Some((old_value, new_value)))
            }
        }
    }

    pub(crate) fn set_cached_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &mut DynamicPropertyKey,
        access: StaticPropertyAccessId,
        value: Value,
    ) -> Result<()> {
        let value = self.runtime_value(value)?;
        let key = self.intern_dynamic_property_key(property)?;
        let lookup = PropertyLookup::from_key(property.name(), key);
        let Some(write) = self.semantic_property_write(object, lookup, value.clone())? else {
            self.set_property_value_with_accessors(object, key, property.name(), value)?;
            return Ok(());
        };
        if let SemanticPropertyWrite::ObjectTail(id) = write
            && !self.is_global_object_id(id)
            && self.objects.array_len_if_array(id)?.is_none()
            && self.set_cached_object_property_value(id, access, lookup, value.clone())?
        {
            return Ok(());
        }
        self.finish_semantic_property_write(write, lookup, value)?;
        Ok(())
    }

    pub(in crate::runtime) fn try_set_cached_dynamic_own_property_value(
        &mut self,
        object: &Value,
        property: &mut DynamicPropertyKey,
        access: StaticPropertyAccessId,
        value: Value,
    ) -> Result<bool> {
        let Value::Object(id) = object else {
            return Ok(false);
        };
        if self.objects.is_proxy(*id)
            || self.objects.is_module_namespace(*id)?
            || self.is_global_object_id(*id)
            || self.objects.array_len_if_array(*id)?.is_some()
        {
            return Ok(false);
        }
        let value = self.runtime_value(value)?;
        let key = self.intern_dynamic_property_key(property)?;
        let lookup = PropertyLookup::from_key(property.name(), key);
        if !matches!(
            self.semantic_own_property_descriptor(object, &*property)?,
            Some(OwnPropertyDescriptor::Data(descriptor)) if descriptor.writable().is_yes()
        ) {
            return Ok(false);
        }
        if !self.set_cached_object_property_value(*id, access, lookup, value)? {
            return Ok(false);
        }
        Ok(true)
    }

    pub(in crate::runtime) fn try_cached_dynamic_property_read_modify_write(
        &mut self,
        object: &Value,
        property: &mut DynamicPropertyKey,
        access: StaticPropertyAccessId,
        update: impl FnOnce(&mut Self, &Value) -> Result<(Value, Value)>,
    ) -> Result<Option<(Value, Value)>> {
        let Value::Object(object_id) = object else {
            return Ok(None);
        };
        if self.is_global_object_id(*object_id)
            || self.objects.array_len_if_array(*object_id)?.is_some()
        {
            return Ok(None);
        }

        let key = self.intern_dynamic_property_key(property)?;
        let lookup = PropertyLookup::from_key(property.name(), key);
        let Some(mutation) =
            self.try_cached_object_property_read_modify_write(*object_id, access, lookup, update)?
        else {
            return Ok(None);
        };
        match mutation {
            CachedPropertyMutation::Updated {
                old_value,
                new_value,
            } => Ok(Some((old_value, new_value))),
            CachedPropertyMutation::NeedsGenericSet {
                old_value,
                new_value,
            } => {
                self.set_cached_dynamic_property_value(
                    object,
                    property,
                    access,
                    new_value.clone(),
                )?;
                Ok(Some((old_value, new_value)))
            }
        }
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

    fn try_cached_object_property_read_modify_write(
        &mut self,
        object: ObjectId,
        access: StaticPropertyAccessId,
        lookup: PropertyLookup<'_>,
        update: impl FnOnce(&mut Self, &Value) -> Result<(Value, Value)>,
    ) -> Result<Option<CachedPropertyMutation>> {
        let Some(cache) = self.current_static_name_atom_cache() else {
            return Ok(None);
        };
        if let Some(cached_lookup) = cache.property_lookup(access)?
            && cached_lookup.matches_property(lookup)
        {
            match self
                .objects
                .read_cacheable_property_value_for(object, cached_lookup)?
            {
                CacheablePropertyValue::Hit(value) => {
                    return self.mutate_cached_object_property(
                        object,
                        cached_lookup,
                        value,
                        update,
                    );
                }
                CacheablePropertyValue::Missing => return Ok(None),
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
                self.mutate_cached_object_property(object, candidate, value, update)
            }
            CacheablePropertyValue::Missing => {
                cache.remember_property_lookup(access, candidate)?;
                Ok(None)
            }
            CacheablePropertyValue::Uncacheable => Ok(None),
        }
    }

    fn mutate_cached_object_property(
        &mut self,
        object: ObjectId,
        lookup: CacheablePropertyLookup,
        old_value: Value,
        update: impl FnOnce(&mut Self, &Value) -> Result<(Value, Value)>,
    ) -> Result<Option<CachedPropertyMutation>> {
        let old_value = self.runtime_value(old_value)?;
        let (old_value, new_value) = update(self, &old_value)?;
        let new_value = self.runtime_value(new_value)?;
        if self
            .objects
            .write_cacheable_own_property_value_for(object, lookup, new_value.clone())?
            == CacheablePropertyWrite::Updated
        {
            return Ok(Some(CachedPropertyMutation::Updated {
                old_value,
                new_value,
            }));
        }
        Ok(Some(CachedPropertyMutation::NeedsGenericSet {
            old_value,
            new_value,
        }))
    }

    pub(crate) fn delete_static_property_value(
        &mut self,
        object: &Value,
        property: &StaticName,
        access: StaticPropertyAccessId,
    ) -> Result<Value> {
        let lookup = self.static_property_lookup(property)?;
        let Some(deletion) = self.semantic_property_delete(object, lookup)? else {
            return delete_property(&mut self.objects, object, lookup).map(Value::Bool);
        };
        if let SemanticPropertyDelete::ObjectTail(id) = deletion
            && !self.is_global_object_id(id)
            && self.objects.array_len_if_array(id)?.is_none()
        {
            return self
                .delete_cached_object_property_value(id, access, lookup)
                .map(Value::Bool);
        }
        self.finish_semantic_property_delete(deletion, lookup)
            .map(Value::Bool)
    }

    pub(crate) fn delete_cached_dynamic_property_value(
        &mut self,
        object: &Value,
        property: &DynamicPropertyKey,
        access: StaticPropertyAccessId,
    ) -> Result<Value> {
        let lookup = property.lookup();
        let Some(deletion) = self.semantic_property_delete(object, lookup)? else {
            return delete_property(&mut self.objects, object, lookup).map(Value::Bool);
        };
        if let SemanticPropertyDelete::ObjectTail(id) = deletion
            && !self.is_global_object_id(id)
            && self.objects.array_len_if_array(id)?.is_none()
        {
            return self
                .delete_cached_object_property_value(id, access, lookup)
                .map(Value::Bool);
        }
        self.finish_semantic_property_delete(deletion, lookup)
            .map(Value::Bool)
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
        let Some(cache) = self.static_name_atom_caches.pop() else {
            return Err(Error::runtime("static name atom cache disappeared"));
        };
        self.storage_ledger.release_count(
            crate::runtime::VmStorageKind::CacheEntry,
            cache.storage_entry_count()?,
        )
    }

    fn pop_static_binding_cache(&mut self) -> Result<()> {
        let Some(cache) = self.static_binding_caches.pop() else {
            return Err(Error::runtime("static binding cache disappeared"));
        };
        self.storage_ledger.release_count(
            crate::runtime::VmStorageKind::CacheEntry,
            cache.storage_entry_count()?,
        )
    }

    fn pop_static_binding_layout(&mut self) -> Result<()> {
        let Some(layout) = self.static_binding_layouts.pop() else {
            return Err(Error::runtime("static binding layout disappeared"));
        };
        self.storage_ledger.release_count(
            crate::runtime::VmStorageKind::CacheEntry,
            layout.storage_entry_count()?,
        )
    }
}
