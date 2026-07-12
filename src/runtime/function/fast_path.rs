use crate::{
    binding_metadata::BindingOperand,
    bytecode::{
        BytecodeBinding, BytecodeFunction, BytecodeFunctionParam, BytecodeInstruction,
        BytecodeNewTargetMode, BytecodeNumericBinaryOp, BytecodeNumericCompareOp,
        BytecodeNumericEqualityOp,
    },
    error::{Error, Result},
    runtime::{
        CompiledBindingFrame, Context,
        binding::scope::BindingCell,
        control::{Completion, reference_error_undefined},
        numeric::number_exponentiate,
    },
    syntax::{DeclKind, StaticString},
    value::Value,
};

#[derive(Debug, Clone)]
pub(in crate::runtime) struct FunctionFastPath {
    pub(super) kind: FunctionFastPathKind,
    pub(super) step_count: usize,
}

impl FunctionFastPath {
    pub(super) fn compile(
        bytecode: &BytecodeFunction,
        param_frames: &[Option<CompiledBindingFrame>],
        new_target_mode: BytecodeNewTargetMode,
        is_async: bool,
        class_constructor: bool,
    ) -> Result<Option<Self>> {
        if !can_use_pre_setup_fast_path(bytecode, new_target_mode, is_async, class_constructor) {
            return Ok(None);
        }
        let Some(kind) = compile_function_fast_path_kind(bytecode, param_frames)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            kind,
            step_count: bytecode.body().instructions().len(),
        }))
    }

    pub(super) const fn needs_upvalues(&self) -> bool {
        self.kind.needs_upvalues()
    }
}

#[derive(Debug, Clone)]
pub(super) enum FunctionFastPathKind {
    ReturnLiteral(Value),
    ReturnString(StaticString),
    ReturnUndefined,
    ReturnSource(FastValueSource),
    ReturnNumberBinary {
        op: BytecodeNumericBinaryOp,
        left: FastValueSource,
        right: FastValueSource,
    },
    ReturnNumberCompare {
        op: BytecodeNumericCompareOp,
        left: FastValueSource,
        right: FastValueSource,
    },
    ReturnNumberEquality {
        op: BytecodeNumericEqualityOp,
        left: FastValueSource,
        right: FastValueSource,
    },
    StoreNumberBinaryReturn {
        target: FastStoreTarget,
        op: BytecodeNumericBinaryOp,
        left: FastValueSource,
        right: FastValueSource,
    },
}

#[derive(Debug, Clone)]
pub(super) enum FastValueSource {
    Param(usize),
    Binding(BytecodeBinding),
    Literal(Value),
    NumberBinary {
        op: BytecodeNumericBinaryOp,
        left: Box<Self>,
        right: Box<Self>,
    },
}

#[derive(Debug, Clone)]
pub(super) enum FastStoreTarget {
    Param,
    Binding(BytecodeBinding),
}

impl FunctionFastPathKind {
    const fn needs_upvalues(&self) -> bool {
        match self {
            Self::ReturnLiteral(_) | Self::ReturnString(_) | Self::ReturnUndefined => false,
            Self::ReturnSource(source) => source.needs_upvalues(),
            Self::ReturnNumberBinary { left, right, .. }
            | Self::ReturnNumberCompare { left, right, .. }
            | Self::ReturnNumberEquality { left, right, .. } => {
                left.needs_upvalues() || right.needs_upvalues()
            }
            Self::StoreNumberBinaryReturn {
                target,
                left,
                right,
                ..
            } => target.needs_upvalues() || left.needs_upvalues() || right.needs_upvalues(),
        }
    }
}

impl FastValueSource {
    const fn needs_upvalues(&self) -> bool {
        match self {
            Self::Param(_) | Self::Literal(_) => false,
            Self::Binding(binding) => matches!(binding.operand(), BindingOperand::Upvalue { .. }),
            Self::NumberBinary { left, right, .. } => {
                left.needs_upvalues() || right.needs_upvalues()
            }
        }
    }
}

