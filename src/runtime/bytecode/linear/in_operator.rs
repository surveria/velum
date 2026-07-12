use crate::{
    bytecode::{BytecodeInstruction, BytecodeNumericBinaryOp},
    error::{Error, Result},
    runtime::{Context, bytecode::state::BytecodeState, numeric::number_to_i32},
    syntax::BinaryOp,
    value::Value,
};

use super::{BytecodeLinearOp, instruction_window};

impl Context {
    pub(super) fn compile_in_static_property_binding<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<(BytecodeLinearOp<'a>, usize)>> {
        if let Some(op) = self.compile_in_static_property_binding_op(instructions, index, true)? {
            return Ok(Some((op, 3)));
        }
        if let Some(op) = self.compile_in_static_property_binding_op(instructions, index, false)? {
            return Ok(Some((op, 2)));
        }
        Ok(None)
    }

    pub(super) fn compile_in_array_index_mask_binding<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::PushLiteral(Value::Number(mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::Binary {
                    op: BinaryOp::In,
                    property_access: Some(access),
                },
            ],
        ) = instruction_window(instructions, index, 5)
        else {
            return Ok(None);
        };
        let Some(index_cell) = self.get_binding_bytecode(index_binding)? else {
            return Ok(None);
        };
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeLinearOp::InArrayIndexMaskBinding {
            index: index_binding,
            index_cell,
            mask: *mask,
            array,
            array_cell,
            access: *access,
        }))
    }

    fn compile_in_static_property_binding_op<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
        store_last: bool,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::InStaticProperty { property, access },
            ],
        ) = instruction_window(instructions, index, 2)
        else {
            return Ok(None);
        };
        if store_last
            && !matches!(
                instruction_window(instructions, index, 3),
                Some([
                    BytecodeInstruction::LoadBinding(_),
                    BytecodeInstruction::InStaticProperty { .. },
                    BytecodeInstruction::StoreLast,
                ])
            )
        {
            return Ok(None);
        }
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeLinearOp::InStaticPropertyBinding {
            binding,
            cell,
            property,
            access: *access,
            store_last,
        }))
    }

    pub(super) fn eval_in_static_property_binding(
        &mut self,
        state: &mut BytecodeState,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<()> {
        let BytecodeLinearOp::InStaticPropertyBinding {
            binding,
            cell,
            property,
            access,
            store_last,
        } = op
        else {
            return Err(Error::runtime("bytecode linear in op mismatch"));
        };
        let object = self.checked_value(cell.value(binding.name())?)?;
        let value = self.eval_bytecode_in_static_property(&object, property, *access)?;
        if *store_last {
            state.last = value;
        } else {
            state.stack.push(value);
        }
        Ok(())
    }

    pub(super) fn eval_in_array_index_mask_binding(
        &mut self,
        state: &mut BytecodeState,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<()> {
        let BytecodeLinearOp::InArrayIndexMaskBinding {
            index,
            index_cell,
            mask,
            array,
            array_cell,
            access,
        } = op
        else {
            return Err(Error::runtime("bytecode linear array in op mismatch"));
        };
        let index_value = self.checked_value(index_cell.value(index.name())?)?;
        let object = self.checked_value(array_cell.value(array.name())?)?;
        let property = if let Value::Number(index_value) = index_value {
            let index = number_to_i32(index_value, "&")? & number_to_i32(*mask, "&")?;
            if self.has_own_array_index_for_in(&object, index)? == Some(true) {
                state.stack.push(Value::Bool(true));
                return Ok(());
            }
            Value::Number(f64::from(index))
        } else {
            self.eval_bytecode_number_binary(
                BytecodeNumericBinaryOp::BitAnd,
                &index_value,
                &Value::Number(*mask),
            )?
        };
        state
            .stack
            .push(self.eval_bytecode_in(&property, &object, Some(*access))?);
        Ok(())
    }
}
