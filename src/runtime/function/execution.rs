use crate::{
    bytecode::BytecodeNewTargetMode,
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, control::Completion, function::FunctionSuperBinding,
    },
    value::{FunctionId, Value},
};

impl Context {
    pub(super) fn capture_function_lexical_this(
        &mut self,
        mode: BytecodeNewTargetMode,
        super_binding: Option<&FunctionSuperBinding>,
    ) -> Result<Option<Value>> {
        if mode != BytecodeNewTargetMode::Lexical {
            return Ok(None);
        }
        if super_binding.is_some_and(|binding| binding.constructor.is_some()) {
            return Ok(None);
        }
        self.current_this().map(Some)
    }

    pub(super) fn function_direct_call_this(
        &mut self,
        id: FunctionId,
        call_this: Value,
    ) -> Result<Value> {
        let function = self.function(id)?;
        if let Some(lexical_this) = &function.lexical_this {
            return Ok(lexical_this.clone());
        }
        if function
            .super_binding
            .as_ref()
            .is_some_and(|binding| binding.constructor.is_some())
        {
            return Ok(Value::Undefined);
        }
        if function.bytecode.strict() {
            return Ok(call_this);
        }
        if matches!(call_this, Value::Undefined | Value::Null) {
            return self.global_this_value();
        }
        self.eval_direct_object_constructor(std::slice::from_ref(&call_this))
    }

    pub(crate) fn eval_function_completion_with_this_and_new_target(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
        new_target: Value,
    ) -> Result<Completion> {
        self.eval_function_completion_with_mode::<false>(id, args, this_value, new_target)
    }

    pub(in crate::runtime) fn eval_async_function_completion_with_this_and_new_target(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
        new_target: Value,
    ) -> Result<Completion> {
        self.eval_function_completion_with_mode::<true>(id, args, this_value, new_target)
    }

    pub(in crate::runtime) fn eval_generator_function_completion_with_this_and_new_target(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
        new_target: Value,
    ) -> Result<Completion> {
        self.eval_function_completion_with_mode::<true>(id, args, this_value, new_target)
    }

    fn eval_function_completion_with_mode<const CAN_SUSPEND: bool>(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
        new_target: Value,
    ) -> Result<Completion> {
        self.call_depth = self
            .call_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("call stack depth overflowed"))?;
        if self.call_depth > self.limits.max_expression_depth {
            self.call_depth = self.call_depth.saturating_sub(1);
            return Err(Error::limit(format!(
                "call stack depth exceeded {}",
                self.limits.max_expression_depth
            )));
        }
        let result = self.eval_function_completion_with_this_inner::<CAN_SUSPEND>(
            id, args, this_value, new_target,
        );
        self.call_depth = self.call_depth.saturating_sub(1);
        result
    }
}