impl FastStoreTarget {
    const fn needs_upvalues(&self) -> bool {
        match self {
            Self::Param => false,
            Self::Binding(binding) => matches!(binding.operand(), BindingOperand::Upvalue { .. }),
        }
    }
}

impl Context {
    pub(super) fn eval_bytecode_function_pre_setup_fast_path(
        &mut self,
        fast_path: &FunctionFastPath,
        args: &[Value],
        upvalues: &[BindingCell],
    ) -> Result<Option<Completion>> {
        match &fast_path.kind {
            FunctionFastPathKind::ReturnLiteral(value) => {
                self.charge_runtime_steps(fast_path.step_count)?;
                Ok(Some(Completion::Return(self.runtime_value(value.clone())?)))
            }
            FunctionFastPathKind::ReturnString(value) => {
                self.charge_runtime_steps(fast_path.step_count)?;
                Ok(Some(Completion::Return(self.static_string_value(value)?)))
            }
            FunctionFastPathKind::ReturnUndefined => {
                self.charge_runtime_steps(fast_path.step_count)?;
                Ok(Some(Completion::Return(Value::Undefined)))
            }
            FunctionFastPathKind::ReturnSource(source) => {
                let Some(value) = self.load_fast_value_source(source, args, upvalues)? else {
                    return Ok(None);
                };
                self.charge_runtime_steps(fast_path.step_count)?;
                Ok(Some(Completion::Return(self.runtime_value(value)?)))
            }
            FunctionFastPathKind::ReturnNumberBinary { op, left, right } => {
                let Some(value) =
                    self.eval_fast_number_binary_sources(*op, left, right, args, upvalues)?
                else {
                    return Ok(None);
                };
                self.charge_runtime_steps(fast_path.step_count)?;
                Ok(Some(Completion::Return(value)))
            }
            FunctionFastPathKind::ReturnNumberCompare { op, left, right } => {
                let Some(value) =
                    self.eval_fast_number_compare_sources(*op, left, right, args, upvalues)?
                else {
                    return Ok(None);
                };
                self.charge_runtime_steps(fast_path.step_count)?;
                Ok(Some(Completion::Return(value)))
            }
            FunctionFastPathKind::ReturnNumberEquality { op, left, right } => {
                let Some(value) =
                    self.eval_fast_number_equality_sources(*op, left, right, args, upvalues)?
                else {
                    return Ok(None);
                };
                self.charge_runtime_steps(fast_path.step_count)?;
                Ok(Some(Completion::Return(value)))
            }
            FunctionFastPathKind::StoreNumberBinaryReturn {
                target,
                op,
                left,
                right,
            } => {
                let Some(value) =
                    self.eval_fast_number_binary_sources(*op, left, right, args, upvalues)?
                else {
                    return Ok(None);
                };
                self.charge_runtime_steps(fast_path.step_count)?;
                self.assign_fast_store_target(target, value.clone(), upvalues)?;
                Ok(Some(Completion::Return(value)))
            }
        }
    }

    fn eval_fast_number_compare_sources(
        &mut self,
        op: BytecodeNumericCompareOp,
        left: &FastValueSource,
        right: &FastValueSource,
        args: &[Value],
        upvalues: &[BindingCell],
    ) -> Result<Option<Value>> {
        let Some(left) = self.load_fast_value_source(left, args, upvalues)? else {
            return Ok(None);
        };
        let Some(right) = self.load_fast_value_source(right, args, upvalues)? else {
            return Ok(None);
        };
        self.fast_number_compare(op, &left, &right)
    }

    fn eval_fast_number_equality_sources(
        &mut self,
        op: BytecodeNumericEqualityOp,
        left: &FastValueSource,
        right: &FastValueSource,
        args: &[Value],
        upvalues: &[BindingCell],
    ) -> Result<Option<Value>> {
        let Some(left) = self.load_fast_value_source(left, args, upvalues)? else {
            return Ok(None);
        };
        let Some(right) = self.load_fast_value_source(right, args, upvalues)? else {
            return Ok(None);
        };
        self.fast_number_equality(op, &left, &right)
    }

