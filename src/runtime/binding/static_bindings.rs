use std::cell::Cell;
use std::rc::Rc;

use crate::{
    binding_layout::BindingLayout,
    binding_layout::{BindingOperand, DeclarationRef, FunctionScopeId, ScopeId},
    bytecode::BytecodeBinding,
    error::{Error, Result},
    runtime::Context,
    runtime::assertions::reference_error_undefined,
    runtime::binding::scope::{BindingCell, BindingScope, BindingSlot},
    runtime::native::NativeFunctionKind,
    storage::atom::AtomId,
    syntax::{StaticBinding, StaticBindingId},
    value::{NativeFunctionId, Value},
};

use super::location::{BindingLocation, LocalScopeIndex};

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

    fn location(&self, binding: &StaticBinding) -> Result<Option<BindingLocation>> {
        self.locations
            .get(binding.id().index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static binding cache slot is not defined"))
    }

    fn remember_id(&self, binding: StaticBindingId, location: BindingLocation) -> Result<()> {
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
    pub(crate) fn current_static_binding_cache(&self) -> Option<StaticBindingCacheHandle> {
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
    ) -> Option<crate::binding_layout::BindingLayout> {
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
            | BindingOperand::Upvalue { .. }
            | BindingOperand::Unresolved => Ok(None),
        }
    }

    pub(crate) fn compiled_active_binding_frame(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<CompiledBindingFrame>> {
        if self.locals.last().is_some() {
            return self.compiled_local_binding_frame(binding);
        }
        match self.compiled_binding_operand(binding.id())? {
            BindingOperand::Global { slot } => Ok(Some(CompiledBindingFrame::global(
                BindingSlot::from_index(slot.index()?),
            ))),
            BindingOperand::Local { .. } | BindingOperand::Upvalue { .. } => {
                Err(Error::runtime("global binding layout is not a global slot"))
            }
            BindingOperand::Unresolved => Ok(None),
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

    pub(crate) fn get_binding_bytecode(
        &self,
        binding: &BytecodeBinding,
    ) -> Result<Option<BindingCell>> {
        if let Some(cell) = self.cached_static_binding(binding.name())? {
            return Ok(Some(cell));
        }
        if let Some((location, cell)) = self.direct_bytecode_static_binding(binding.operand())? {
            self.remember_static_binding(binding.name(), location)?;
            return Ok(Some(cell));
        }
        self.get_binding_static(binding.name())
    }

    pub(crate) fn assign_bytecode(
        &mut self,
        binding: &BytecodeBinding,
        value: Value,
    ) -> Result<()> {
        let value = self.runtime_value(value)?;
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Err(reference_error_undefined(binding.name()));
        };
        cell.assign(binding.name(), value)
    }

    pub(crate) fn assign_bytecode_or_builtin(
        &mut self,
        binding: &BytecodeBinding,
        value: Value,
    ) -> Result<()> {
        let value = self.runtime_value(value)?;
        let Some(cell) = self.get_or_materialize_binding_bytecode(binding)? else {
            return Err(reference_error_undefined(binding.name()));
        };
        cell.assign(binding.name(), value)
    }

    pub(crate) fn get_or_materialize_binding_bytecode(
        &mut self,
        binding: &BytecodeBinding,
    ) -> Result<Option<BindingCell>> {
        if let Some(cell) = self.get_binding_bytecode(binding)? {
            return Ok(Some(cell));
        }
        if self.builtin_value(binding.name().name())?.is_none() {
            return Ok(None);
        }
        self.get_binding_bytecode(binding)
    }

    pub(crate) fn binding_exists_or_materialize_bytecode(
        &mut self,
        binding: &BytecodeBinding,
    ) -> Result<bool> {
        self.get_or_materialize_binding_bytecode(binding)
            .map(|binding| binding.is_some())
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

    fn compiled_binding_operand(&self, binding: StaticBindingId) -> Result<BindingOperand> {
        let Some(layout) = self.current_static_binding_layout() else {
            return Ok(BindingOperand::Unresolved);
        };
        Ok(layout
            .operand_for_binding_id(binding)?
            .unwrap_or(BindingOperand::Unresolved))
    }

    fn direct_compiled_static_binding(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        self.direct_bytecode_static_binding(self.compiled_binding_operand(binding.id())?)
    }

    fn direct_bytecode_static_binding(
        &self,
        operand: BindingOperand,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        match operand {
            BindingOperand::Local { scope, slot } => {
                self.direct_compiled_local_static_binding(scope, slot)
            }
            BindingOperand::Upvalue { slot, .. } => {
                let location = BindingLocation::upvalue(BindingSlot::from_index(slot.index()?));
                let Some(cell) = self.binding_at_location(location)? else {
                    return Ok(None);
                };
                Ok(Some((location, cell)))
            }
            BindingOperand::Global { .. } | BindingOperand::Unresolved => Ok(None),
        }
    }

    fn direct_compiled_local_static_binding(
        &self,
        scope: ScopeId,
        slot: crate::binding_layout::LocalSlot,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        let slot = BindingSlot::from_index(slot.index()?);
        for (index, frame) in self.locals.iter().enumerate().rev() {
            if frame.compiled_scope() != Some(scope) {
                continue;
            }
            let location = BindingLocation::ExactLocal {
                frame: LocalScopeIndex::new(index),
                compiled_scope: scope,
                slot,
            };
            let Some(cell) = self.binding_at_location(location)? else {
                return Ok(None);
            };
            return Ok(Some((location, cell)));
        }
        Ok(None)
    }

    fn compiled_global_static_binding(
        &self,
        binding: &StaticBinding,
        atom: AtomId,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        let BindingOperand::Global { slot } = self.compiled_binding_operand(binding.id())? else {
            return Ok(None);
        };
        let slot = BindingSlot::from_index(slot.index()?);
        let location = self.compiled_global_location(atom, slot);
        let Some(cell) = self.binding_at_location(location)? else {
            return Ok(None);
        };
        Ok(Some((location, cell)))
    }

    fn compiled_local_static_binding(
        &self,
        binding: &StaticBinding,
        atom: AtomId,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        let BindingOperand::Local { scope, slot } = self.compiled_binding_operand(binding.id())?
        else {
            return Ok(None);
        };
        let slot = BindingSlot::from_index(slot.index()?);
        for (index, frame) in self.locals.iter().enumerate().rev() {
            if frame.compiled_scope() != Some(scope) {
                continue;
            }
            let location = BindingLocation::local(atom, LocalScopeIndex::new(index), slot);
            let Some(cell) = self.binding_at_location(location)? else {
                return Ok(None);
            };
            return Ok(Some((location, cell)));
        }
        Ok(None)
    }

    fn compiled_upvalue_static_binding(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        let BindingOperand::Upvalue { slot, .. } = self.compiled_binding_operand(binding.id())?
        else {
            return Ok(None);
        };
        let location = BindingLocation::upvalue(BindingSlot::from_index(slot.index()?));
        let Some(cell) = self.binding_at_location(location)? else {
            return Ok(None);
        };
        Ok(Some((location, cell)))
    }

    fn compiled_declaration_static_binding(
        &self,
        layout: &BindingLayout,
        declaration: DeclarationRef,
        atom: AtomId,
    ) -> Result<Option<BindingCell>> {
        let Some(operand) = layout.declaration_operand(declaration)? else {
            return Ok(None);
        };
        match operand {
            BindingOperand::Global { slot } => {
                let location =
                    self.compiled_global_location(atom, BindingSlot::from_index(slot.index()?));
                self.binding_at_location(location)
            }
            BindingOperand::Local { scope, slot } => {
                self.compiled_declaration_local_binding(scope, slot)
            }
            BindingOperand::Upvalue { .. } | BindingOperand::Unresolved => Ok(None),
        }
    }

    fn compiled_declaration_local_binding(
        &self,
        scope: ScopeId,
        slot: crate::binding_layout::LocalSlot,
    ) -> Result<Option<BindingCell>> {
        let slot = BindingSlot::from_index(slot.index()?);
        for (index, frame) in self.locals.iter().enumerate().rev() {
            if frame.compiled_scope() != Some(scope) {
                continue;
            }
            let location = BindingLocation::ExactLocal {
                frame: LocalScopeIndex::new(index),
                compiled_scope: scope,
                slot,
            };
            return self.binding_at_location(location);
        }
        Ok(None)
    }

    fn compiled_parent_upvalue_static_binding(
        &self,
        layout: &BindingLayout,
        owner_function: FunctionScopeId,
        declaration: DeclarationRef,
    ) -> Result<Option<BindingCell>> {
        let Some(parent) = layout.parent_function(owner_function)? else {
            return Ok(None);
        };
        let Some(slot) = layout.upvalue_slot_for_declaration(parent, declaration)? else {
            return Ok(None);
        };
        let location = BindingLocation::upvalue(BindingSlot::from_index(slot.index()?));
        self.binding_at_location(location)
    }

    fn compiled_active_static_binding(
        &self,
        binding: StaticBindingId,
        atom: AtomId,
    ) -> Result<Option<BindingLocation>> {
        let operand = self.compiled_binding_operand(binding)?;
        if self.locals.last().is_some() {
            return Self::compiled_active_local_binding(atom, operand, self.locals.len());
        }
        match operand {
            BindingOperand::Global { slot } => Ok(Some(
                self.compiled_global_location(atom, BindingSlot::from_index(slot.index()?)),
            )),
            BindingOperand::Local { .. } | BindingOperand::Upvalue { .. } => {
                Err(Error::runtime("global binding layout is not a global slot"))
            }
            BindingOperand::Unresolved => Ok(None),
        }
    }

    fn compiled_active_local_binding(
        atom: AtomId,
        operand: BindingOperand,
        local_count: usize,
    ) -> Result<Option<BindingLocation>> {
        let BindingOperand::Local { slot, .. } = operand else {
            return Ok(None);
        };
        let Some(index) = local_count.checked_sub(1) else {
            return Ok(None);
        };
        Ok(Some(BindingLocation::local(
            atom,
            LocalScopeIndex::new(index),
            BindingSlot::from_index(slot.index()?),
        )))
    }

    fn compiled_global_location(&self, atom: AtomId, slot: BindingSlot) -> BindingLocation {
        if self.locals.last().is_none() && self.globals.cell_for_slot(atom, slot).is_some() {
            return BindingLocation::exact_global(slot);
        }
        BindingLocation::global(atom, slot)
    }

    fn cached_static_binding(&self, binding: &StaticBinding) -> Result<Option<BindingCell>> {
        let Some(cache) = self.current_static_binding_cache() else {
            return Ok(None);
        };
        let Some(location) = cache.location(binding)? else {
            return Ok(None);
        };
        self.binding_at_location(location)
    }

    fn remember_static_binding(
        &self,
        binding: &StaticBinding,
        location: BindingLocation,
    ) -> Result<()> {
        let Some(cache) = self.current_static_binding_cache() else {
            return Ok(());
        };
        self.remember_layout_static_bindings(&cache, binding, location)
    }

    fn remember_layout_static_bindings(
        &self,
        cache: &StaticBindingCacheHandle,
        binding: &StaticBinding,
        location: BindingLocation,
    ) -> Result<()> {
        self.remember_layout_static_binding_id(cache, binding.id(), location)
    }

    fn remember_layout_static_binding_id(
        &self,
        cache: &StaticBindingCacheHandle,
        binding: StaticBindingId,
        location: BindingLocation,
    ) -> Result<()> {
        let Some(layout) = self.current_static_binding_layout() else {
            return cache.remember_id(binding, location);
        };
        let Some(operand) = layout.operand_for_binding_id(binding)? else {
            return Ok(());
        };
        let location = location.for_compiled_operand(operand);
        layout.for_each_matching_operand_id(binding, |binding| cache.remember_id(binding, location))
    }

    fn resolve_binding_location(&self, atom: AtomId) -> Option<BindingLocation> {
        for (index, scope) in self.locals.iter().enumerate().rev() {
            if let Some(slot) = scope.slot_of(atom) {
                return Some(BindingLocation::local(
                    atom,
                    LocalScopeIndex::new(index),
                    slot,
                ));
            }
        }
        self.globals
            .slot_of(atom)
            .map(|slot| BindingLocation::global(atom, slot))
            .or_else(|| {
                self.builtin_globals
                    .slot_of(atom)
                    .map(|slot| BindingLocation::builtin_global(atom, slot))
            })
    }

    fn binding_at_location(&self, location: BindingLocation) -> Result<Option<BindingCell>> {
        match location {
            BindingLocation::Global { atom, slot, .. } => {
                Ok(self.global_binding_at_location(location, atom, slot))
            }
            BindingLocation::ExactGlobal { slot } => {
                Ok(self.exact_global_binding_at_location(slot))
            }
            BindingLocation::BuiltinGlobal { atom, slot, .. } => {
                Ok(self.builtin_global_binding_at_location(location, atom, slot))
            }
            BindingLocation::Local {
                atom, scope, slot, ..
            } => self.local_binding_at_location(location, atom, scope, slot),
            BindingLocation::ExactLocal {
                frame,
                compiled_scope,
                slot,
            } => Ok(self.exact_local_binding_at_location(frame, compiled_scope, slot)),
            BindingLocation::Upvalue { slot } => Ok(self.upvalue_binding_at_location(slot)),
        }
    }

    fn global_binding_at_location(
        &self,
        location: BindingLocation,
        atom: AtomId,
        slot: BindingSlot,
    ) -> Option<BindingCell> {
        if location.needs_shadow_guard() && self.scope_above_has_binding(0, atom) {
            return None;
        }
        self.globals.cell_for_slot(atom, slot)
    }

    fn exact_global_binding_at_location(&self, slot: BindingSlot) -> Option<BindingCell> {
        if self.locals.last().is_some() {
            return None;
        }
        self.globals.cell_at_slot(slot)
    }

    fn builtin_global_binding_at_location(
        &self,
        location: BindingLocation,
        atom: AtomId,
        slot: BindingSlot,
    ) -> Option<BindingCell> {
        if self.globals.contains(atom)
            || (location.needs_shadow_guard() && self.scope_above_has_binding(0, atom))
        {
            return None;
        }
        self.builtin_globals.cell_for_slot(atom, slot)
    }

    fn local_binding_at_location(
        &self,
        location: BindingLocation,
        atom: AtomId,
        index: LocalScopeIndex,
        slot: BindingSlot,
    ) -> Result<Option<BindingCell>> {
        let Some(scope) = self.locals.get(index.index()) else {
            return Ok(None);
        };
        let Some(binding) = scope.cell_for_slot(atom, slot) else {
            return Ok(None);
        };
        let start = index
            .index()
            .checked_add(1)
            .ok_or_else(|| Error::limit("local scope index overflowed"))?;
        if location.needs_shadow_guard() && self.scope_above_has_binding(start, atom) {
            return Ok(None);
        }
        Ok(Some(binding))
    }

    fn exact_local_binding_at_location(
        &self,
        index: LocalScopeIndex,
        compiled_scope: ScopeId,
        slot: BindingSlot,
    ) -> Option<BindingCell> {
        let scope = self.locals.get(index.index())?;
        if scope.compiled_scope() != Some(compiled_scope) {
            return None;
        }
        scope.cell_at_slot(slot)
    }

    fn upvalue_binding_at_location(&self, slot: BindingSlot) -> Option<BindingCell> {
        self.upvalue_frames
            .last()
            .and_then(|frame| frame.get(slot.index()))
            .cloned()
    }

    fn scope_above_has_binding(&self, start: usize, atom: AtomId) -> bool {
        for scope in self.locals.iter().skip(start) {
            if scope.contains(atom) {
                return true;
            }
        }
        false
    }
}
