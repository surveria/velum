use std::{collections::BTreeMap, rc::Rc};

use parking_lot::Mutex;

use crate::ast::DeclKind;
use crate::error::{Error, Result};
use crate::value::Value;

#[derive(Debug, Clone, Default)]
pub struct BindingScope {
    bindings: BTreeMap<String, BindingCell>,
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

    pub fn contains(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<BindingCell> {
        self.bindings.get(name).cloned()
    }

    pub fn insert(&mut self, name: String, binding: BindingCell) -> Option<BindingCell> {
        self.bindings.insert(name, binding)
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
