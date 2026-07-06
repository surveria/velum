use std::cell::Cell;
use std::rc::Rc;

use crate::{
    ast::StaticName,
    atom::AtomId,
    error::{Error, Result},
    runtime::Context,
    runtime_assertions::reference_error_undefined,
    runtime_object::{PropertyKey, PropertyLookup},
    runtime_property::{delete_property, get_property, set_property},
    runtime_scope::BindingCell,
    value::Value,
};

#[derive(Debug, Clone)]
pub struct StaticNameAtomCacheHandle(Rc<[Cell<Option<AtomId>>]>);

impl StaticNameAtomCacheHandle {
    pub(super) fn new(slot_count: usize) -> Self {
        let mut atoms = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            atoms.push(Cell::new(None));
        }
        Self(Rc::from(atoms.into_boxed_slice()))
    }

    fn atom(&self, name: &StaticName) -> Result<Option<AtomId>> {
        self.0
            .get(name.id().index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static name atom cache slot is not defined"))
    }

    fn remember(&self, name: &StaticName, atom: AtomId) -> Result<()> {
        let slot = self
            .0
            .get(name.id().index()?)
            .ok_or_else(|| Error::runtime("static name atom cache slot is not defined"))?;
        slot.set(Some(atom));
        Ok(())
    }
}

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

    pub(crate) fn get_binding_static(&self, name: &StaticName) -> Result<Option<BindingCell>> {
        let Some(atom) = self.lookup_static_name_atom(name)? else {
            return Ok(None);
        };
        Ok(self.get_binding_by_atom(atom))
    }

    pub(crate) fn assign_static(&self, name: &StaticName, value: Value) -> Result<()> {
        self.checked_value(value.clone())?;
        let Some(binding) = self.get_binding_static(name)? else {
            return Err(reference_error_undefined(name));
        };
        binding.assign(name, value)
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
    ) -> Result<Value> {
        if let Value::Function(id) = object {
            return self.get_function_property(*id, property);
        }
        if let Value::NativeFunction(id) = object {
            return self.get_native_function_property(*id, property);
        }
        self.checked_value(get_property(
            &self.objects,
            object,
            self.static_property_lookup(property)?,
        )?)
    }

    pub(crate) fn set_static_property_value(
        &mut self,
        object: &Value,
        property: &StaticName,
        value: Value,
    ) -> Result<()> {
        self.checked_value(value.clone())?;
        if let Value::Function(id) = object {
            return self.set_function_property(*id, property, value);
        }
        if let Value::NativeFunction(id) = object {
            return self.set_native_function_property(*id, property, value);
        }
        let key = self.intern_static_property_key(property)?;
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
        if let Value::Function(id) = object {
            return self
                .delete_function_property(*id, property)
                .map(Value::Bool);
        }
        if let Value::NativeFunction(id) = object {
            return self
                .delete_native_function_property(*id, property)
                .map(Value::Bool);
        }
        let lookup = self.static_property_lookup(property)?;
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
}
