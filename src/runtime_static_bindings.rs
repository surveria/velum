use std::cell::Cell;
use std::rc::Rc;

use crate::{
    ast::{StaticBinding, StaticBindingId},
    atom::AtomId,
    binding_layout::BindingLayout,
    binding_layout_types::{BindingOperand, DeclarationRef, FunctionScopeId, ScopeId},
    error::{Error, Result},
    runtime::Context,
    runtime_assertions::reference_error_undefined,
    runtime_scope::{BindingCell, BindingScope, BindingSlot},
    value::Value,
};

#[derive(Debug, Clone)]
pub struct StaticBindingCacheHandle(Rc<[Cell<Option<BindingLocation>>]>);

impl StaticBindingCacheHandle {
    pub(super) fn new(slot_count: usize) -> Self {
        let mut bindings = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            bindings.push(Cell::new(None));
        }
        Self(Rc::from(bindings.into_boxed_slice()))
    }

    fn location(&self, binding: &StaticBinding) -> Result<Option<BindingLocation>> {
        self.0
            .get(binding.id().index()?)
            .map(Cell::get)
            .ok_or_else(|| Error::runtime("static binding cache slot is not defined"))
    }

    fn remember_id(&self, binding: StaticBindingId, location: BindingLocation) -> Result<()> {
        let slot = self
            .0
            .get(binding.index()?)
            .ok_or_else(|| Error::runtime("static binding cache slot is not defined"))?;
        slot.set(Some(location));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
enum BindingLocation {
    Global {
        atom: AtomId,
        slot: BindingSlot,
        validation: BindingLocationValidation,
    },
    BuiltinGlobal {
        atom: AtomId,
        slot: BindingSlot,
        validation: BindingLocationValidation,
    },
    Local {
        atom: AtomId,
        scope: LocalScopeIndex,
        slot: BindingSlot,
        validation: BindingLocationValidation,
    },
    ExactLocal {
        frame: LocalScopeIndex,
        compiled_scope: ScopeId,
        slot: BindingSlot,
    },
    Upvalue {
        slot: BindingSlot,
    },
}

impl BindingLocation {
    const fn global(atom: AtomId, slot: BindingSlot) -> Self {
        Self::Global {
            atom,
            slot,
            validation: BindingLocationValidation::Guarded,
        }
    }

    const fn builtin_global(atom: AtomId, slot: BindingSlot) -> Self {
        Self::BuiltinGlobal {
            atom,
            slot,
            validation: BindingLocationValidation::Guarded,
        }
    }

    const fn local(atom: AtomId, scope: LocalScopeIndex, slot: BindingSlot) -> Self {
        Self::Local {
            atom,
            scope,
            slot,
            validation: BindingLocationValidation::Guarded,
        }
    }

    const fn upvalue(slot: BindingSlot) -> Self {
        Self::Upvalue { slot }
    }

    const fn exact(self) -> Self {
        match self {
            Self::Global { atom, slot, .. } => Self::Global {
                atom,
                slot,
                validation: BindingLocationValidation::Exact,
            },
            Self::BuiltinGlobal { atom, slot, .. } => Self::BuiltinGlobal {
                atom,
                slot,
                validation: BindingLocationValidation::Exact,
            },
            Self::Local { .. } | Self::ExactLocal { .. } => self,
            Self::Upvalue { slot } => Self::Upvalue { slot },
        }
    }

    const fn needs_shadow_guard(self) -> bool {
        matches!(
            self,
            Self::Global {
                validation: BindingLocationValidation::Guarded,
                ..
            } | Self::BuiltinGlobal {
                validation: BindingLocationValidation::Guarded,
                ..
            } | Self::Local {
                validation: BindingLocationValidation::Guarded,
                ..
            }
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum BindingLocationValidation {
    Guarded,
    Exact,
}

#[derive(Debug, Clone, Copy)]
struct LocalScopeIndex(usize);

impl LocalScopeIndex {
    const fn new(index: usize) -> Self {
        Self(index)
    }

    const fn index(self) -> usize {
        self.0
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
        let Some(atom) = self.lookup_static_name_atom(binding.name())? else {
            return Ok(None);
        };
        if let Some(cell) = self.cached_static_binding(binding)? {
            return Ok(Some(cell));
        }
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

    pub(crate) fn assign_static(&self, binding: &StaticBinding, value: Value) -> Result<()> {
        self.checked_value(value.clone())?;
        let Some(cell) = self.get_binding_static(binding)? else {
            return Err(reference_error_undefined(binding));
        };
        cell.assign(binding, value)
    }

    pub(crate) fn assign_static_or_builtin(
        &mut self,
        binding: &StaticBinding,
        value: Value,
    ) -> Result<()> {
        self.checked_value(value.clone())?;
        let Some(cell) = self.get_or_materialize_binding_static(binding)? else {
            return Err(reference_error_undefined(binding));
        };
        cell.assign(binding, value)
    }

    pub(crate) fn get_or_materialize_binding_static(
        &mut self,
        binding: &StaticBinding,
    ) -> Result<Option<BindingCell>> {
        if let Some(cell) = self.get_binding_static(binding)? {
            return Ok(Some(cell));
        }
        if self.builtin_value(binding.name())?.is_none() {
            return Ok(None);
        }
        self.get_binding_static(binding)
    }

    pub(crate) fn binding_exists_or_materialize_static(
        &mut self,
        binding: &StaticBinding,
    ) -> Result<bool> {
        self.get_or_materialize_binding_static(binding)
            .map(|binding| binding.is_some())
    }

    pub(crate) fn resolve_runtime_static_binding(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<BindingCell>> {
        let Some(atom) = self.lookup_static_name_atom(binding.name())? else {
            return Ok(None);
        };
        let Some(location) = self.resolve_binding_location(atom) else {
            return Ok(None);
        };
        self.binding_at_location(location)
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
        self.resolve_runtime_static_binding(binding)
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

    fn compiled_global_static_binding(
        &self,
        binding: &StaticBinding,
        atom: AtomId,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        let BindingOperand::Global { slot } = self.compiled_binding_operand(binding.id())? else {
            return Ok(None);
        };
        let location = BindingLocation::global(atom, BindingSlot::from_index(slot.index()?));
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
                    BindingLocation::global(atom, BindingSlot::from_index(slot.index()?)).exact();
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
        slot: crate::binding_layout_types::LocalSlot,
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
            BindingOperand::Global { slot } => Ok(Some(BindingLocation::global(
                atom,
                BindingSlot::from_index(slot.index()?),
            ))),
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
            .and_then(Clone::clone)
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

impl BindingLocation {
    const fn for_compiled_operand(self, operand: BindingOperand) -> Self {
        match (operand, self) {
            (
                BindingOperand::Local {
                    scope: compiled_scope,
                    ..
                },
                Self::Local {
                    scope: frame, slot, ..
                },
            ) => Self::ExactLocal {
                frame,
                compiled_scope,
                slot,
            },
            (BindingOperand::Local { .. }, Self::ExactLocal { .. }) => self,
            (BindingOperand::Upvalue { .. }, Self::Upvalue { slot }) => Self::Upvalue { slot },
            (_, location) => location,
        }
    }
}
