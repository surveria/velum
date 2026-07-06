use std::cell::Cell;
use std::rc::Rc;

use crate::{
    ast::{StaticBinding, StaticBindingId},
    atom::AtomId,
    binding_layout_types::BindingOperand,
    error::{Error, Result},
    runtime::Context,
    runtime_assertions::reference_error_undefined,
    runtime_scope::{BindingCell, BindingSlot},
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
struct BindingLocation {
    atom: AtomId,
    scope: BindingScopeLocation,
    slot: BindingSlot,
    validation: BindingLocationValidation,
}

impl BindingLocation {
    const fn global(atom: AtomId, slot: BindingSlot) -> Self {
        Self {
            atom,
            scope: BindingScopeLocation::Global,
            slot,
            validation: BindingLocationValidation::Guarded,
        }
    }

    const fn builtin_global(atom: AtomId, slot: BindingSlot) -> Self {
        Self {
            atom,
            scope: BindingScopeLocation::BuiltinGlobal,
            slot,
            validation: BindingLocationValidation::Guarded,
        }
    }

    const fn local(atom: AtomId, scope: LocalScopeIndex, slot: BindingSlot) -> Self {
        Self {
            atom,
            scope: BindingScopeLocation::Local(scope),
            slot,
            validation: BindingLocationValidation::Guarded,
        }
    }

    const fn exact(self) -> Self {
        Self {
            validation: BindingLocationValidation::Exact,
            ..self
        }
    }

    const fn needs_shadow_guard(self) -> bool {
        matches!(self.validation, BindingLocationValidation::Guarded)
    }
}

#[derive(Debug, Clone, Copy)]
enum BindingLocationValidation {
    Guarded,
    Exact,
}

#[derive(Debug, Clone, Copy)]
enum BindingScopeLocation {
    Global,
    BuiltinGlobal,
    Local(LocalScopeIndex),
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

impl Context {
    pub(crate) fn current_static_binding_cache(&self) -> Option<StaticBindingCacheHandle> {
        self.static_binding_caches.last().cloned()
    }

    pub(crate) fn current_static_binding_layout(
        &self,
    ) -> Option<crate::binding_layout::BindingLayout> {
        self.static_binding_layouts.last().cloned()
    }

    pub(crate) fn compiled_local_binding_slot(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<BindingSlot>> {
        match self.compiled_binding_operand(binding.id())? {
            BindingOperand::Local { slot, .. } => Ok(Some(BindingSlot::from_index(slot.index()?))),
            BindingOperand::Global { .. }
            | BindingOperand::Upvalue { .. }
            | BindingOperand::Unresolved => Ok(None),
        }
    }

    pub(crate) fn compiled_active_binding_slot(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<BindingSlot>> {
        if self.locals.last().is_some() {
            return self.compiled_local_binding_slot(binding);
        }
        match self.compiled_binding_operand(binding.id())? {
            BindingOperand::Global { slot } => Ok(Some(BindingSlot::from_index(slot.index()?))),
            BindingOperand::Local { .. } | BindingOperand::Upvalue { .. } => {
                Err(Error::runtime("global binding layout is not a global slot"))
            }
            BindingOperand::Unresolved => Ok(None),
        }
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
        match location.scope {
            BindingScopeLocation::Global => Ok(self.global_binding_at_location(location)),
            BindingScopeLocation::BuiltinGlobal => {
                Ok(self.builtin_global_binding_at_location(location))
            }
            BindingScopeLocation::Local(index) => self.local_binding_at_location(index, location),
        }
    }

    fn global_binding_at_location(&self, location: BindingLocation) -> Option<BindingCell> {
        if location.needs_shadow_guard() && self.scope_above_has_binding(0, location.atom) {
            return None;
        }
        self.globals.cell_for_slot(location.atom, location.slot)
    }

    fn builtin_global_binding_at_location(&self, location: BindingLocation) -> Option<BindingCell> {
        if self.globals.contains(location.atom)
            || (location.needs_shadow_guard() && self.scope_above_has_binding(0, location.atom))
        {
            return None;
        }
        self.builtin_globals
            .cell_for_slot(location.atom, location.slot)
    }

    fn local_binding_at_location(
        &self,
        index: LocalScopeIndex,
        location: BindingLocation,
    ) -> Result<Option<BindingCell>> {
        let Some(scope) = self.locals.get(index.index()) else {
            return Ok(None);
        };
        let Some(binding) = scope.cell_for_slot(location.atom, location.slot) else {
            return Ok(None);
        };
        let start = index
            .index()
            .checked_add(1)
            .ok_or_else(|| Error::limit("local scope index overflowed"))?;
        if location.needs_shadow_guard() && self.scope_above_has_binding(start, location.atom) {
            return Ok(None);
        }
        Ok(Some(binding))
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
        match (operand, self.scope) {
            (
                BindingOperand::Local { .. } | BindingOperand::Upvalue { .. },
                BindingScopeLocation::Local(_),
            ) => self.exact(),
            _ => self,
        }
    }
}