    pub(super) fn eval_bytecode_function_fast_path(
        &mut self,
        bytecode: &BytecodeFunction,
    ) -> Result<Option<Completion>> {
        match bytecode.body().instructions() {
            [
                BytecodeInstruction::PushLiteral(value),
                BytecodeInstruction::Complete(completion),
            ] if completion.is_return() => {
                Ok(Some(Completion::Return(self.runtime_value(value.clone())?)))
            }
            [
                BytecodeInstruction::PushString(value),
                BytecodeInstruction::Complete(completion),
            ] if completion.is_return() => {
                Ok(Some(Completion::Return(self.static_string_value(value)?)))
            }
            [
                BytecodeInstruction::PushUndefined,
                BytecodeInstruction::Complete(completion),
            ] if completion.is_return() => Ok(Some(Completion::Return(Value::Undefined))),
            [
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::Complete(completion),
            ] if completion.is_return() => {
                let value = self.fast_function_load_binding(binding)?;
                Ok(Some(Completion::Return(value)))
            }
            [
                BytecodeInstruction::LoadBinding(left),
                BytecodeInstruction::LoadBinding(right),
                BytecodeInstruction::NumberBinary(op),
                BytecodeInstruction::Complete(completion),
            ] if completion.is_return() => self.fast_function_return_binary(*op, left, right),
            [
                BytecodeInstruction::LoadBinding(left),
                BytecodeInstruction::LoadBinding(right),
                BytecodeInstruction::NumberBinary(op),
                BytecodeInstruction::DeclareBinding {
                    name,
                    kind: DeclKind::Var,
                    has_init: true,
                },
                BytecodeInstruction::LoadBinding(returned),
                BytecodeInstruction::Complete(completion),
            ] if completion.is_return() && same_bytecode_binding(returned, name) => {
                self.fast_function_declare_binary_return(*op, left, right, name)
            }
            [
                BytecodeInstruction::LoadBinding(left),
                BytecodeInstruction::LoadBinding(right),
                BytecodeInstruction::NumberBinary(op),
                BytecodeInstruction::StoreBinding(target),
                BytecodeInstruction::StoreLast,
                BytecodeInstruction::LoadBinding(returned),
                BytecodeInstruction::Complete(completion),
            ] if completion.is_return() && same_bytecode_binding(returned, target) => {
                self.fast_function_store_binary_return(*op, left, right, target)
            }
            [
                BytecodeInstruction::LoadBinding(left),
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::NumberBinary(op),
                BytecodeInstruction::StoreBinding(target),
                BytecodeInstruction::StoreLast,
            ] if same_bytecode_binding(left, target) => {
                self.fast_function_store_number_normal(*op, left, *right)
            }
            _ => Ok(None),
        }
    }

    fn eval_fast_number_binary_sources(
        &mut self,
        op: BytecodeNumericBinaryOp,
        left: &FastValueSource,
        right: &FastValueSource,
        args: &[Value],
        upvalues: &[BindingCell],
    ) -> Result<Option<Value>> {
        let Some(left) = self.load_fast_value_source(left, args, upvalues)? else {
            return Ok(None);
        };
        let Some(right) = self.load_fast_value_source(right, args, upvalues)? else {
            return Ok(None);
        };
        self.fast_number_binary(op, &left, &right)
    }

