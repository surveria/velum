use std::{cell::RefCell, rc::Rc};

use crate::error::Result;

use super::{BindingCell, BindingCellInner, BindingScope};

impl BindingScope {
    pub(in crate::runtime) fn fresh_iteration_copy(&self) -> Result<Self> {
        let mut slots = Vec::with_capacity(self.slots.len());
        for cell in &self.slots {
            slots.push(cell.detached_copy()?);
        }
        Ok(Self {
            slots,
            index: self.index.clone(),
            compiled_scope: self.compiled_scope,
            storage_ledger: None,
            resource_stacks: Vec::new(),
        })
    }
}

impl BindingCell {
    fn detached_copy(&self) -> Result<Self> {
        let binding = self.borrow()?.clone();
        Ok(Self(Rc::new(BindingCellInner {
            binding: RefCell::new(binding),
            kind: self.kind(),
        })))
    }
}
