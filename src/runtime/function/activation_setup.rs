use alloc::rc::Rc;

use crate::{
    bytecode::BytecodeFunction,
    error::Result,
    runtime::{
        Context,
        activation::DynamicEnvironment,
        control::{Completion, TailCallReturnMode},
    },
    value::{FunctionId, Value},
};

use super::{FunctionSelfBinding, FunctionSuperBinding};

impl Context {
    pub(super) fn append_direct_eval_environment(
        &mut self,
        bytecode: &BytecodeFunction,
        environments: &mut Vec<DynamicEnvironment>,
    ) -> Result<()> {
        if bytecode.contains_direct_eval() && !bytecode.strict() {
            environments.push(DynamicEnvironment::EvalBindings(
                self.create_eval_binding_environment()?,
            ));
        }
        Ok(())
    }

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

    pub(super) fn finish_function_call(
        &mut self,
        id: FunctionId,
        local_base: usize,
        has_arguments_binding: bool,
        has_self_binding: bool,
        derived_super_binding: Option<&Rc<FunctionSuperBinding>>,
        mut result: Result<Completion>,
    ) -> Result<(Completion, TailCallReturnMode)> {
        if let Ok(completion) = result {
            result = self.dispose_active_binding_scope(completion);
        }
        let binding_result =
            self.pop_function_binding_storage(local_base, has_arguments_binding, has_self_binding);
        let activation_result = self.pop_call_activation(local_base);
        binding_result?;
        activation_result?;
        let return_mode = self.function_return_mode(id, derived_super_binding)?;
        result.map(|completion| (completion, return_mode))
    }
}