    fn load_fast_value_source(
        &mut self,
        source: &FastValueSource,
        args: &[Value],
        upvalues: &[BindingCell],
    ) -> Result<Option<Value>> {
        match source {
            FastValueSource::Param(index) => {
                Ok(Some(args.get(*index).cloned().unwrap_or(Value::Undefined)))
            }
            FastValueSource::Binding(binding) => self
                .fast_pre_setup_load_binding(binding, upvalues)
                .map(Some),
            FastValueSource::Literal(value) => self.runtime_value(value.clone()).map(Some),
            FastValueSource::NumberBinary { op, left, right } => {
                let Some(left) = self.load_fast_value_source(left, args, upvalues)? else {
                    return Ok(None);
                };
                let Some(right) = self.load_fast_value_source(right, args, upvalues)? else {
                    return Ok(None);
                };
                self.fast_number_binary(*op, &left, &right)
            }
        }
    }

    fn fast_pre_setup_load_binding(
        &mut self,
        binding: &BytecodeBinding,
        upvalues: &[BindingCell],
    ) -> Result<Value> {
        if let BindingOperand::Upvalue { slot, .. } = binding.operand() {
            let cell = fast_upvalue_cell(upvalues, slot.index()?)?;
            return self.runtime_value(cell.value(binding.name())?);
        }
        self.fast_function_load_binding(binding)
    }

    fn assign_fast_store_target(
        &mut self,
        target: &FastStoreTarget,
        value: Value,
        upvalues: &[BindingCell],
    ) -> Result<()> {
        match target {
            FastStoreTarget::Param => Ok(()),
            FastStoreTarget::Binding(binding) => {
                if let BindingOperand::Upvalue { slot, .. } = binding.operand() {
                    let cell = fast_upvalue_cell(upvalues, slot.index()?)?;
                    return cell.assign(binding.name(), self.runtime_value(value)?);
                }
                self.assign_bytecode(binding, value)
            }
        }
    }

    fn fast_function_return_binary(
        &mut self,
        op: BytecodeNumericBinaryOp,
        left: &BytecodeBinding,
        right: &BytecodeBinding,
    ) -> Result<Option<Completion>> {
        let Some(value) = self.fast_function_binding_binary(op, left, right)? else {
            return Ok(None);
        };
        Ok(Some(Completion::Return(value)))
    }

    fn fast_function_declare_binary_return(
        &mut self,
        op: BytecodeNumericBinaryOp,
        left: &BytecodeBinding,
        right: &BytecodeBinding,
        target: &BytecodeBinding,
    ) -> Result<Option<Completion>> {
        let Some(value) = self.fast_function_binding_binary(op, left, right)? else {
            return Ok(None);
        };
        self.assign_bytecode(target, value.clone())?;
        Ok(Some(Completion::Return(value)))
    }

    fn fast_function_store_binary_return(
        &mut self,
        op: BytecodeNumericBinaryOp,
        left: &BytecodeBinding,
        right: &BytecodeBinding,
        target: &BytecodeBinding,
    ) -> Result<Option<Completion>> {
        let Some(value) = self.fast_function_binding_binary(op, left, right)? else {
            return Ok(None);
        };
        self.assign_bytecode(target, value.clone())?;
        Ok(Some(Completion::Return(value)))
    }

    fn fast_function_store_number_normal(
        &mut self,
        op: BytecodeNumericBinaryOp,
        binding: &BytecodeBinding,
        right: f64,
    ) -> Result<Option<Completion>> {
        let left = self.fast_function_load_binding(binding)?;
        let Some(value) = self.fast_number_binary(op, &left, &Value::Number(right))? else {
            return Ok(None);
        };
        self.assign_bytecode(binding, value.clone())?;
        Ok(Some(Completion::Normal(value)))
    }

    fn fast_function_binding_binary(
        &mut self,
        op: BytecodeNumericBinaryOp,
        left: &BytecodeBinding,
        right: &BytecodeBinding,
    ) -> Result<Option<Value>> {
        let left = self.fast_function_load_binding(left)?;
        let right = self.fast_function_load_binding(right)?;
        self.fast_number_binary(op, &left, &right)
    }

