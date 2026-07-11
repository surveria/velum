use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, control::Completion},
    value::{FunctionId, Value},
};

impl Context {
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
