use crate::{
    bytecode::{BytecodeInstruction, BytecodeNumericBinaryOp},
    error::{Error, Result},
    runtime::Context,
    value::Value,
};

use super::{BytecodeLinearOp, BytecodeState};

impl Context {
    pub(super) fn compile_push_compare_binding_number<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::NumberCompare(op),
            ],
        ) = instruction_window(instructions, index, 3)
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeLinearOp::PushCompareBindingNumber {
            binding,
            cell,
            op: *op,
            right: *right,
        }))
    }

    pub(super) fn compile_push_binding_bitand_number_equality<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::PushLiteral(Value::Number(mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::NumberEquality(op),
            ],
        ) = instruction_window(instructions, index, 5)
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeLinearOp::PushBindingBitAndNumberEquality {
            binding,
            cell,
            mask: *mask,
            op: *op,
            right: *right,
        }))
    }

    pub(super) fn eval_condition_peephole_op(
        &mut self,
        state: &mut BytecodeState,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<()> {
        match op {
            BytecodeLinearOp::PushCompareBindingNumber {
                binding,
                cell,
                op,
                right,
            } => {
                let left = self.runtime_value(cell.value(binding.name())?)?;
                let value =
                    self.eval_bytecode_number_compare(*op, &left, &Value::Number(*right))?;
                state.stack.push(value);
            }
            BytecodeLinearOp::PushBindingBitAndNumberEquality {
                binding,
                cell,
                mask,
                op,
                right,
            } => {
                let left = self.runtime_value(cell.value(binding.name())?)?;
                let masked = self.eval_bytecode_number_binary(
                    BytecodeNumericBinaryOp::BitAnd,
                    &left,
                    &Value::Number(*mask),
                )?;
                let value =
                    self.eval_bytecode_number_equality(*op, &masked, &Value::Number(*right))?;
                state.stack.push(value);
            }
            _ => return Err(Error::runtime("bytecode linear condition op mismatch")),
        }
        Ok(())
    }
}

fn instruction_window(
    instructions: &[BytecodeInstruction],
    start: usize,
    len: usize,
) -> Option<&[BytecodeInstruction]> {
    let end = start.checked_add(len)?;
    instructions.get(start..end)
}
