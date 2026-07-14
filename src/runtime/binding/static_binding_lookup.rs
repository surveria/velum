use crate::{
    binding_metadata::{BindingLayout, BindingOperand, DeclarationRef, FunctionScopeId, ScopeId},
    error::{Error, Result},
    runtime::{
        Context,
        binding::{
            location::{BindingLocation, LocalScopeIndex},
            scope::{BindingCell, BindingSlot},
            static_bindings::StaticBindingCacheHandle,
        },
    },
    storage::atom::AtomId,
    syntax::{StaticBinding, StaticBindingId},
};

impl Context {
    pub(in crate::runtime::binding) fn compiled_binding_operand(
        &self,
        binding: StaticBindingId,
    ) -> Result<BindingOperand> {
        let Some(layout) = self.current_static_binding_layout() else {
            return Ok(BindingOperand::Unresolved);
        };
        Ok(layout
            .operand_for_binding_id(binding)?
            .unwrap_or(BindingOperand::Unresolved))
    }

    pub(in crate::runtime::binding) fn direct_compiled_static_binding(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        self.direct_bytecode_static_binding(self.compiled_binding_operand(binding.id())?)
    }

    pub(in crate::runtime::binding) fn direct_bytecode_static_binding(
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
            BindingOperand::Global { .. }
            | BindingOperand::EvalVariable { .. }
            | BindingOperand::Unresolved => Ok(None),
        }
    }

    pub(in crate::runtime::binding) fn compiled_global_static_binding(
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

    pub(in crate::runtime::binding) fn compiled_local_static_binding(
        &self,
        binding: &StaticBinding,
        atom: AtomId,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        let BindingOperand::Local { scope, slot } = self.compiled_binding_operand(binding.id())?
        else {
            return Ok(None);
        };
        let slot = BindingSlot::from_index(slot.index()?);
        for (index, frame) in self
            .locals
            .iter()
            .enumerate()
            .skip(self.current_local_frame_start())
            .rev()
        {
            if frame.compiled_scope() != Some(scope) {
                continue;
            }
            let location =
                BindingLocation::local(atom, self.relative_local_scope_index(index)?, slot);
            let Some(cell) = self.binding_at_location(location)? else {
                return Ok(None);
            };
            return Ok(Some((location, cell)));
        }
        Ok(None)
    }

    pub(in crate::runtime::binding) fn compiled_upvalue_static_binding(
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

    pub(in crate::runtime::binding) fn compiled_declaration_static_binding(
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
            BindingOperand::EvalVariable { .. }
            | BindingOperand::Upvalue { .. }
            | BindingOperand::Unresolved => Ok(None),
        }
    }

    pub(in crate::runtime::binding) fn compiled_parent_upvalue_static_binding(
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

    pub(in crate::runtime::binding) fn compiled_active_static_binding(
        &self,
        binding: StaticBindingId,
        atom: AtomId,
    ) -> Result<Option<BindingLocation>> {
        let operand = self.compiled_binding_operand(binding)?;
        if self.has_visible_local_scope() {
            return self.compiled_active_local_binding(atom, operand);
        }
        match operand {
            BindingOperand::Global { slot } => Ok(Some(
                self.compiled_global_location(atom, BindingSlot::from_index(slot.index()?)),
            )),
            BindingOperand::Local { .. } | BindingOperand::Upvalue { .. } => {
                Err(Error::runtime("global binding layout is not a global slot"))
            }
            BindingOperand::EvalVariable { .. } | BindingOperand::Unresolved => Ok(None),
        }
    }

    pub(in crate::runtime::binding) fn cached_static_binding(
        &self,
        binding: &StaticBinding,
    ) -> Result<Option<BindingCell>> {
        let Some(cache) = self.current_static_binding_cache() else {
            return Ok(None);
        };
        let Some(location) = cache.location(binding)? else {
            return Ok(None);
        };
        self.binding_at_location(location)
    }

    pub(in crate::runtime::binding) fn remember_static_binding(
        &self,
        binding: &StaticBinding,
        location: BindingLocation,
    ) -> Result<()> {
        let Some(cache) = self.current_static_binding_cache() else {
            return Ok(());
        };
        self.remember_layout_static_bindings(&cache, binding, location)
    }

    pub(in crate::runtime::binding) fn remember_layout_static_binding_id(
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

    pub(in crate::runtime::binding) fn resolve_binding_location(
        &self,
        atom: AtomId,
    ) -> Option<BindingLocation> {
        for (index, scope) in self
            .locals
            .iter()
            .enumerate()
            .skip(self.current_local_frame_start())
            .rev()
        {
            if let Some(slot) = scope.slot_of(atom) {
                return Some(BindingLocation::local(
                    atom,
                    self.relative_local_scope_index(index).ok()?,
                    slot,
                ));
            }
        }
        if self.realm.object_global_names.contains(&atom) {
            return None;
        }
        self.realm
            .globals
            .slot_of(atom)
            .map(|slot| BindingLocation::global(atom, slot))
            .or_else(|| {
                self.realm
                    .builtin_globals
                    .slot_of(atom)
                    .map(|slot| BindingLocation::builtin_global(atom, slot))
            })
    }

    pub(in crate::runtime::binding) fn unresolved_binding_name_is_shadowed(
        &self,
        name: &str,
    ) -> bool {
        let Some(atom) = self.atom(name) else {
            return false;
        };
        self.realm.object_global_names.contains(&atom)
            || self.realm.globals.contains(atom)
            || self.scope_above_has_binding(0, atom)
    }

    fn direct_compiled_local_static_binding(
        &self,
        scope: ScopeId,
        slot: crate::binding_metadata::LocalSlot,
    ) -> Result<Option<(BindingLocation, BindingCell)>> {
        let slot = BindingSlot::from_index(slot.index()?);
        for (index, frame) in self
            .locals
            .iter()
            .enumerate()
            .skip(self.current_local_frame_start())
            .rev()
        {
            if frame.compiled_scope() != Some(scope) {
                continue;
            }
            let location = BindingLocation::ExactLocal {
                frame: self.relative_local_scope_index(index)?,
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

    fn compiled_declaration_local_binding(
        &self,
        scope: ScopeId,
        slot: crate::binding_metadata::LocalSlot,
    ) -> Result<Option<BindingCell>> {
        let slot = BindingSlot::from_index(slot.index()?);
        for (index, frame) in self
            .locals
            .iter()
            .enumerate()
            .skip(self.current_local_frame_start())
            .rev()
        {
            if frame.compiled_scope() != Some(scope) {
                continue;
            }
            let location = BindingLocation::ExactLocal {
                frame: self.relative_local_scope_index(index)?,
                compiled_scope: scope,
                slot,
            };
            return self.binding_at_location(location);
        }
        Ok(None)
    }

    fn compiled_active_local_binding(
        &self,
        atom: AtomId,
        operand: BindingOperand,
    ) -> Result<Option<BindingLocation>> {
        let BindingOperand::Local { slot, .. } = operand else {
            return Ok(None);
        };
        let Some(index) = self.locals.len().checked_sub(1) else {
            return Ok(None);
        };
        Ok(Some(BindingLocation::local(
            atom,
            self.relative_local_scope_index(index)?,
            BindingSlot::from_index(slot.index()?),
        )))
    }

    fn compiled_global_location(&self, atom: AtomId, slot: BindingSlot) -> BindingLocation {
        if self.realm.globals.cell_for_slot(atom, slot).is_some() {
            return BindingLocation::exact_global(atom, slot);
        }
        BindingLocation::global(atom, slot)
    }

    fn remember_layout_static_bindings(
        &self,
        cache: &StaticBindingCacheHandle,
        binding: &StaticBinding,
        location: BindingLocation,
    ) -> Result<()> {
        self.remember_layout_static_binding_id(cache, binding.id(), location)
    }

    pub(in crate::runtime::binding) fn binding_at_location(
        &self,
        location: BindingLocation,
    ) -> Result<Option<BindingCell>> {
        match location {
            BindingLocation::Global { atom, slot, .. } => {
                Ok(self.global_binding_at_location(location, atom, slot))
            }
            BindingLocation::ExactGlobal { atom, slot } => {
                Ok(self.exact_global_binding_at_location(atom, slot))
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
        if self.realm.object_global_names.contains(&atom)
            || (location.needs_shadow_guard() && self.scope_above_has_binding(0, atom))
        {
            return None;
        }
        self.realm.globals.cell_for_slot(atom, slot)
    }

    fn exact_global_binding_at_location(
        &self,
        atom: AtomId,
        slot: BindingSlot,
    ) -> Option<BindingCell> {
        if self.realm.object_global_names.contains(&atom) {
            return None;
        }
        self.realm.globals.cell_at_slot(slot)
    }

    fn builtin_global_binding_at_location(
        &self,
        location: BindingLocation,
        atom: AtomId,
        slot: BindingSlot,
    ) -> Option<BindingCell> {
        if self.realm.object_global_names.contains(&atom)
            || self.realm.globals.contains(atom)
            || (location.needs_shadow_guard() && self.scope_above_has_binding(0, atom))
        {
            return None;
        }
        self.realm.builtin_globals.cell_for_slot(atom, slot)
    }

    fn local_binding_at_location(
        &self,
        location: BindingLocation,
        atom: AtomId,
        index: LocalScopeIndex,
        slot: BindingSlot,
    ) -> Result<Option<BindingCell>> {
        let absolute_index = self.absolute_local_scope_index(index)?;
        let Some(scope) = self.locals.get(absolute_index) else {
            return Ok(None);
        };
        let Some(binding) = scope.cell_for_slot(atom, slot) else {
            return Ok(None);
        };
        let start = absolute_index
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
        let absolute_index = self.absolute_local_scope_index(index).ok()?;
        let scope = self.locals.get(absolute_index)?;
        if scope.compiled_scope() != Some(compiled_scope) {
            return None;
        }
        scope.cell_at_slot(slot)
    }

    fn upvalue_binding_at_location(&self, slot: BindingSlot) -> Option<BindingCell> {
        self.current_activation_upvalues()
            .and_then(|frame| frame.get(slot.index()))
            .cloned()
    }

    fn scope_above_has_binding(&self, start: usize, atom: AtomId) -> bool {
        let start = start.max(self.current_local_frame_start());
        for scope in self.locals.iter().skip(start) {
            if scope.contains(atom) {
                return true;
            }
        }
        false
    }

    fn relative_local_scope_index(&self, absolute_index: usize) -> Result<LocalScopeIndex> {
        let relative = absolute_index
            .checked_sub(self.current_local_frame_start())
            .ok_or_else(|| Error::runtime("local scope index is outside the active frame"))?;
        Ok(LocalScopeIndex::new(relative))
    }

    fn absolute_local_scope_index(&self, relative_index: LocalScopeIndex) -> Result<usize> {
        self.current_local_frame_start()
            .checked_add(relative_index.index())
            .ok_or_else(|| Error::limit("local scope index overflowed"))
    }
}