    fn fast_function_load_binding(&mut self, binding: &BytecodeBinding) -> Result<Value> {
        if let Some(value) = self.unresolved_builtin_numeric_constant(binding) {
            return Ok(value);
        }
        if let Some(cell) = self.get_binding_bytecode(binding)? {
            return self.runtime_value(cell.value(binding.name())?);
        }
        self.unresolved_global_property_value(binding.name().name())?
            .ok_or_else(|| reference_error_undefined(binding.name()))
    }

    fn fast_number_binary(
        &self,
        op: BytecodeNumericBinaryOp,
        left: &Value,
        right: &Value,
    ) -> Result<Option<Value>> {
        let (Value::Number(left), Value::Number(right)) = (left, right) else {
            return Ok(None);
        };
        let value = match op {
            BytecodeNumericBinaryOp::Add => left + right,
            BytecodeNumericBinaryOp::Sub => left - right,
            BytecodeNumericBinaryOp::Mul => left * right,
            BytecodeNumericBinaryOp::Div => left / right,
            BytecodeNumericBinaryOp::Rem => left % right,
            BytecodeNumericBinaryOp::Pow => number_exponentiate(*left, *right),
            BytecodeNumericBinaryOp::BitAnd
            | BytecodeNumericBinaryOp::BitOr
            | BytecodeNumericBinaryOp::BitXor
            | BytecodeNumericBinaryOp::ShiftLeft
            | BytecodeNumericBinaryOp::ShiftRight
            | BytecodeNumericBinaryOp::ShiftRightUnsigned => return Ok(None),
        };
        self.checked_value(Value::Number(value)).map(Some)
    }
}

fn can_use_pre_setup_fast_path(
    bytecode: &BytecodeFunction,
    new_target_mode: BytecodeNewTargetMode,
    is_async: bool,
    class_constructor: bool,
) -> bool {
    !is_async
        && !class_constructor
        && new_target_mode == BytecodeNewTargetMode::Own
        && !bytecode.uses_arguments()
        && !bytecode.has_parameter_defaults()
        && !bytecode.has_rest_parameter()
        && params_have_unique_names(bytecode.params())
}

