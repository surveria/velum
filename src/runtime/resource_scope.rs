use crate::{
    error::{Error, Result},
    runtime::{
        Context, binding::scope::BindingScope, call::RuntimeCallArgs, control::Completion,
        native::DisposableStackFunctionKind, roots::VmRootKind,
    },
    value::Value,
};

impl Context {
    pub(in crate::runtime) fn register_using_resource(&mut self, value: Value) -> Result<()> {
        let existing = self
            .locals
            .last()
            .and_then(BindingScope::disposable_stack)
            .cloned();
        let stack = match existing {
            Some(ref stack) => stack.clone(),
            None => self.construct_disposable_stack()?,
        };
        let _root_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, [&stack, &value])?;
        self.eval_disposable_stack_function(
            DisposableStackFunctionKind::Use,
            RuntimeCallArgs::values(std::slice::from_ref(&value)),
            &stack,
        )?;
        if existing.is_none() {
            self.locals
                .last_mut()
                .ok_or_else(|| Error::runtime("using declaration has no lexical scope"))?
                .set_disposable_stack(stack)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn dispose_binding_scope(
        &mut self,
        mut scope: BindingScope,
        completion: Completion,
    ) -> Result<Completion> {
        let Some(stack) = scope.take_disposable_stack() else {
            return Ok(completion);
        };
        self.dispose_scope_stack(stack, completion)
    }

    pub(in crate::runtime) fn dispose_active_binding_scope(
        &mut self,
        completion: Completion,
    ) -> Result<Completion> {
        let stack = self
            .locals
            .last_mut()
            .and_then(BindingScope::take_disposable_stack);
        let Some(stack) = stack else {
            return Ok(completion);
        };
        self.dispose_scope_stack(stack, completion)
    }

    fn dispose_scope_stack(&mut self, stack: Value, completion: Completion) -> Result<Completion> {
        let _root_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&stack))?;
        self.dispose_disposable_stack_completion(&stack, completion)
    }
}
