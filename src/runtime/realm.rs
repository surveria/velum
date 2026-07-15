use std::collections::BTreeSet;

use crate::{
    error::{Error, Result},
    ownership::VmIdentity,
    runtime::{
        Context, VmStorageKind,
        abstract_operations::SetFailureBehavior,
        binding::scope::{BindingScope, BindingScopeStorageFootprint},
        native::{IntlFunctionKind, NativeFunctionKind, NativeFunctionRegistry},
        storage_ledger::VmStorageLedger,
    },
    storage::atom::AtomId,
    value::{HostFunctionId, NativeFunctionId, ObjectId, Value},
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
    pub(super) object_global_names: BTreeSet<AtomId>,
    pub(super) native_function_registry: NativeFunctionRegistry,
    pub(super) throw_type_error: Option<NativeFunctionId>,
    pub(super) shadow_realm_constructor: Option<NativeFunctionId>,
    pub(super) abstract_module_source_constructor: Option<HostFunctionId>,
    pub(super) global_object: Option<ObjectId>,
    pub(super) object_prototype: Option<ObjectId>,
    pub(super) array_prototype: Option<ObjectId>,
    pub(super) generator_prototype: Option<ObjectId>,
    pub(super) generator_function_prototype: Option<ObjectId>,
    pub(super) async_iterator_prototype: Option<ObjectId>,
    pub(super) async_generator_prototype: Option<ObjectId>,
    pub(super) async_generator_function_prototype: Option<ObjectId>,
    pub(super) promise_prototype: Option<ObjectId>,
    pub(super) shadow_realm_prototype: Option<ObjectId>,
    pub(super) abstract_module_source_prototype: Option<ObjectId>,
}

impl RealmState {
    pub(super) fn new(storage_ledger: VmStorageLedger) -> Self {
        Self {
            globals: BindingScope::new_active(storage_ledger.clone()),
            builtin_globals: BindingScope::new_active(storage_ledger),
            object_global_names: BTreeSet::new(),
            native_function_registry: NativeFunctionRegistry::new(),
            throw_type_error: None,
            shadow_realm_constructor: None,
            abstract_module_source_constructor: None,
            global_object: None,
            object_prototype: None,
            array_prototype: None,
            generator_prototype: None,
            generator_function_prototype: None,
            async_iterator_prototype: None,
            async_generator_prototype: None,
            async_generator_function_prototype: None,
            promise_prototype: None,
            shadow_realm_prototype: None,
            abstract_module_source_prototype: None,
        }
    }

    fn binding_scope_storage_footprint(&self) -> Result<BindingScopeStorageFootprint> {
        self.globals
            .storage_footprint()?
            .checked_add(self.builtin_globals.storage_footprint()?)
    }

    pub(super) fn binding_count(&self) -> Result<usize> {
        Ok(self.binding_scope_storage_footprint()?.binding_count())
    }

    pub(super) fn cache_entry_count(&self) -> Result<usize> {
        self.binding_scope_storage_footprint()?
            .cache_entry_count()
            .checked_add(self.object_global_names.len())
            .and_then(|count| count.checked_add(self.native_function_registry.ids().count()))
            .ok_or_else(|| Error::limit("realm cache entry count overflowed"))
    }

    pub(super) fn association_count(&self) -> usize {
        [
            self.global_object,
            self.object_prototype,
            self.array_prototype,
            self.promise_prototype,
            self.shadow_realm_prototype,
            self.abstract_module_source_prototype,
            self.generator_prototype,
            self.generator_function_prototype,
            self.async_iterator_prototype,
            self.async_generator_prototype,
            self.async_generator_function_prototype,
        ]
        .into_iter()
        .flatten()
        .count()
        .saturating_add(usize::from(self.throw_type_error.is_some()))
        .saturating_add(usize::from(self.shadow_realm_constructor.is_some()))
        .saturating_add(usize::from(
            self.abstract_module_source_constructor.is_some(),
        ))
    }

    pub(super) fn anchor_objects(&self) -> impl Iterator<Item = ObjectId> {
        [
            self.global_object,
            self.object_prototype,
            self.array_prototype,
            self.promise_prototype,
            self.shadow_realm_prototype,
            self.abstract_module_source_prototype,
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
        self.native_function_registry
            .ids()
            .chain(self.throw_type_error)
            .chain(self.shadow_realm_constructor)
    }

    pub(super) fn host_function_ids(&self) -> impl Iterator<Item = HostFunctionId> + '_ {
        self.abstract_module_source_constructor.into_iter()
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
        let index = self.create_realm_index()?;
        Ok(RealmId::new(self.identity.clone(), index))
    }

