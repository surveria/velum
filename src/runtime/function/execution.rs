use crate::{
    bytecode::BytecodeNewTargetMode,
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        control::{Completion, TailCallReturnMode},
        function::FunctionSuperBinding,
        roots::VmRootKind,
    },
    value::{ErrorName, FunctionId, Value},
};

impl Context {
    pub(in crate::runtime) fn normalize_resumed_tail_call(
        &mut self,
        completion: Completion,
    ) -> Result<Completion> {
        let Completion::TailCall(request) = completion else {
            return Ok(completion);
        };
        let (callee, args, call_this, return_mode) = request.into_parts();
        let completion = tail_call_result(self.call(&callee, &args, call_this)?)?;
        self.normalize_tail_call_return(completion, return_mode)
    }

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
        self.enter_call_stack_frame()?;
        let result = self.eval_function_tail_chain::<CAN_SUSPEND>(
            id,
            args.as_slice().to_vec(),
            this_value,
            new_target,
        );
        self.leave_call_stack_frame();
        result
    }

    pub(in crate::runtime) fn enter_call_stack_frame(&mut self) -> Result<()> {
        let next_call_depth = self
            .call_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("call stack depth overflowed"))?;
        if next_call_depth > self.limits.max_call_depth {
            return Err(maximum_call_stack_error());
        }
        self.enter_native_stack_frame()?;
        self.call_depth = next_call_depth;
        Ok(())
    }

    pub(in crate::runtime) const fn leave_call_stack_frame(&mut self) {
        self.call_depth = self.call_depth.saturating_sub(1);
        self.leave_native_stack_frame();
    }

    pub(in crate::runtime) fn enter_native_stack_frame(&mut self) -> Result<()> {
        let stack_position = native_stack_position();
        let starts_native_stack = self.native_stack_depth == 0;
        if starts_native_stack {
            self.native_stack_base = Some(stack_position);
        }
        let stack_bytes = self
            .native_stack_base
            .map_or(0, |base| base.abs_diff(stack_position));
        let next_native_stack_depth = self
            .native_stack_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("native stack guard depth overflowed"))?;
        if stack_bytes >= self.limits.max_call_stack_bytes {
            if starts_native_stack {
                self.native_stack_base = None;
            }
            return Err(maximum_call_stack_error());
        }
        self.native_stack_depth = next_native_stack_depth;
        Ok(())
    }

    pub(in crate::runtime) const fn leave_native_stack_frame(&mut self) {
        self.native_stack_depth = self.native_stack_depth.saturating_sub(1);
        if self.native_stack_depth == 0 {
            self.native_stack_base = None;
        }
    }

    fn eval_function_tail_chain<const CAN_SUSPEND: bool>(
        &mut self,
        mut id: FunctionId,
        mut args: Vec<Value>,
        mut this_value: Value,
        mut new_target: Value,
    ) -> Result<Completion> {
        let mut return_mode = TailCallReturnMode::Ordinary;
        loop {
            let _return_root =
                self.transient_root_scope(VmRootKind::TransientCall, return_mode.root_value())?;
            let realm = self.function(id)?.realm;
            let (completion, function_return_mode) = self.with_realm(realm, |context| {
                context.eval_function_completion_with_this_inner::<CAN_SUSPEND>(
                    id,
                    RuntimeCallArgs::values(&args),
                    this_value.clone(),
                    new_target.clone(),
                )
            })?;
            return_mode = return_mode.merge(function_return_mode)?;
            let Completion::TailCall(request) = completion else {
                return self.normalize_tail_call_return(completion, return_mode);
            };
            let (callee, next_args, call_this, request_return_mode) = request.into_parts();
            return_mode = return_mode.merge(request_return_mode)?;
            if CAN_SUSPEND {
                let completion = tail_call_result(self.call(&callee, &next_args, call_this)?)?;
                return self.normalize_tail_call_return(completion, return_mode);
            }
            let Value::Function(next_id) = callee else {
                let completion = tail_call_result(self.call(&callee, &next_args, call_this)?)?;
                return self.normalize_tail_call_return(completion, return_mode);
            };
            let next_realm = self.function(next_id)?.realm;
            let Some((next_this, next_target)) = self.with_realm(next_realm, |context| {
                context.reject_class_constructor_call(next_id)?;
                if context.function(next_id)?.kind.is_async()
                    || context.function(next_id)?.kind.is_generator()
                {
                    return Ok(None);
                }
                Ok(Some((
                    context.function_direct_call_this(next_id, call_this.clone())?,
                    context.function_direct_call_new_target(next_id)?,
                )))
            })?
            else {
                let completion = tail_call_result(self.call(
                    &Value::Function(next_id),
                    &next_args,
                    call_this,
                )?)?;
                return self.normalize_tail_call_return(completion, return_mode);
            };
            id = next_id;
            args = next_args;
            this_value = next_this;
            new_target = next_target;
        }
    }
}

#[inline(never)]
fn native_stack_position() -> usize {
    let marker = 0_u8;
    std::ptr::from_ref(std::hint::black_box(&marker)).addr()
}

fn maximum_call_stack_error() -> Error {
    Error::exception(ErrorName::RangeError, "Maximum call stack size exceeded")
}

fn tail_call_result(completion: Completion) -> Result<Completion> {
    match completion {
        Completion::Normal(value) => Ok(Completion::Return(value)),
        Completion::Throw(value) => Ok(Completion::Throw(value)),
        other => Err(Error::runtime(format!(
            "tail call produced invalid completion {other:?}"
        ))),
    }
}
