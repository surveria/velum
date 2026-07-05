use std::rc::Rc;

use parking_lot::Mutex;

use crate::ast::DeclKind;
use crate::atom::AtomId;
use crate::error::{Error, Result};
use crate::value::Value;

#[derive(Debug, Clone, Default)]
pub struct BindingScope {
    slots: Vec<BindingCell>,
    bindings: Vec<BindingEntry>,
}

impl BindingScope {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            bindings: Vec::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.slots.len()
    }

    pub(crate) fn contains(&self, atom: AtomId) -> bool {
        self.binding_position(atom).is_ok()
    }

    pub(crate) fn get(&self, atom: AtomId) -> Option<BindingCell> {
        let position = self.binding_position(atom).ok()?;
        let entry = self.bindings.get(position)?;
        self.cell(entry.slot()).cloned()
    }

    pub(crate) fn insert(&mut self, atom: AtomId, binding: BindingCell) {
        self.insert_or_replace(atom, binding);
    }

    pub(crate) fn insert_or_replace(&mut self, atom: AtomId, binding: BindingCell) {
        match self.binding_position(atom) {
            Ok(position) => {
                let Some(entry) = self.bindings.get(position) else {
                    return;
                };
                if let Some(existing) = self.cell_mut(entry.slot()) {
                    *existing = binding;
                }
            }
            Err(position) => {
                let slot = BindingSlot::from_index(self.slots.len());
                self.slots.push(binding);
                self.bindings
                    .insert(position, BindingEntry::new(atom, slot));
            }
        }
    }

    pub(crate) fn retain_only(&mut self, atom: AtomId) {
        let Some(binding) = self.get(atom) else {
            self.slots.clear();
            self.bindings.clear();
            return;
        };
        self.slots.clear();
        self.bindings.clear();
        self.slots.push(binding);
        self.bindings
            .push(BindingEntry::new(atom, BindingSlot::zero()));
    }

    fn cell(&self, slot: BindingSlot) -> Option<&BindingCell> {
        self.slots.get(slot.index())
    }

    fn cell_mut(&mut self, slot: BindingSlot) -> Option<&mut BindingCell> {
        self.slots.get_mut(slot.index())
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
struct BindingSlot(usize);

impl BindingSlot {
    const fn from_index(index: usize) -> Self {
        Self(index)
    }

    const fn zero() -> Self {
        Self(0)
    }

    const fn index(self) -> usize {
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
