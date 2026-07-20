#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::{boxed::Box, vec::Vec};

use crate::{
    bytecode::{BytecodeCompletion, BytecodeInstruction},
    error::Result,
    runtime::CompiledBindingFrame,
    value::Value,
};

use super::fast_path::{FastValueSource, FunctionFastPathKind, source_for_binding};

const MAX_FAST_NUMBER_EXPRESSION_INSTRUCTIONS: usize = 16;

pub(super) fn compile_numeric_expression(
    empty_hoist: bool,
    param_frames: &[Option<CompiledBindingFrame>],
    instructions: &[BytecodeInstruction],
) -> Result<Option<FunctionFastPathKind>> {
    if !empty_hoist || instructions.len() > MAX_FAST_NUMBER_EXPRESSION_INSTRUCTIONS {
        return Ok(None);
    }
    let Some((completion, expression)) = instructions.split_last() else {
        return Ok(None);
    };
    if !matches!(
        completion,
        BytecodeInstruction::Complete(BytecodeCompletion::Return)
    ) {
        return Ok(None);
    }

    let mut stack = Vec::new();
    for instruction in expression {
        match instruction {
            BytecodeInstruction::LoadBinding(binding) => {
                let Some(source) = source_for_binding(param_frames, binding)? else {
                    return Ok(None);
                };
                if !matches!(source, FastValueSource::Param(_)) {
                    return Ok(None);
                }
                stack.push(source);
            }
            BytecodeInstruction::PushLiteral(Value::Number(value)) => {
                stack.push(FastValueSource::Literal(Value::Number(*value)));
            }
            BytecodeInstruction::NumberBinary(op) => {
                let Some(right) = stack.pop() else {
                    return Ok(None);
                };
                let Some(left) = stack.pop() else {
                    return Ok(None);
                };
                stack.push(FastValueSource::NumberBinary {
                    op: *op,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            _ => return Ok(None),
        }
    }

    if stack.len() != 1 {
        return Ok(None);
    }
    let Some(FastValueSource::NumberBinary { op, left, right }) = stack.pop() else {
        return Ok(None);
    };
    Ok(Some(FunctionFastPathKind::ReturnNumberBinary {
        op,
        left: *left,
        right: *right,
    }))
}
