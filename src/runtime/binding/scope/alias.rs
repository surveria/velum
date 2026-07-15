use crate::error::{Error, Result};

use super::{BindingCell, BindingState, ImmutableAssignment};

impl BindingCell {
    pub(in crate::runtime) fn terminal_alias_target(&self) -> Result<Self> {
        let binding = self.borrow()?;
        match &binding.state {
            BindingState::Alias(target) => Ok(target.clone()),
            BindingState::Initialized(_) | BindingState::Uninitialized | BindingState::Deleted => {
                Ok(self.clone())
            }
        }
    }

    pub(in crate::runtime) fn redirect_to_terminal(&self, target: Self) -> Result<()> {
        if self.same_cell(&target) {
            return Err(Error::runtime("binding cannot redirect to itself"));
        }
        let mut binding = self.borrow_mut()?;
        if binding.is_terminal_alias_target {
            return Err(Error::runtime("terminal binding cannot be redirected"));
        }
        if matches!(
            binding.state,
            BindingState::Alias(_) | BindingState::Deleted
        ) {
            return Err(Error::runtime(
                "binding cannot be redirected in its current state",
            ));
        }
        let mut target_binding = target.borrow_mut()?;
        if matches!(target_binding.state, BindingState::Alias(_)) {
            return Err(Error::runtime("binding redirect target is not terminal"));
        }
        target_binding.is_terminal_alias_target = true;
        drop(target_binding);
        binding.state = BindingState::Alias(target);
        binding.mutable = false;
        binding.immutable_assignment = ImmutableAssignment::AlwaysThrow;
        drop(binding);
        Ok(())
    }
}
