use std::{collections::BTreeMap, rc::Rc};

use parking_lot::Mutex;

use crate::ast::DeclKind;
use crate::atom::AtomId;
use crate::error::{Error, Result};
use crate::value::Value;

#[derive(Debug, Clone, Default)]
pub struct BindingScope {
    bindings: BTreeMap<AtomId, BindingCell>,
}

impl BindingScope {
    pub const fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub(crate) fn contains(&self, atom: AtomId) -> bool {
        self.bindings.contains_key(&atom)
    }

    pub(crate) fn get(&self, atom: AtomId) -> Option<BindingCell> {
        self.bindings.get(&atom).cloned()
    }

    pub(crate) fn insert(&mut self, atom: AtomId, binding: BindingCell) {
        self.insert_or_replace(atom, binding);
    }

    pub(crate) fn insert_or_replace(&mut self, atom: AtomId, binding: BindingCell) {
        if let Some(existing) = self.bindings.get_mut(&atom) {
            *existing = binding;
            return;
        }
        self.bindings.insert(atom, binding);
    }

    pub(crate) fn retain_only(&mut self, atom: AtomId) {
        self.bindings
            .retain(|binding_atom, _| *binding_atom == atom);
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
