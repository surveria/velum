use core::cell::Ref;

use crate::{
    error::{Error, Result},
    runtime::control::{reference_error_undefined, reference_error_uninitialized},
    value::Value,
};

use super::{Binding, BindingCell, BindingState};

impl BindingCell {
    /// Keeps the initialized read available to callers without expanding alias
    /// resolution and error construction at every bytecode binding access.
    #[inline]
    pub fn value(&self, name: &str) -> Result<Value> {
        let binding = self.borrow()?;
        if let BindingState::Initialized(value) = &binding.state {
            return Ok(value.clone());
        }
        Self::read_indirect_or_uninitialized(binding, name)
    }

    #[inline(never)]
    fn read_indirect_or_uninitialized(binding: Ref<'_, Binding>, name: &str) -> Result<Value> {
        let target = match &binding.state {
            BindingState::Initialized(value) => return Ok(value.clone()),
            BindingState::Uninitialized => return Err(reference_error_uninitialized(name)),
            BindingState::Deleted => return Err(reference_error_undefined(name)),
            BindingState::Alias(target) => target.clone(),
        };
        // Preserve the borrow boundary before following a live import target.
        drop(binding);
        let target_binding = target.borrow()?;
        match &target_binding.state {
            BindingState::Initialized(value) => Ok(value.clone()),
            BindingState::Uninitialized => Err(reference_error_uninitialized(name)),
            BindingState::Deleted => Err(reference_error_undefined(name)),
            BindingState::Alias(_) => Err(Error::runtime(
                "import binding alias target is not terminal",
            )),
        }
    }

    pub(crate) fn with_initialized_value<R>(&self, visit: impl FnOnce(&Value) -> R) -> Option<R> {
        let value = self.value("<binding>").ok()?;
        Some(visit(&value))
    }
}
