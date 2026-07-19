#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;
use core::cell::RefCell;

use crate::error::Result;
use crate::storage::atom::AtomId;

use super::{BindingCell, BindingCellInner, BindingScope};

impl BindingScope {
    pub(in crate::runtime) fn for_each_active_binding(
        &self,
        mut visit: impl FnMut(AtomId, &BindingCell) -> Result<()>,
    ) -> Result<()> {
        for entry in self.index.bindings() {
            let Some(cell) = self.cell(entry.slot()).filter(|cell| !cell.is_deleted()) else {
                continue;
            };
            visit(entry.atom(), cell)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn fresh_iteration_copy(&self) -> Result<Self> {
        let mut slots = Vec::with_capacity(self.slots.len());
        for cell in &self.slots {
            slots.push(cell.detached_copy()?);
        }
        Ok(Self {
            slots,
            index: self.index.clone(),
            compiled_scope: self.compiled_scope,
            eval_var_conflict: self.eval_var_conflict,
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
