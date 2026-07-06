use std::rc::Rc;

use parking_lot::Mutex;

use crate::ast::DeclKind;
use crate::atom::AtomId;
use crate::binding_layout_types::ScopeId;
use crate::error::{Error, Result};
use crate::value::Value;

#[derive(Debug, Clone, Default)]
pub struct BindingScope {
    slots: Vec<BindingCell>,
    slot_atoms: Vec<AtomId>,
    bindings: Vec<BindingEntry>,
    compiled_scope: Option<ScopeId>,
}

impl BindingScope {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            slot_atoms: Vec::new(),
            bindings: Vec::new(),
            compiled_scope: None,
        }
    }

    pub const fn len(&self) -> usize {
        self.slots.len()
    }

    pub(crate) fn contains(&self, atom: AtomId) -> bool {
        self.binding_position(atom).is_ok()
    }

    pub(crate) fn get(&self, atom: AtomId) -> Option<BindingCell> {
        let slot = self.slot_of(atom)?;
        self.cell(slot).cloned()
    }

    pub(crate) const fn compiled_scope(&self) -> Option<ScopeId> {
        self.compiled_scope
    }

    pub(crate) fn mark_compiled_scope(&mut self, scope: ScopeId) -> Result<()> {
        if let Some(existing) = self.compiled_scope {
            if existing != scope {
                return Err(Error::runtime("binding frame layout scope mismatch"));
            }
            return Ok(());
        }
        self.compiled_scope = Some(scope);
        Ok(())
    }

    pub(crate) fn slot_of(&self, atom: AtomId) -> Option<BindingSlot> {
        let position = self.binding_position(atom).ok()?;
        self.bindings.get(position).map(|entry| entry.slot())
    }

    pub(crate) fn cell_for_slot(&self, atom: AtomId, slot: BindingSlot) -> Option<BindingCell> {
        let slot_atom = self.slot_atoms.get(slot.index()).copied()?;
        if slot_atom != atom {
            return None;
        }
        self.cell(slot).cloned()
    }

    pub(crate) fn cell_at_slot(&self, slot: BindingSlot) -> Option<BindingCell> {
        self.cell(slot).cloned()
    }

    pub(crate) fn insert(&mut self, atom: AtomId, binding: BindingCell) -> BindingSlot {
        self.insert_or_replace(atom, binding)
    }

    pub(crate) fn insert_or_replace(&mut self, atom: AtomId, binding: BindingCell) -> BindingSlot {
        match self.binding_position(atom) {
            Ok(position) => {
                let Some(entry) = self.bindings.get(position) else {
                    return BindingSlot::from_index(self.slots.len());
                };
                let slot = entry.slot();
                if let Some(existing) = self.cell_mut(slot) {
                    *existing = binding;
                }
                slot
            }
            Err(position) => {
                let slot = BindingSlot::from_index(self.slots.len());
                self.slots.push(binding);
                self.slot_atoms.push(atom);
                self.bindings
                    .insert(position, BindingEntry::new(atom, slot));
                slot
            }
        }
    }

    pub(crate) fn insert_or_replace_at_slot(
        &mut self,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Result<BindingSlot> {
        match self.binding_position(atom) {
            Ok(position) => {
                let Some(entry) = self.bindings.get(position) else {
                    return Err(Error::runtime("binding frame index disappeared"));
                };
                if entry.slot() != slot {
                    return Err(Error::runtime("binding frame slot mismatch"));
                }
                let Some(existing) = self.cell_mut(slot) else {
                    return Err(Error::runtime("binding frame slot is not defined"));
                };
                *existing = binding;
                Ok(slot)
            }
            Err(position) => self.insert_new_at_slot(position, atom, binding, slot),
        }
    }

    pub(crate) fn insert_or_replace_at_optional_slot(
        &mut self,
        atom: AtomId,
        binding: BindingCell,
        slot: Option<BindingSlot>,
    ) -> Result<BindingSlot> {
        if let Some(slot) = slot
            && let Some(inserted) =
                self.try_insert_or_replace_at_slot(atom, binding.clone(), slot)?
        {
            return Ok(inserted);
        }
        Ok(self.insert_or_replace(atom, binding))
    }

    pub(crate) fn retain_only(&mut self, atom: AtomId) {
        let Some(binding) = self.get(atom) else {
            self.slots.clear();
            self.slot_atoms.clear();
            self.bindings.clear();
            return;
        };
        self.slots.clear();
        self.slot_atoms.clear();
        self.bindings.clear();
        self.slots.push(binding);
        self.slot_atoms.push(atom);
        self.bindings
            .push(BindingEntry::new(atom, BindingSlot::zero()));
    }

    fn cell(&self, slot: BindingSlot) -> Option<&BindingCell> {
        self.slots.get(slot.index())
    }

    fn cell_mut(&mut self, slot: BindingSlot) -> Option<&mut BindingCell> {
        self.slots.get_mut(slot.index())
    }

    fn insert_new_at_slot(
        &mut self,
        position: usize,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Result<BindingSlot> {
        let slot_index = slot.index();
        if slot_index < self.slots.len() {
            return Err(Error::runtime("binding frame slot is already occupied"));
        }
        if slot_index > self.slots.len() {
            return Err(Error::runtime("binding frame slot gap is not supported"));
        }
        self.slots.push(binding);
        self.slot_atoms.push(atom);
        self.bindings
            .insert(position, BindingEntry::new(atom, slot));
        Ok(slot)
    }

    fn try_insert_or_replace_at_slot(
        &mut self,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Result<Option<BindingSlot>> {
        match self.binding_position(atom) {
            Ok(position) => {
                let Some(entry) = self.bindings.get(position) else {
                    return Err(Error::runtime("binding frame index disappeared"));
                };
                if entry.slot() != slot {
                    return Ok(None);
                }
                let Some(existing) = self.cell_mut(slot) else {
                    return Err(Error::runtime("binding frame slot is not defined"));
                };
                *existing = binding;
                Ok(Some(slot))
            }
            Err(position) => Ok(self.try_insert_new_at_slot(position, atom, binding, slot)),
        }
    }

    fn try_insert_new_at_slot(
        &mut self,
        position: usize,
        atom: AtomId,
        binding: BindingCell,
        slot: BindingSlot,
    ) -> Option<BindingSlot> {
        if slot.index() != self.slots.len() {
            return None;
        }
        self.slots.push(binding);
        self.slot_atoms.push(atom);
        self.bindings
            .insert(position, BindingEntry::new(atom, slot));
        Some(slot)
    }

    fn binding_position(&self, atom: AtomId) -> std::result::Result<usize, usize> {
        self.bindings
            .binary_search_by(|entry| entry.atom().cmp(&atom))
    }
}

#[derive(Debug, Clone, Copy)]
struct BindingEntry {
    atom: AtomId,
    slot: BindingSlot,
}

impl BindingEntry {
    const fn new(atom: AtomId, slot: BindingSlot) -> Self {
        Self { atom, slot }
    }

    const fn atom(self) -> AtomId {
        self.atom
    }

    const fn slot(self) -> BindingSlot {
        self.slot
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct BindingSlot(usize);

impl BindingSlot {
    pub(crate) const fn from_index(index: usize) -> Self {
        Self(index)
    }

    const fn zero() -> Self {
        Self(0)
    }

    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct BindingCell(Rc<Mutex<Binding>>);

impl BindingCell {
    pub fn new(value: Value, mutable: bool, kind: DeclKind) -> Self {
        Self(Rc::new(Mutex::new(Binding {
            value,
            mutable,
            kind,
        })))
    }

    pub fn value(&self) -> Value {
        self.0.lock().value.clone()
    }

    pub fn kind(&self) -> DeclKind {
        self.0.lock().kind
    }

    pub fn assign(&self, name: &str, value: Value) -> Result<()> {
        let mut binding = self.0.lock();
        if !binding.mutable {
            return Err(Error::runtime(format!("assignment to constant '{name}'")));
        }
        binding.value = value;
        drop(binding);
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Binding {
    value: Value,
    mutable: bool,
    kind: DeclKind,
}
