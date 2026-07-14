use crate::{
    error::{Error, Result},
    storage::atom::AtomId,
    value::Value,
};

use super::{BindingCell, BindingScope, BindingState};

impl BindingScope {
    pub(crate) fn contains(&self, atom: AtomId) -> bool {
        self.get(atom).is_some()
    }

    pub(crate) fn get(&self, atom: AtomId) -> Option<BindingCell> {
        let slot = self.slot_of(atom)?;
        self.cell(slot).filter(|cell| !cell.is_deleted()).cloned()
    }
}

impl BindingCell {
    pub(in crate::runtime) fn mark_deleted(&self) -> Result<()> {
        let mut binding = self.borrow_mut()?;
        if binding.is_terminal_alias_target {
            return Err(Error::runtime("terminal import binding cannot be deleted"));
        }
        binding.state = BindingState::Deleted;
        Ok(())
    }

    pub(in crate::runtime) fn restore_deleted(&self, value: Value) -> Result<()> {
        let mut binding = self.borrow_mut()?;
        if !matches!(binding.state, BindingState::Deleted) {
            return Err(Error::runtime("eval binding is not deleted"));
        }
        binding.state = BindingState::Initialized(value);
        Ok(())
    }

    pub(in crate::runtime) fn is_deleted(&self) -> bool {
        self.borrow()
            .is_ok_and(|binding| matches!(binding.state, BindingState::Deleted))
    }
}
