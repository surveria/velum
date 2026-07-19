#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;
use core::cell::Cell;

use crate::{
    binding_metadata::BindingLayout,
    binding_metadata::{BindingOperand, DeclarationRef, FunctionScopeId, ScopeId},
    bytecode::BytecodeBinding,
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::{BindingCell, BindingScope, BindingSlot},
    runtime::control::reference_error_undefined,
    runtime::native::NativeFunctionKind,
    runtime::native::{INFINITY_NAME, NAN_NAME, UNDEFINED_NAME},
    runtime::object::{PropertyConfigurable, PropertyEnumerable, PropertyWritable},
    storage::atom::AtomId,
    syntax::{DeclKind, StaticBinding, StaticBindingId},
    value::{NativeFunctionId, Value},
};

use super::location::BindingLocation;

#[derive(Debug, Clone)]
pub struct StaticBindingCacheHandle {
    locations: Rc<[Cell<Option<BindingLocation>>]>,
    native_calls: Rc<[Cell<Option<StaticBindingNativeCallCache>>]>,
}

impl StaticBindingCacheHandle {
    pub(in crate::runtime) fn new(slot_count: usize) -> Self {
        let mut bindings = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            bindings.push(Cell::new(None));
        }
        let mut native_calls = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            native_calls.push(Cell::new(None));
        }
        Self {
            locations: Rc::from(bindings.into_boxed_slice()),
            native_calls: Rc::from(native_calls.into_boxed_slice()),
        }
    }

    pub(in crate::runtime) fn storage_entry_count(&self) -> Result<usize> {
        self.locations
            .len()
            .checked_add(self.native_calls.len())
            .ok_or_else(|| Error::limit("static binding cache entry count overflowed"))
    }

    pub(in crate::runtime) fn is_compatible(
        &self,
        binding: &StaticBinding,
        expected_atom: Option<AtomId>,
    ) -> Result<bool> {
        let Some(slot) = self.locations.get(binding.id().index()?) else {
            return Ok(false);
        };
        let Some(location) = slot.get() else {
            return Ok(true);
        };
        Ok(expected_atom.is_some_and(|atom| location.matches_atom(atom)))
    }

    pub(in crate::runtime) fn invalidate_identity_caches(&self) {
        for slot in self.native_calls.iter() {
            slot.set(None);
        }
    }

    pub(in crate::runtime::binding) fn location(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<BindingLocation>> {
        self.locations
            .get(binding.id().index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static binding cache slot is not defined"))
    }

    pub(in crate::runtime::binding) fn remember_id(
        &self,
        binding: StaticBindingId,
        location: BindingLocation,
    ) -> Result<()> {
        let slot = self
            .locations
            .get(binding.index()?)
            .ok_or_else(|| Error::runtime("static binding cache slot is not defined"))?;
        slot.set(Some(location));
        Ok(())
    }

    fn native_call(&self, binding: &StaticBinding) -> Result<Option<StaticBindingNativeCallCache>> {
        self.native_calls
            .get(binding.id().index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static binding native call cache slot is not defined"))
    }

    fn remember_native_call(
        &self,
        binding: &StaticBinding,
        function: NativeFunctionId,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let slot = self
            .native_calls
            .get(binding.id().index()?)
            .ok_or_else(|| {
                Error::runtime("static binding native call cache slot is not defined")
            })?;
        slot.set(Some(StaticBindingNativeCallCache::new(function, kind)));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct StaticBindingNativeCallCache {
    function: NativeFunctionId,
    kind: NativeFunctionKind,
}

impl StaticBindingNativeCallCache {
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

#[derive(Debug, Clone, Copy)]
pub struct CompiledBindingFrame {
    scope: Option<ScopeId>,
    slot: BindingSlot,
}

impl CompiledBindingFrame {
    pub const fn global(slot: BindingSlot) -> Self {
        Self { scope: None, slot }
    }

    pub const fn local(scope: ScopeId, slot: BindingSlot) -> Self {
        Self {
            scope: Some(scope),
            slot,
        }
    }

    pub const fn scope(self) -> Option<ScopeId> {
        self.scope
    }

    pub const fn slot(self) -> BindingSlot {
        self.slot
    }
}

impl Context {
    pub(crate) fn active_static_caches_are_compatible(
        &self,
        binding: &BytecodeBinding,
    ) -> Result<bool> {
        let expected_atom = self.atom(binding.name().as_str());
        let atom_addressable = match self.current_static_name_atom_cache_owner() {
            Some(cache) => cache.is_compatible(binding.name().name(), expected_atom)?,
            None => true,
        };
        let binding_addressable = match self.current_static_binding_cache_owner() {
            Some(cache) => cache.is_compatible(binding.name(), expected_atom)?,
            None => true,
        };
        Ok(atom_addressable && binding_addressable)
    }

    pub(crate) fn current_static_binding_cache_owner(&self) -> Option<StaticBindingCacheHandle> {
        self.static_binding_caches.last().cloned()
    }

    pub(crate) fn current_static_binding_cache(&self) -> Option<StaticBindingCacheHandle> {
        if !self.optional_optimizations_enabled() {
            return None;
        }
        self.static_binding_caches.last().cloned()
    }

    pub(in crate::runtime) fn cached_static_binding_native_call_kind(
        &self,
        binding: &StaticBinding,
        function: NativeFunctionId,
    ) -> Result<Option<NativeFunctionKind>> {
        let Some(cache) = self.current_static_binding_cache() else {
            return Ok(None);
        };
        Ok(cache
            .native_call(binding)?
            .and_then(|cached| cached.kind_if_current(function)))
    }

    pub(in crate::runtime) fn remember_static_binding_native_call_kind(
        &self,
        binding: &StaticBinding,
        function: NativeFunctionId,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let Some(cache) = self.current_static_binding_cache() else {
            return Ok(());
        };
        cache.remember_native_call(binding, function, kind)
    }

    pub(crate) fn current_static_binding_layout(
        &self,
    ) -> Option<crate::binding_metadata::BindingLayout> {
        self.static_binding_layouts.last().cloned()
    }

    pub(crate) fn compiled_local_binding_frame(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<CompiledBindingFrame>> {
        match self.compiled_binding_operand(binding.id())? {
            BindingOperand::Local { scope, slot } => Ok(Some(CompiledBindingFrame::local(
                scope,
                BindingSlot::from_index(slot.index()?),
            ))),
            BindingOperand::Global { .. }
            | BindingOperand::EvalVariable { .. }
            | BindingOperand::Upvalue { .. }
            | BindingOperand::Unresolved => Ok(None),
        }
    }

    pub(crate) fn compiled_active_binding_frame(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<CompiledBindingFrame>> {
        if self.has_visible_local_scope() {
            return self.compiled_local_binding_frame(binding);
        }
        match self.compiled_binding_operand(binding.id())? {
            BindingOperand::Global { slot } => Ok(Some(CompiledBindingFrame::global(
                BindingSlot::from_index(slot.index()?),
            ))),
            BindingOperand::Local { .. } | BindingOperand::Upvalue { .. } => {
                Err(Error::runtime("global binding layout is not a global slot"))
            }
            BindingOperand::EvalVariable { .. } | BindingOperand::Unresolved => Ok(None),
        }
    }

    pub(crate) fn mark_active_binding_frame_slot(
        &mut self,
        frame: Option<CompiledBindingFrame>,
        inserted: BindingSlot,
    ) -> Result<()> {
        let Some(frame) = frame else {
            return Ok(());
        };
        Self::mark_binding_scope_frame_slot(self.active_bindings_mut(), frame, inserted)
    }

    pub(crate) fn mark_binding_scope_frame_slot(
        scope: &mut BindingScope,
        frame: CompiledBindingFrame,
        inserted: BindingSlot,
    ) -> Result<()> {
        if frame.slot() != inserted {
            return Ok(());
        }
        let Some(scope_id) = frame.scope() else {
            return Ok(());
        };
        scope.mark_compiled_scope(scope_id)
    }

    pub(crate) fn get_binding_static(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<BindingCell>> {
        let operand = self.compiled_binding_operand(binding.id())?;
        if matches!(operand, BindingOperand::Global { .. }) {
            let Some(atom) = self.lookup_static_name_atom(binding.name())? else {
                return Ok(None);
            };
            if let Some(cell) = self.current_function_eval_variable_binding(atom)? {
                return Ok(Some(cell));
            }
        }
        if !self.optional_optimizations_enabled() {
            return self.get_binding_by_name(binding);
        }
        if let Some(cell) = self.cached_static_binding(binding)? {
            return Ok(Some(cell));
        }
        if let Some((location, cell)) = self.direct_compiled_static_binding(binding)? {
            self.remember_static_binding(binding, location)?;
            return Ok(Some(cell));
        }
        let Some(atom) = self.lookup_static_name_atom(binding.name())? else {
            return Ok(None);
        };
        if let Some((location, cell)) = self.compiled_global_static_binding(binding, atom)? {
            self.remember_static_binding(binding, location)?;
            return Ok(Some(cell));
        }
        if let Some((location, cell)) = self.compiled_local_static_binding(binding, atom)? {
            self.remember_static_binding(binding, location)?;
            return Ok(Some(cell));
        }
        if let Some((location, cell)) = self.compiled_upvalue_static_binding(binding)? {
            self.remember_static_binding(binding, location)?;
            return Ok(Some(cell));
        }
        let Some(location) = self.resolve_binding_location(atom) else {
            return Ok(None);
        };
        self.remember_static_binding(binding, location)?;
        self.binding_at_location(location)
    }

    fn get_binding_by_name(&self, binding: &StaticBinding) -> Result<Option<BindingCell>> {
        let Some(atom) = self.lookup_static_name_atom(binding.name())? else {
            return Ok(None);
        };
        let Some(location) = self.resolve_binding_location(atom) else {
            return Ok(None);
        };
        self.binding_at_location(location)
    }

    fn current_function_eval_variable_binding(&self, atom: AtomId) -> Result<Option<BindingCell>> {
        if !self.current_function_contains_sloppy_direct_eval()? {
            return Ok(None);
        }
        let Some(index) = self.current_function_variable_scope_index()? else {
            return Ok(None);
        };
        Ok(self.locals.get(index).and_then(|scope| scope.get(atom)))
    }

    pub(crate) fn get_binding_bytecode(
        &self,
        binding: &BytecodeBinding,
    ) -> Result<Option<BindingCell>> {
        if matches!(binding.operand(), BindingOperand::Global { .. }) {
            let Some(atom) = self.lookup_static_name_atom(binding.name().name())? else {
                return Ok(None);
            };
            if let Some(cell) = self.current_function_eval_variable_binding(atom)? {
                return Ok(Some(cell));
            }
        }
        if matches!(binding.operand(), BindingOperand::Upvalue { .. }) {
            let Some(atom) = self.lookup_static_name_atom(binding.name().name())? else {
                return Ok(None);
            };
            if let Some(location @ BindingLocation::Local { .. }) =
                self.resolve_binding_location(atom)
            {
                return self.binding_at_location(location);
            }
        }
        if let Some(cell) = self.cached_static_binding(binding.name())? {
            return Ok(Some(cell));
        }
        if let Some((location, cell)) = self.direct_bytecode_static_binding(binding.operand())? {
            self.remember_static_binding(binding.name(), location)?;
            return Ok(Some(cell));
        }
        self.get_binding_static(binding.name())
    }

    pub(crate) fn unresolved_builtin_numeric_constant(
        &self,
        binding: &BytecodeBinding,
    ) -> Option<Value> {
        if !self.optional_optimizations_enabled() {
            return None;
        }
        if binding.operand() != BindingOperand::Unresolved {
            return None;
        }
        let name = binding.name().as_str();
        let value = match name {
            NAN_NAME => Value::Number(f64::NAN),
            INFINITY_NAME => Value::Number(f64::INFINITY),
            _ => return None,
        };
        if self.unresolved_binding_name_is_shadowed(name) {
            return None;
        }
        Some(value)
    }

    pub(crate) fn unresolved_direct_builtin_callable(
        &mut self,
        binding: &BytecodeBinding,
    ) -> Result<Option<Value>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        if binding.operand() != BindingOperand::Unresolved {
            return Ok(None);
        }
        let name = binding.name().as_str();
        if self.unresolved_binding_name_is_shadowed(name) {
            return Ok(None);
        }
        self.direct_builtin_callable_value(name)
    }

    pub(crate) fn assign_bytecode(
        &mut self,
        binding: &BytecodeBinding,
        value: Value,
    ) -> Result<()> {
        if let Some(reference) = self.resolve_with_binding(binding)? {
            return reference.set(self, binding, value);
        }
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Err(reference_error_undefined(binding.name()));
        };
        self.assign_bytecode_cell(binding, &cell, value)
    }

    pub(crate) fn assign_bytecode_cell(
        &mut self,
        binding: &BytecodeBinding,
        cell: &BindingCell,
        value: Value,
    ) -> Result<()> {
        let value = self.checked_value(value)?;
        let Some(atom) = self.atom(binding.name().name()) else {
            return cell.assign_bytecode(binding.name(), value, binding.strict_write());
        };
        if let Some(global) = self.realm.globals.get(atom)
            && global.same_cell(cell)
            && global.kind() == DeclKind::Var
        {
            cell.assign_bytecode(binding.name(), value.clone(), binding.strict_write())?;
            let global_object = self.global_object_id()?;
            return self.update_global_object_data_property_value(
                global_object,
                binding.name().name(),
                value,
            );
        }
        if let Some(builtin) = self.realm.builtin_globals.get(atom)
            && builtin.same_cell(cell)
        {
            if matches!(
                binding.name().as_str(),
                NAN_NAME | INFINITY_NAME | UNDEFINED_NAME
            ) {
                return cell.assign_bytecode(binding.name(), value, binding.strict_write());
            }
            if builtin.kind() == DeclKind::Var {
                builtin.assign_bytecode(binding.name(), value.clone(), binding.strict_write())?;
            }
            let global_object = self.global_object_id()?;
            self.define_global_object_data_property(
                global_object,
                binding.name().name(),
                value,
                PropertyWritable::Yes,
                PropertyEnumerable::No,
                PropertyConfigurable::Yes,
            )?;
            return self
                .mark_global_object_property_authoritative(global_object, binding.name().name());
        }
        cell.assign_bytecode(binding.name(), value, binding.strict_write())
    }

    pub(crate) fn get_or_materialize_binding_bytecode(
        &mut self,
        binding: &BytecodeBinding,
    ) -> Result<Option<BindingCell>> {
        if let Some(cell) = self.get_binding_bytecode(binding)? {
            return Ok(Some(cell));
        }
        if self.global_object_name_is_authoritative(binding.name().name()) {
            return Ok(None);
        }
        if self.builtin_value(binding.name().name())?.is_none() {
            return Ok(None);
        }
        self.get_binding_bytecode(binding)
    }

    pub(crate) fn bytecode_binding_is_declarative(
        &self,
        binding: &BytecodeBinding,
    ) -> Result<bool> {
        if matches!(
            binding.operand(),
            BindingOperand::Global { .. } | BindingOperand::EvalVariable { .. }
        ) {
            let Some(cell) = self.get_binding_bytecode(binding)? else {
                return Ok(false);
            };
            let Some(atom) = self.lookup_static_name_atom(binding.name().name())? else {
                return Ok(false);
            };
            if self
                .realm
                .globals
                .get(atom)
                .is_some_and(|global| global.same_cell(&cell))
            {
                return self
                    .global_own_property_is_configurable(binding.name().name())
                    .map(|configurable| !configurable);
            }
            return Ok(false);
        }
        if binding.operand() != BindingOperand::Unresolved {
            if matches!(binding.operand(), BindingOperand::Upvalue { .. })
                && let Some(atom) = self.lookup_static_name_atom(binding.name().name())?
                && let Some(location @ BindingLocation::Local { .. }) =
                    self.resolve_binding_location(atom)
                && self
                    .binding_at_location(location)?
                    .is_some_and(|cell| cell.kind() == DeclKind::Var)
            {
                return Ok(false);
            }
            return Ok(true);
        }
        let Some(atom) = self.lookup_static_name_atom(binding.name().name())? else {
            return Ok(false);
        };
        if self.realm.globals.contains(atom)
            && self.global_own_property_is_configurable(binding.name().name())?
        {
            return Ok(false);
        }
        Ok(self
            .resolve_binding_location(atom)
            .is_some_and(|location| !matches!(location, BindingLocation::BuiltinGlobal { .. })))
    }

    fn global_own_property_is_configurable(&self, name: &str) -> Result<bool> {
        let Some(global_object) = self.realm.global_object else {
            return Ok(false);
        };
        self.objects
            .own_property_descriptor(global_object, self.property_lookup(name))
            .map(|descriptor| {
                descriptor.is_some_and(|descriptor| {
                    descriptor.configurable() == PropertyConfigurable::Yes
                })
            })
    }

    pub(crate) fn resolve_runtime_static_declaration(
        &self,
        layout: &BindingLayout,
        owner_function: FunctionScopeId,
        declaration: DeclarationRef,
        binding: &StaticBinding,
    ) -> Result<Option<BindingCell>> {
        let Some(atom) = self.lookup_static_name_atom(binding.name())? else {
            return Ok(None);
        };
        if let Some(cell) = self.compiled_declaration_static_binding(layout, declaration, atom)? {
            return Ok(Some(cell));
        }
        if let Some(cell) =
            self.compiled_parent_upvalue_static_binding(layout, owner_function, declaration)?
        {
            return Ok(Some(cell));
        }
        Ok(None)
    }

    pub(crate) fn remember_active_static_binding(
        &self,
        binding: &StaticBinding,
        atom: AtomId,
    ) -> Result<()> {
        self.remember_active_static_binding_id(binding.id(), atom)
    }

    pub(crate) fn remember_active_static_binding_id(
        &self,
        binding: StaticBindingId,
        atom: AtomId,
    ) -> Result<()> {
        let Some(cache) = self.current_static_binding_cache() else {
            return Ok(());
        };
        if let Some(location) = self.compiled_active_static_binding(binding, atom)? {
            return self.remember_layout_static_binding_id(&cache, binding, location);
        }
        if let Some(location) = self.resolve_binding_location(atom) {
            return self.remember_layout_static_binding_id(&cache, binding, location);
        }
        Ok(())
    }
}