fn compile_function_fast_path_kind(
    bytecode: &BytecodeFunction,
    param_frames: &[Option<CompiledBindingFrame>],
) -> Result<Option<FunctionFastPathKind>> {
    let instructions = bytecode.body().instructions();
    match instructions {
        [
            BytecodeInstruction::PushLiteral(value),
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return() && has_empty_hoist(bytecode) => {
            Ok(Some(FunctionFastPathKind::ReturnLiteral(value.clone())))
        }
        [
            BytecodeInstruction::PushString(value),
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return() && has_empty_hoist(bytecode) => {
            Ok(Some(FunctionFastPathKind::ReturnString(value.clone())))
        }
        [
            BytecodeInstruction::PushUndefined,
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return() && has_empty_hoist(bytecode) => {
            Ok(Some(FunctionFastPathKind::ReturnUndefined))
        }
        [
            BytecodeInstruction::LoadBinding(binding),
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return() && has_empty_hoist(bytecode) => {
            Ok(source_for_binding(param_frames, binding)?.map(FunctionFastPathKind::ReturnSource))
        }
        [
            BytecodeInstruction::LoadBinding(left),
            BytecodeInstruction::LoadBinding(right),
            BytecodeInstruction::NumberBinary(op),
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return() && has_empty_hoist(bytecode) => {
            compile_return_binary_kind(param_frames, *op, left, right)
        }
        [
            BytecodeInstruction::LoadBinding(left),
            BytecodeInstruction::PushLiteral(Value::Number(right)),
            BytecodeInstruction::NumberCompare(op),
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return() && has_empty_hoist(bytecode) => {
            compile_return_compare_number_kind(param_frames, *op, left, *right)
        }
        [
            BytecodeInstruction::LoadBinding(left),
            BytecodeInstruction::PushLiteral(Value::Number(right)),
            BytecodeInstruction::NumberEquality(op),
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return() && has_empty_hoist(bytecode) => {
            compile_return_equality_number_kind(param_frames, *op, left, *right)
        }
        [
            BytecodeInstruction::LoadBinding(left),
            BytecodeInstruction::PushLiteral(Value::Number(binary_right)),
            BytecodeInstruction::NumberBinary(binary_op),
            BytecodeInstruction::PushLiteral(Value::Number(expected)),
            BytecodeInstruction::NumberEquality(equality_op),
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return() && has_empty_hoist(bytecode) => {
            compile_return_binary_number_equality_kind(
                param_frames,
                *binary_op,
                left,
                *binary_right,
                *equality_op,
                *expected,
            )
        }
        [
            BytecodeInstruction::LoadBinding(left),
            BytecodeInstruction::LoadBinding(right),
            BytecodeInstruction::NumberBinary(op),
            BytecodeInstruction::DeclareBinding {
                name,
                kind: DeclKind::Var,
                has_init: true,
            },
            BytecodeInstruction::LoadBinding(returned),
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return()
            && same_bytecode_binding(returned, name)
            && has_single_var_hoist(bytecode, name) =>
        {
            compile_return_binary_kind(param_frames, *op, left, right)
        }
        [
            BytecodeInstruction::LoadBinding(left),
            BytecodeInstruction::LoadBinding(right),
            BytecodeInstruction::NumberBinary(op),
            BytecodeInstruction::StoreBinding(target),
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::LoadBinding(returned),
            BytecodeInstruction::Complete(completion),
        ] if completion.is_return()
            && same_bytecode_binding(returned, target)
            && has_empty_hoist(bytecode) =>
        {
            compile_store_binary_return_kind(param_frames, target, *op, left, right)
        }
        _ => Ok(None),
    }
}

fn compile_return_compare_number_kind(
    param_frames: &[Option<CompiledBindingFrame>],
    op: BytecodeNumericCompareOp,
    left: &BytecodeBinding,
    right: f64,
) -> Result<Option<FunctionFastPathKind>> {
    let Some(left) = source_for_binding(param_frames, left)? else {
        return Ok(None);
    };
    Ok(Some(FunctionFastPathKind::ReturnNumberCompare {
        op,
        left,
        right: FastValueSource::Literal(Value::Number(right)),
    }))
}

fn compile_return_equality_number_kind(
    param_frames: &[Option<CompiledBindingFrame>],
    op: BytecodeNumericEqualityOp,
    left: &BytecodeBinding,
    right: f64,
) -> Result<Option<FunctionFastPathKind>> {
    let Some(left) = source_for_binding(param_frames, left)? else {
        return Ok(None);
    };
    Ok(Some(FunctionFastPathKind::ReturnNumberEquality {
        op,
        left,
        right: FastValueSource::Literal(Value::Number(right)),
    }))
}

fn compile_return_binary_number_equality_kind(
    param_frames: &[Option<CompiledBindingFrame>],
    binary_op: BytecodeNumericBinaryOp,
    left: &BytecodeBinding,
    binary_right: f64,
    equality_op: BytecodeNumericEqualityOp,
    expected: f64,
) -> Result<Option<FunctionFastPathKind>> {
    let Some(left) = source_for_binding(param_frames, left)? else {
        return Ok(None);
    };
    Ok(Some(FunctionFastPathKind::ReturnNumberEquality {
        op: equality_op,
        left: FastValueSource::NumberBinary {
            op: binary_op,
            left: Box::new(left),
            right: Box::new(FastValueSource::Literal(Value::Number(binary_right))),
        },
        right: FastValueSource::Literal(Value::Number(expected)),
    }))
}

fn compile_return_binary_kind(
    param_frames: &[Option<CompiledBindingFrame>],
    op: BytecodeNumericBinaryOp,
    left: &BytecodeBinding,
    right: &BytecodeBinding,
) -> Result<Option<FunctionFastPathKind>> {
    let Some(left) = source_for_binding(param_frames, left)? else {
        return Ok(None);
    };
    let Some(right) = source_for_binding(param_frames, right)? else {
        return Ok(None);
    };
    Ok(Some(FunctionFastPathKind::ReturnNumberBinary {
        op,
        left,
        right,
    }))
}

fn compile_store_binary_return_kind(
    param_frames: &[Option<CompiledBindingFrame>],
    target: &BytecodeBinding,
    op: BytecodeNumericBinaryOp,
    left: &BytecodeBinding,
    right: &BytecodeBinding,
) -> Result<Option<FunctionFastPathKind>> {
    let Some(target) = store_target_for_binding(param_frames, target)? else {
        return Ok(None);
    };
    let Some(left) = source_for_binding(param_frames, left)? else {
        return Ok(None);
    };
    let Some(right) = source_for_binding(param_frames, right)? else {
        return Ok(None);
    };
    Ok(Some(FunctionFastPathKind::StoreNumberBinaryReturn {
        target,
        op,
        left,
        right,
    }))
}

fn source_for_binding(
    param_frames: &[Option<CompiledBindingFrame>],
    binding: &BytecodeBinding,
) -> Result<Option<FastValueSource>> {
    if let Some(index) = param_index_for_operand(param_frames, binding.operand())? {
        return Ok(Some(FastValueSource::Param(index)));
    }
    if matches!(binding.operand(), BindingOperand::Local { .. }) {
        return Ok(None);
    }
    Ok(Some(FastValueSource::Binding(binding.clone())))
}

fn store_target_for_binding(
    param_frames: &[Option<CompiledBindingFrame>],
    binding: &BytecodeBinding,
) -> Result<Option<FastStoreTarget>> {
    if param_index_for_operand(param_frames, binding.operand())?.is_some() {
        return Ok(Some(FastStoreTarget::Param));
    }
    if matches!(binding.operand(), BindingOperand::Local { .. }) {
        return Ok(None);
    }
    Ok(Some(FastStoreTarget::Binding(binding.clone())))
}

fn param_index_for_operand(
    param_frames: &[Option<CompiledBindingFrame>],
    operand: BindingOperand,
) -> Result<Option<usize>> {
    let BindingOperand::Local { scope, slot } = operand else {
        return Ok(None);
    };
    let slot = slot.index()?;
    Ok(param_frames.iter().enumerate().find_map(|(index, frame)| {
        let frame = frame.as_ref()?;
        (frame.scope() == Some(scope) && frame.slot().index() == slot).then_some(index)
    }))
}

fn params_have_unique_names(params: &[BytecodeFunctionParam]) -> bool {
    for (index, param) in params.iter().enumerate() {
        if params
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.binding().name() == param.binding().name())
        {
            return false;
        }
    }
    true
}

fn has_empty_hoist(bytecode: &BytecodeFunction) -> bool {
    bytecode.hoist_plan().lexical_declaration_count() == 0
        && bytecode.hoist_plan().var_declaration_count() == 0
        && bytecode.hoist_plan().function_declaration_count() == 0
}

fn has_single_var_hoist(bytecode: &BytecodeFunction, binding: &BytecodeBinding) -> bool {
    bytecode.hoist_plan().lexical_declaration_count() == 0
        && bytecode.hoist_plan().function_declaration_count() == 0
        && bytecode.hoist_plan().var_declaration_count() == 1
        && bytecode
            .hoist_plan()
            .var_declarations()
            .first()
            .is_some_and(|declaration| declaration.id() == binding.name().id())
}

fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.name().name() == right.name().name() && left.operand() == right.operand()
}

fn fast_upvalue_cell(upvalues: &[BindingCell], slot: usize) -> Result<BindingCell> {
    upvalues
        .get(slot)
        .cloned()
        .ok_or_else(|| Error::runtime("function fast path upvalue slot is not defined"))
}

trait BytecodeCompletionExt {
    fn is_return(&self) -> bool;
}

impl BytecodeCompletionExt for crate::bytecode::BytecodeCompletion {
    fn is_return(&self) -> bool {
        matches!(self, Self::Return)
    }
}
