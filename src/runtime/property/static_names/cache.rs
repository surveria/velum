use std::cell::Cell;
use std::rc::Rc;

use crate::{
    error::{Error, Result},
    runtime::native::NativeFunctionKind,
    runtime::object::CacheablePropertyLookup,
    storage::atom::AtomId,
    syntax::{StaticCallSiteId, StaticName, StaticPropertyAccessId},
    value::{FunctionId, HostFunctionId, NativeFunctionId, Value},
};

use super::native_call_cache::StaticPropertyNativeCallCache;

#[derive(Debug, Clone)]
pub struct StaticNameAtomCacheHandle {
    atoms: Rc<[Cell<Option<AtomId>>]>,
    property_lookups: Rc<[Cell<Option<CacheablePropertyLookup>>]>,
    native_calls: Rc<[Cell<Option<StaticPropertyNativeCallCache>>]>,
    call_values: Rc<[Cell<Option<CallValueCache>>]>,
}

impl StaticNameAtomCacheHandle {
    pub(in crate::runtime) fn new(
        static_name_count: usize,
        static_property_access_count: usize,
        static_call_site_count: usize,
    ) -> Self {
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
        let mut call_values = Vec::with_capacity(static_call_site_count);
        for _ in 0..static_call_site_count {
            call_values.push(Cell::new(None));
        }
        Self {
            atoms: Rc::from(atoms.into_boxed_slice()),
            property_lookups: Rc::from(property_lookups.into_boxed_slice()),
            native_calls: Rc::from(native_calls.into_boxed_slice()),
            call_values: Rc::from(call_values.into_boxed_slice()),
        }
    }

    pub(in crate::runtime) fn storage_entry_count(&self) -> Result<usize> {
        self.atoms
            .len()
            .checked_add(self.property_lookups.len())
            .and_then(|count| count.checked_add(self.native_calls.len()))
            .and_then(|count| count.checked_add(self.call_values.len()))
            .ok_or_else(|| Error::limit("static name cache entry count overflowed"))
    }

    pub(in crate::runtime) fn invalidate_identity_caches(&self) {
        for slot in self.native_calls.iter() {
            slot.set(None);
        }
        for slot in self.call_values.iter() {
            slot.set(None);
        }
    }

    pub(super) fn atom(&self, name: &StaticName) -> Result<Option<AtomId>> {
        self.atoms
            .get(name.id().index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static name atom cache slot is not defined"))
    }

    pub(super) fn remember(&self, name: &StaticName, atom: AtomId) -> Result<()> {
        let slot = self
            .atoms
            .get(name.id().index()?)
            .ok_or_else(|| Error::runtime("static name atom cache slot is not defined"))?;
        slot.set(Some(atom));
        Ok(())
    }

    pub(super) fn property_lookup(
        &self,
        access: StaticPropertyAccessId,
    ) -> Result<Option<CacheablePropertyLookup>> {
        self.property_lookups
            .get(access.index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static property cache slot is not defined"))
    }

    pub(super) fn remember_property_lookup(
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

    pub(super) fn native_call(
        &self,
        access: StaticPropertyAccessId,
    ) -> Result<Option<StaticPropertyNativeCallCache>> {
        self.native_calls
            .get(access.index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static property native call cache slot is not defined"))
    }

    pub(super) fn remember_native_call(
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

    pub(super) fn remember_object_property_native_call(
        &self,
        access: StaticPropertyAccessId,
        lookup: CacheablePropertyLookup,
        version: u64,
        function: NativeFunctionId,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let slot = self.native_calls.get(access.index()?).ok_or_else(|| {
            Error::runtime("static property native call cache slot is not defined")
        })?;
        slot.set(Some(StaticPropertyNativeCallCache::new_object_property(
            lookup, version, function, kind,
        )));
        Ok(())
    }

    pub(super) fn call_value(&self, site: StaticCallSiteId) -> Result<Option<CallValueCache>> {
        self.call_values
            .get(site.index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static call value cache slot is not defined"))
    }

    pub(super) fn remember_call_value(
        &self,
        site: StaticCallSiteId,
        cache: CallValueCache,
    ) -> Result<()> {
        let slot = self
            .call_values
            .get(site.index()?)
            .ok_or_else(|| Error::runtime("static call value cache slot is not defined"))?;
        slot.set(Some(cache));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::runtime) enum CallValueCache {
    Function(FunctionId),
    NativeFunction {
        function: NativeFunctionId,
        kind: NativeFunctionKind,
    },
    HostFunction(HostFunctionId),
}

impl CallValueCache {
    pub(in crate::runtime) fn from_callee(
        callee: &Value,
        native_kind: Option<NativeFunctionKind>,
    ) -> Option<Self> {
        match callee {
            Value::Function(id) => Some(Self::Function(*id)),
            Value::NativeFunction(function) => native_kind.map(|kind| Self::NativeFunction {
                function: *function,
                kind,
            }),
            Value::HostFunction(id) => Some(Self::HostFunction(*id)),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Object(_) => None,
        }
    }

    pub(in crate::runtime) const fn matches_callee(self, callee: &Value) -> bool {
        matches!(
            (self, callee),
            (Self::Function(expected), Value::Function(actual)) if expected.index() == actual.index()
        ) || matches!(
            (self, callee),
            (Self::NativeFunction { function: expected, .. }, Value::NativeFunction(actual))
                if expected.index() == actual.index()
        ) || matches!(
            (self, callee),
            (Self::HostFunction(expected), Value::HostFunction(actual)) if expected.index() == actual.index()
        )
    }
}
