use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    syntax::{DeclKind, StaticString},
    value::Value,
};

use super::{for_loop::BytecodeForLoopFastPath, loop_helpers::same_bytecode_binding};

#[derive(Debug)]
pub(super) struct BytecodeForStringConcatLengthFastPath<'a> {
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    index: &'a BytecodeBinding,
    prefix_len: usize,
}

impl Context {
    pub(super) fn compile_bytecode_for_string_concat_length_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeForStringConcatLengthFastPath<'a>>> {
        let [BytecodeInstruction::ScopedBlock(block)] = body.instructions() else {
            return Ok(None);
        };
        let [
            BytecodeInstruction::PushString(left_text),
            BytecodeInstruction::DeclareBinding {
                name: left_binding,
                kind: DeclKind::Let,
                has_init: true,
            },
            BytecodeInstruction::LoadBinding(left_read),
            BytecodeInstruction::StringConcatStatic {
                text: middle_text,
                final_result: false,
            },
            BytecodeInstruction::LoadBinding(index_read),
            BytecodeInstruction::StringConcat { final_result: true },
            BytecodeInstruction::DeclareBinding {
                name: result_binding,
                kind: DeclKind::Let,
                has_init: true,
            },
            BytecodeInstruction::LoadBinding(target_read),
            BytecodeInstruction::LoadBinding(result_read),
            BytecodeInstruction::ArrayLength { .. },
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(target_write),
            BytecodeInstruction::StoreLast,
        ] = block.instructions()
        else {
            return Ok(None);
        };
        if !same_bytecode_binding(left_binding, left_read)
            || !same_bytecode_binding(index, index_read)
            || !same_bytecode_binding(result_binding, result_read)
            || !same_bytecode_binding(target_read, target_write)
        {
            return Ok(None);
        }
        if self.builtin_value(target_write.name().name())?.is_some() {
            return Ok(None);
        }
        let Some(target_cell) = self.get_binding_bytecode(target_write)? else {
            return Ok(None);
        };
        let Some(prefix_len) = string_prefix_len(left_text, middle_text) else {
            return Ok(None);
        };
        Ok(Some(BytecodeForStringConcatLengthFastPath {
            target: target_write,
            target_cell,
            index,
            prefix_len,
        }))
    }

    pub(super) fn bytecode_for_string_concat_length_fast_path_ready(
        body: &BytecodeForStringConcatLengthFastPath<'_>,
    ) -> Result<bool> {
        Ok(matches!(
            body.target_cell.value(body.target.name())?,
            Value::Number(_)
        ))
    }

    pub(super) fn eval_bytecode_for_string_concat_length_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeForStringConcatLengthFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(fast_path.compare, BytecodeNumericCompareOp::Less)
            || fast_path.update_step.to_bits() != 1.0f64.to_bits()
            || !same_bytecode_binding(fast_path.index, body.index)
        {
            return Ok(false);
        }
        let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(false);
        };
        let Value::Number(mut total) = body.target_cell.value(body.target.name())? else {
            return Ok(false);
        };
        let Some(mut index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        let Some(limit) = non_negative_integer_index(self.fast_loop_limit(fast_path)?) else {
            return Ok(false);
        };
        while index < limit {
            self.step()?;
            self.record_bytecode_linear_direct_run()?;
            let Some(length) = body
                .prefix_len
                .checked_add(decimal_digit_count(index))
                .filter(|length| *length <= self.limits.max_string_len)
            else {
                return Ok(false);
            };
            let Some(length) = usize_to_f64(length) else {
                return Ok(false);
            };
            total += length;
            index = index.saturating_add(1);
        }
        let total_value = self.checked_value(Value::Number(total))?;
        self.assign_fast_path_cell(body.target, &body.target_cell, total_value.clone())?;
        let Some(index_number) = usize_to_f64(index) else {
            return Ok(false);
        };
        let index_value = self.checked_value(Value::Number(index_number))?;
        self.assign_fast_path_cell(fast_path.index, &fast_path.index_cell, index_value)?;
        state.last = total_value;
        state.pc = next;
        Ok(true)
    }
}

fn string_prefix_len(left: &StaticString, middle: &StaticString) -> Option<usize> {
    left.as_str().len().checked_add(middle.as_str().len())
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "string concat loop index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn usize_to_f64(value: usize) -> Option<f64> {
    u32::try_from(value).ok().map(f64::from)
}

const fn decimal_digit_count(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}
