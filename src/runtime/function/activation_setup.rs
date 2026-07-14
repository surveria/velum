use crate::{
    error::Result,
    runtime::Context,
    value::{FunctionId, Value},
};

use super::FunctionSelfBinding;

impl Context {
    pub(super) fn initialize_base_fields_at_activation(
        &mut self,
        id: FunctionId,
        receiver: Option<&Value>,
        local_base: usize,
    ) -> Result<()> {
        let Some(receiver) = receiver else {
            return Ok(());
        };
        if let Err(error) = self.initialize_class_fields(id, receiver) {
            self.pop_call_activation(local_base)?;
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn push_optional_function_self_scope(
        &mut self,
        id: FunctionId,
        binding: Option<FunctionSelfBinding>,
        local_base: usize,
    ) -> Result<()> {
        let Some(binding) = binding else {
            return Ok(());
        };
        let self_scope = match self.named_function_self_scope(id, binding) {
            Ok(scope) => scope,
            Err(error) => {
                self.pop_call_activation(local_base)?;
                return Err(error);
            }
        };
        if let Err(error) = self.push_lexical_scope_with(self_scope) {
            self.leave_function_local_frame(local_base)?;
            self.pop_call_activation(local_base)?;
            return Err(error);
        }
        Ok(())
    }
}
