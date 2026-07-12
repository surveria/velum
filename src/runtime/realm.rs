use crate::{
    error::{Error, Result},
    ownership::VmIdentity,
    runtime::{
        Context, VmStorageKind, abstract_operations::SetFailureBehavior,
        binding::scope::BindingScope, native::NativeFunctionRegistry,
        storage_ledger::VmStorageLedger,
    },
    value::{NativeFunctionId, ObjectId, Value},
};

/// Opaque handle for one realm owned by a JavaScript VM.
///
/// A handle is valid only for the VM that created it. Realms inside that VM
/// share values and heap identity while retaining independent globals and
/// intrinsic objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RealmId {
    identity: VmIdentity,
    index: RealmIndex,
}

impl RealmId {
    const fn new(identity: VmIdentity, index: RealmIndex) -> Self {
        Self { identity, index }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct RealmIndex(usize);

impl RealmIndex {
    pub(in crate::runtime) const ROOT: Self = Self(0);

    const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(in crate::runtime) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub(in crate::runtime) struct RealmState {
    pub(super) globals: BindingScope,
    pub(super) builtin_globals: BindingScope,
    pub(super) native_function_registry: NativeFunctionRegistry,
    pub(super) global_object: Option<ObjectId>,
    pub(super) generator_prototype: Option<ObjectId>,
    pub(super) generator_function_prototype: Option<ObjectId>,
    pub(super) async_iterator_prototype: Option<ObjectId>,
    pub(super) async_generator_prototype: Option<ObjectId>,
    pub(super) async_generator_function_prototype: Option<ObjectId>,
    pub(super) promise_prototype: Option<ObjectId>,
}

impl RealmState {
    pub(super) fn new(storage_ledger: VmStorageLedger) -> Self {
        Self {
            globals: BindingScope::new_active(storage_ledger.clone()),
            builtin_globals: BindingScope::new_active(storage_ledger),
            native_function_registry: NativeFunctionRegistry::new(),
            global_object: None,
            generator_prototype: None,
            generator_function_prototype: None,
            async_iterator_prototype: None,
            async_generator_prototype: None,
            async_generator_function_prototype: None,
            promise_prototype: None,
        }
    }

    pub(super) fn binding_count(&self) -> Result<usize> {
        self.globals
            .len()
            .checked_add(self.builtin_globals.len())
            .ok_or_else(|| Error::limit("realm binding count overflowed"))
    }

    pub(super) fn cache_entry_count(&self) -> Result<usize> {
        self.globals
            .index_entry_count()?
            .checked_add(self.builtin_globals.index_entry_count()?)
            .and_then(|count| count.checked_add(self.native_function_registry.ids().count()))
            .ok_or_else(|| Error::limit("realm cache entry count overflowed"))
    }

    pub(super) fn association_count(&self) -> usize {
        [
            self.global_object,
            self.promise_prototype,
            self.generator_prototype,
            self.generator_function_prototype,
            self.async_iterator_prototype,
            self.async_generator_prototype,
            self.async_generator_function_prototype,
        ]
        .into_iter()
        .flatten()
        .count()
    }

    pub(super) fn anchor_objects(&self) -> impl Iterator<Item = ObjectId> {
        [
            self.global_object,
            self.promise_prototype,
            self.generator_prototype,
            self.generator_function_prototype,
            self.async_iterator_prototype,
            self.async_generator_prototype,
            self.async_generator_function_prototype,
        ]
        .into_iter()
        .flatten()
    }

    pub(super) fn native_function_ids(&self) -> impl Iterator<Item = NativeFunctionId> + '_ {
        self.native_function_registry.ids()
    }
}

impl Context {
    /// Returns the currently active realm.
    #[must_use]
    pub fn current_realm(&self) -> RealmId {
        RealmId::new(self.identity.clone(), self.active_realm)
    }

    /// Creates an independent realm in this VM.
    ///
    /// The new realm shares VM-owned values and heap identity with its creator,
    /// but starts with separate globals and lazily materialized intrinsics.
    ///
    /// # Errors
    /// Fails when realm bookkeeping exceeds configured VM storage limits.
    pub fn create_realm(&mut self) -> Result<RealmId> {
        self.inactive_realms
            .try_reserve(1)
            .map_err(|error| Error::limit(format!("realm storage exhausted: {error}")))?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        let index = RealmIndex::new(self.inactive_realms.len());
        self.inactive_realms
            .push(Some(RealmState::new(self.storage_ledger.clone())));
        Ok(RealmId::new(self.identity.clone(), index))
    }

    /// Returns a realm's global object as a raw VM-local value.
    ///
    /// # Errors
    /// Fails when `realm` belongs to another VM or its state is unavailable.
    pub fn realm_global(&mut self, realm: &RealmId) -> Result<Value> {
        let index = self.validate_realm(realm)?;
        self.with_realm(index, Self::global_this_value)
    }

    /// Evaluates script source in a realm owned by this VM.
    ///
    /// # Errors
    /// Fails when `realm` belongs to another VM, or compilation, evaluation,
    /// or configured resource limits fail.
    pub fn eval_in_realm(&mut self, realm: &RealmId, source: &str) -> Result<Value> {
        let index = self.validate_realm(realm)?;
        self.with_realm(index, |context| context.eval(source))
    }

    pub(crate) fn install_realm_global_eval(
        &mut self,
        realm: &RealmId,
        eval: Value,
    ) -> Result<Value> {
        let index = self.validate_realm(realm)?;
        self.with_realm(index, |context| {
            let global = context.global_this_value()?;
            let lookup = context.property_lookup("eval");
            context.set(&global, lookup, eval, &global, SetFailureBehavior::Throw)?;
            Ok(global)
        })
    }

    pub(crate) fn eval_realm_source_value(
        &mut self,
        realm: &RealmId,
        source: &Value,
    ) -> Result<Value> {
        let source = self.to_string(source)?;
        self.eval_in_realm(realm, &source)
    }

    pub(in crate::runtime) const fn active_realm_index(&self) -> RealmIndex {
        self.active_realm
    }

    pub(in crate::runtime) fn realm_states(&self) -> impl Iterator<Item = &RealmState> {
        std::iter::once(&self.realm).chain(self.inactive_realms.iter().filter_map(Option::as_ref))
    }

    pub(in crate::runtime) fn global_object_realm(&self, id: ObjectId) -> Option<RealmIndex> {
        if self.realm.global_object == Some(id) {
            return Some(self.active_realm);
        }
        self.inactive_realms
            .iter()
            .enumerate()
            .find_map(|(index, state)| {
                state
                    .as_ref()
                    .is_some_and(|realm| realm.global_object == Some(id))
                    .then_some(RealmIndex::new(index))
            })
    }

    pub(in crate::runtime) fn with_realm<T>(
        &mut self,
        target: RealmIndex,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let previous = self.active_realm;
        if target == previous {
            return operation(self);
        }
        self.swap_active_realm(target)?;
        let result = operation(self);
        let restore_result = self.swap_active_realm(previous);
        restore_result?;
        result
    }

    fn validate_realm(&self, realm: &RealmId) -> Result<RealmIndex> {
        if realm.identity != self.identity {
            return Err(Error::runtime("realm belongs to another VM"));
        }
        if realm.index == self.active_realm {
            return Ok(realm.index);
        }
        if self
            .inactive_realms
            .get(realm.index.index())
            .is_some_and(Option::is_some)
        {
            return Ok(realm.index);
        }
        Err(Error::runtime("realm is not defined"))
    }

    fn swap_active_realm(&mut self, target: RealmIndex) -> Result<()> {
        let target_state = self
            .inactive_realms
            .get_mut(target.index())
            .ok_or_else(|| Error::runtime("realm is not defined"))?
            .take()
            .ok_or_else(|| Error::runtime("realm is already active"))?;
        let previous_state = std::mem::replace(&mut self.realm, target_state);
        let previous_slot = self
            .inactive_realms
            .get_mut(self.active_realm.index())
            .ok_or_else(|| Error::runtime("active realm slot is not defined"))?;
        if previous_slot.is_some() {
            return Err(Error::runtime("active realm slot is occupied"));
        }
        *previous_slot = Some(previous_state);
        self.active_realm = target;
        Ok(())
    }
}
