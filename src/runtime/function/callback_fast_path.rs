use std::rc::Rc;

use crate::{
    bytecode::{BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp},
    error::Result,
    runtime::{Context, abstract_operations::number_strict_equality, control::Completion},
    value::Value,
};

use super::fast_path::{FastValueSource, FunctionFastPathKind};

impl Context {
    pub(in crate::runtime) fn eval_pure_function_callback_fast_path(
        &mut self,
        callback: &Value,
        args: &[Value],
    ) -> Result<Option<Value>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        let Value::Function(id) = callback else {
            return Ok(None);
        };
        let Some((fast_path, fast_upvalues)) = ({
            let function = self.function(*id)?;
            if !function.with_environments.is_empty() {
                return Ok(None);
            }
            function.fast_path.as_ref().and_then(|fast_path| {
                fast_path.kind.is_pure_return().then(|| {
                    let upvalues = if fast_path.needs_upvalues() {
                        Some(Rc::clone(&function.upvalues))
                    } else {
                        None
                    };
                    (Rc::clone(fast_path), upvalues)
                })
            })
        }) else {
            return Ok(None);
        };
        let upvalues = fast_upvalues.as_deref().unwrap_or(&[]);
        let Some(completion) =
            self.eval_bytecode_function_pre_setup_fast_path(&fast_path, args, upvalues)?
        else {
            return Ok(None);
        };
        let Completion::Return(value) = completion else {
            return Ok(None);
        };
        Ok(Some(value))
    }

    pub(super) fn fast_number_compare(
        &self,
        op: BytecodeNumericCompareOp,
        left: &Value,
        right: &Value,
    ) -> Result<Option<Value>> {
        let (Value::Number(left), Value::Number(right)) = (left, right) else {
            return Ok(None);
        };
        let value = match op {
            BytecodeNumericCompareOp::Less => left < right,
            BytecodeNumericCompareOp::LessEqual => left <= right,
            BytecodeNumericCompareOp::Greater => left > right,
            BytecodeNumericCompareOp::GreaterEqual => left >= right,
        };
        self.checked_value(Value::Bool(value)).map(Some)
    }

    pub(super) fn fast_number_equality(
        &self,
        op: BytecodeNumericEqualityOp,
        left: &Value,
        right: &Value,
    ) -> Result<Option<Value>> {
        let (Value::Number(left), Value::Number(right)) = (left, right) else {
            return Ok(None);
        };
        let equal = number_strict_equality(*left, *right);
        let value = match op {
            BytecodeNumericEqualityOp::Equal | BytecodeNumericEqualityOp::StrictEqual => equal,
            BytecodeNumericEqualityOp::NotEqual | BytecodeNumericEqualityOp::StrictNotEqual => {
                !equal
            }
        };
        self.checked_value(Value::Bool(value)).map(Some)
    }
}

impl FunctionFastPathKind {
    fn is_pure_return(&self) -> bool {
        matches!(
            self,
            Self::ReturnLiteral(_)
                | Self::ReturnString(_)
                | Self::ReturnUndefined
                | Self::ReturnNumberCompare { .. }
                | Self::ReturnSource(_)
        ) || matches!(
            self,
            Self::ReturnNumberBinary { op, .. } if numeric_binary_op_is_supported(*op)
        ) || matches!(
            self,
            Self::ReturnNumberEquality { left, right, .. }
                if left.uses_supported_number_ops() && right.uses_supported_number_ops()
        )
    }
}

impl FastValueSource {
    fn uses_supported_number_ops(&self) -> bool {
        match self {
            Self::Param(_) | Self::Binding(_) | Self::Literal(_) => true,
            Self::NumberBinary { op, left, right } => {
                numeric_binary_op_is_supported(*op)
                    && left.uses_supported_number_ops()
                    && right.uses_supported_number_ops()
            }
        }
    }
}

const fn numeric_binary_op_is_supported(op: BytecodeNumericBinaryOp) -> bool {
    matches!(
        op,
        BytecodeNumericBinaryOp::Add
            | BytecodeNumericBinaryOp::Sub
            | BytecodeNumericBinaryOp::Mul
            | BytecodeNumericBinaryOp::Div
            | BytecodeNumericBinaryOp::Rem
            | BytecodeNumericBinaryOp::Pow
    )
}