    pub(in crate::runtime) fn create_realm_index(&mut self) -> Result<RealmIndex> {
        self.inactive_realms
            .try_reserve(1)
            .map_err(|error| Error::limit(format!("realm storage exhausted: {error}")))?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        let index = RealmIndex::new(self.inactive_realms.len());
        self.inactive_realms
            .push(Some(RealmState::new(self.storage_ledger.clone())));
        Ok(index)
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

    pub(crate) fn with_realm_id<T>(
        &mut self,
        realm: &RealmId,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let index = self.validate_realm(realm)?;
        self.with_realm(index, operation)
    }

    pub(crate) fn eval_realm_source_value(
        &mut self,
        realm: &RealmId,
        source: &Value,
    ) -> Result<Value> {
        let index = self.validate_realm(realm)?;
        self.with_realm(index, |context| context.eval_script_source_value(source))
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

    pub(crate) fn with_global_object_realm<T>(
        &mut self,
        global: ObjectId,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let realm = self
            .global_object_realm(global)
            .ok_or_else(|| Error::type_error("object is not a realm global"))?;
        self.with_realm(realm, operation)
    }

    pub(in crate::runtime) fn constructor_instance_prototype_with_default(
        &mut self,
        new_target: &Value,
        default_kind: NativeFunctionKind,
    ) -> Result<ObjectId> {
        let prototype =
            self.constructor_instance_semantic_prototype_with_default(new_target, default_kind)?;
        let Value::Object(prototype) = prototype else {
            return Err(Error::runtime(
                "constructor requires an ordinary-object prototype",
            ));
        };
        Ok(prototype)
    }

    pub(in crate::runtime) fn constructor_instance_semantic_prototype_with_default(
        &mut self,
        new_target: &Value,
        default_kind: NativeFunctionKind,
    ) -> Result<Value> {
        let prototype =
            self.get_named(new_target, crate::runtime::CONSTRUCTOR_PROTOTYPE_PROPERTY)?;
        if self.semantic_object_ref(&prototype)?.is_some() {
            return Ok(prototype);
        }
        let realm = self.callable_realm_index(new_target)?;
        self.with_realm(realm, |context| {
            context
                .native_constructor_default_prototype(default_kind)
                .map(Value::Object)
        })
    }

    pub(in crate::runtime) fn callable_realm_index(&self, value: &Value) -> Result<RealmIndex> {
        let mut current = value.clone();
        let mut depth = 0_usize;
        loop {
            if depth > self.limits.max_expression_depth {
                return Err(Error::limit("function realm resolution depth exceeded"));
            }
            match current {
                Value::Function(id) => return self.function(id).map(|function| function.realm),
                Value::NativeFunction(id) => {
                    let function = self.native_function(id)?;
                    if let NativeFunctionKind::BoundFunction(bound) = function.kind() {
                        if self.bound_function_is_shadow_realm(bound)? {
                            return Ok(function.realm());
                        }
                        current = self.bound_function_target(bound)?;
                    } else {
                        return Ok(function.realm());
                    }
                }
                Value::Object(id) => {
                    let proxy = self
                        .objects
                        .proxy_value(id)?
                        .ok_or_else(|| Error::type_error("new.target is not a constructor"))?;
                    current = proxy.target().cloned().ok_or_else(|| {
                        Error::type_error("Cannot resolve the realm of a revoked Proxy")
                    })?;
                }
                Value::HostFunction(_)
                | Value::Undefined
                | Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::BigInt(_)
                | Value::String(_)
                | Value::Symbol(_) => {
                    return Err(Error::type_error("new.target is not a constructor"));
                }
            }
            depth = depth
                .checked_add(1)
                .ok_or_else(|| Error::limit("function realm resolution depth overflowed"))?;
        }
    }

    fn native_constructor_default_prototype(
        &mut self,
        kind: NativeFunctionKind,
    ) -> Result<ObjectId> {
        let constructor = match kind {
            NativeFunctionKind::AsyncFunction => self.async_function_constructor_value()?,
            NativeFunctionKind::AsyncGeneratorFunction => {
                self.async_generator_function_constructor_value()?
            }
            NativeFunctionKind::GeneratorFunction => self.generator_function_constructor_value()?,
            NativeFunctionKind::ErrorConstructor(name) => self.error_constructor_value(name)?,
            NativeFunctionKind::Intl(IntlFunctionKind::NumberFormatConstructor) => {
                self.intl_number_format_constructor_value()?
            }
            NativeFunctionKind::Intl(IntlFunctionKind::DateTimeFormatConstructor) => {
                self.intl_date_time_format_constructor_value()?
            }
            NativeFunctionKind::Intl(IntlFunctionKind::LocaleConstructor) => {
                self.intl_locale_constructor_value()?
            }
            NativeFunctionKind::Intl(IntlFunctionKind::ListFormatConstructor) => {
                self.intl_list_format_constructor_value()?
            }
            NativeFunctionKind::Intl(IntlFunctionKind::SegmenterConstructor) => {
                self.intl_segmenter_constructor_value()?
            }
            NativeFunctionKind::Intl(IntlFunctionKind::DisplayNamesConstructor) => {
                self.intl_display_names_constructor_value()?
            }
            NativeFunctionKind::Intl(IntlFunctionKind::CollatorConstructor) => {
                self.intl_collator_constructor_value()?
            }
            NativeFunctionKind::Intl(IntlFunctionKind::DurationFormatConstructor) => {
                self.intl_duration_format_constructor_value()?
            }
            NativeFunctionKind::Intl(IntlFunctionKind::PluralRulesConstructor) => {
                self.intl_plural_rules_constructor_value()?
            }
            NativeFunctionKind::Intl(IntlFunctionKind::RelativeTimeFormatConstructor) => {
                self.intl_relative_time_format_constructor_value()?
            }
            _ => self
                .builtin_value(kind.name())?
                .ok_or_else(|| Error::runtime("default intrinsic constructor is unavailable"))?,
        };
        let Value::NativeFunction(id) = constructor else {
            return Err(Error::runtime(
                "default intrinsic constructor is not a native function",
            ));
        };
        let Value::Object(prototype) = self.native_function(id)?.properties().prototype() else {
            return Err(Error::runtime(
                "default intrinsic constructor prototype is not an object",
            ));
        };
        Ok(prototype)
    }

    pub(in crate::runtime) fn eval_native_function_in_realm(
        &mut self,
        id: NativeFunctionId,
        kind: NativeFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let realm = self.native_function(id)?.realm();
        self.with_realm(realm, |context| {
            context.eval_direct_or_generic_native_function_kind(kind, args, this_value)
        })
    }

    pub(in crate::runtime) fn is_foreign_intrinsic_array_constructor(
        &mut self,
        constructor: &Value,
    ) -> Result<bool> {
        let realm = self.callable_realm_index(constructor)?;
        if realm == self.active_realm {
            return Ok(false);
        }
        self.with_realm(realm, |context| {
            let intrinsic = context.array_constructor_value()?;
            Ok(constructor == &intrinsic)
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
        let result = match operation(self) {
            Err(error) if error.javascript_error_request().is_some() => {
                match crate::runtime::control::runtime_exception_value(self, &error) {
                    Ok(Some(value)) => Err(Error::javascript_local(self.identity.clone(), value)),
                    Ok(None) => Err(error),
                    Err(conversion_error) => Err(conversion_error),
                }
            }
            result => result,
        };
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
        let previous = self.active_realm;
        if !self
            .inactive_realms
            .get(previous.index())
            .is_some_and(Option::is_none)
        {
            return Err(Error::runtime("active realm slot is occupied"));
        }
        let target_state = self
            .inactive_realms
            .get_mut(target.index())
            .ok_or_else(|| Error::runtime("realm is not defined"))?
            .take()
            .ok_or_else(|| Error::runtime("realm is already active"))?;
        std::mem::swap(
            &mut self.realm.object_prototype,
            &mut self.objects.object_prototype,
        );
        std::mem::swap(
            &mut self.realm.array_prototype,
            &mut self.objects.array_prototype,
        );
        let previous_state = std::mem::replace(&mut self.realm, target_state);
        let previous_slot = self
            .inactive_realms
            .get_mut(previous.index())
            .ok_or_else(|| Error::runtime("active realm slot is not defined"))?;
        *previous_slot = Some(previous_state);
        self.active_realm = target;
        std::mem::swap(
            &mut self.realm.object_prototype,
            &mut self.objects.object_prototype,
        );
        std::mem::swap(
            &mut self.realm.array_prototype,
            &mut self.objects.array_prototype,
        );
        Ok(())
    }
}
