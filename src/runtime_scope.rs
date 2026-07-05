use std::{collections::BTreeMap, rc::Rc};

use parking_lot::Mutex;

use crate::ast::DeclKind;
use crate::atom::AtomId;
use crate::error::{Error, Result};
use crate::value::Value;

#[derive(Debug, Clone, Default)]
pub struct BindingScope {
    slots: Vec<BindingCell>,
    bindings: BTreeMap<AtomId, BindingSlot>,
}

impl BindingScope {
    pub const fn new() -> Self {
        Self {
            slots: Vec::new(),
            bindings: BTreeMap::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.slots.len()
    }

    pub(crate) fn contains(&self, atom: AtomId) -> bool {
        self.bindings.contains_key(&atom)
    }

    pub(crate) fn get(&self, atom: AtomId) -> Option<BindingCell> {
        let slot = self.bindings.get(&atom)?;
        self.cell(*slot).cloned()
    }

    pub(crate) fn insert(&mut self, atom: AtomId, binding: BindingCell) {
        self.insert_or_replace(atom, binding);
    }

    pub(crate) fn insert_or_replace(&mut self, atom: AtomId, binding: BindingCell) {
        if let Some(slot) = self.bindings.get(&atom).copied()
            && let Some(existing) = self.cell_mut(slot)
        {
            *existing = binding;
            return;
        }
        let slot = BindingSlot::from_index(self.slots.len());
        self.slots.push(binding);
        self.bindings.insert(atom, slot);
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
        self.bindings.insert(atom, BindingSlot::zero());
    }

    fn cell(&self, slot: BindingSlot) -> Option<&BindingCell> {
        self.slots.get(slot.index())
    }

    fn cell_mut(&mut self, slot: BindingSlot) -> Option<&mut BindingCell> {
        self.slots.get_mut(slot.index())
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
