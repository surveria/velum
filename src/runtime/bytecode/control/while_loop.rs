use std::cmp::Ordering;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeCompletion, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell, control::Completion, numeric::number_to_i32},
    value::Value,
};

#[derive(Debug)]
pub(super) struct BytecodeWhileLoopFastPath<'a> {
    index: &'a BytecodeBinding,
    index_cell: BindingCell,
    condition_compare: BytecodeNumericCompareOp,
    condition_limit: f64,
    update_step: f64,
    continue_mask_i32: i32,
    continue_op: BytecodeNumericEqualityOp,
    continue_right: f64,
    break_compare: BytecodeNumericCompareOp,
    break_limit: f64,
    total: &'a BytecodeBinding,
    total_cell: BindingCell,
    array: &'a BytecodeBinding,
    array_cell: BindingCell,
    index_mask_i32: i32,
}

impl Context {
    pub(super) fn compile_bytecode_while_loop_fast_path<'a>(
        &mut self,
        condition: &'a BytecodeBlock,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeWhileLoopFastPath<'a>>> {
        let [
            BytecodeInstruction::LoadBinding(condition_index),
            BytecodeInstruction::PushLiteral(Value::Number(condition_limit)),
            BytecodeInstruction::NumberCompare(condition_compare),
            BytecodeInstruction::StoreLast,
        ] = condition.instructions()
        else {
            return Ok(None);
        };
        let [
            BytecodeInstruction::LoadBinding(update_read),
            BytecodeInstruction::PushLiteral(Value::Number(update_step)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(update_write),
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::LoadBinding(continue_read),
            BytecodeInstruction::PushLiteral(Value::Number(continue_mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::PushLiteral(Value::Number(continue_right)),
            BytecodeInstruction::NumberEquality(continue_op),
            BytecodeInstruction::JumpIfFalse(continue_alternate),
            BytecodeInstruction::Complete(BytecodeCompletion::Continue(None)),
            BytecodeInstruction::Jump(continue_end),
            BytecodeInstruction::PushUndefined,
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::LoadBinding(break_read),
            BytecodeInstruction::PushLiteral(Value::Number(break_limit)),
            BytecodeInstruction::NumberCompare(break_compare),
            BytecodeInstruction::JumpIfFalse(break_alternate),
            BytecodeInstruction::Complete(BytecodeCompletion::Break(None)),
            BytecodeInstruction::Jump(break_end),
            BytecodeInstruction::PushUndefined,
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::LoadBinding(total_read),
            BytecodeInstruction::LoadBinding(array),
            BytecodeInstruction::LoadBinding(index_read),
            BytecodeInstruction::PushLiteral(Value::Number(index_mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::ComputedMember { .. },
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(total_write),
            BytecodeInstruction::StoreLast,
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if continue_alternate.index() != 13
            || continue_end.index() != 15
            || break_alternate.index() != 21
            || break_end.index() != 23
            || !same_bytecode_binding(condition_index, update_read)
            || !same_bytecode_binding(condition_index, update_write)
            || !same_bytecode_binding(condition_index, continue_read)
            || !same_bytecode_binding(condition_index, break_read)
            || !same_bytecode_binding(condition_index, index_read)
            || !same_bytecode_binding(total_read, total_write)
        {
            return Ok(None);
        }
        let Ok(continue_mask_i32) = number_to_i32(*continue_mask, "&") else {
            return Ok(None);
        };
        let Ok(index_mask_i32) = number_to_i32(*index_mask, "&") else {
            return Ok(None);
        };
        let Some(index_cell) = self.get_binding_bytecode(condition_index)? else {
            return Ok(None);
        };
        let Some(total_cell) = self.get_or_materialize_binding_bytecode(total_write)? else {
            return Ok(None);
        };
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        if self.builtin_value(condition_index.name().name())?.is_some()
            || self.builtin_value(total_write.name().name())?.is_some()
        {
            return Ok(None);
        }
        Ok(Some(BytecodeWhileLoopFastPath {
            index: condition_index,
            index_cell,
            condition_compare: *condition_compare,
            condition_limit: *condition_limit,
            update_step: *update_step,
            continue_mask_i32,
            continue_op: *continue_op,
            continue_right: *continue_right,
            break_compare: *break_compare,
            break_limit: *break_limit,
            total: total_write,
            total_cell,
            array,
            array_cell,
            index_mask_i32,
        }))
    }

    pub(super) fn bytecode_while_loop_fast_path_ready(
        &self,
        fast_path: &BytecodeWhileLoopFastPath<'_>,
    ) -> Result<bool> {
        Ok(matches!(
            fast_path.index_cell.value(fast_path.index.name())?,
            Value::Number(_)
        ) && matches!(
            fast_path.total_cell.value(fast_path.total.name())?,
            Value::Number(_)
        ) && self.while_loop_numeric_array_values(fast_path)?.is_some())
    }

    pub(super) fn eval_bytecode_while_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeWhileLoopFastPath<'_>,
    ) -> Result<Option<Completion>> {
        let mut last = Value::Undefined;
        let Some(array_values) = self.while_loop_numeric_array_values(fast_path)? else {
            return Ok(None);
        };
        let Value::Number(mut index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(None);
        };
        let Value::Number(mut total) = fast_path.total_cell.value(fast_path.total.name())? else {
            return Ok(None);
        };
        loop {
            self.record_bytecode_linear_direct_run()?;
            if !compare_number(
                fast_path.condition_compare,
                index,
                fast_path.condition_limit,
            ) {
                break;
            }
            self.step()?;
            self.record_bytecode_linear_direct_run()?;
            index += fast_path.update_step;
            let index_value = self.checked_value(Value::Number(index))?;
            fast_path
                .index_cell
                .assign(fast_path.index.name(), index_value)?;
            if Self::while_loop_continue(fast_path, index)? {
                continue;
            }
            if compare_number(fast_path.break_compare, index, fast_path.break_limit) {
                last = Value::Undefined;
                break;
            }
            self.record_bytecode_linear_direct_run()?;
            let Some(element) = Self::while_loop_array_element(fast_path, &array_values, index)?
            else {
                last = Value::Undefined;
                continue;
            };
            total += element;
            last = self.checked_value(Value::Number(total))?;
            fast_path
                .total_cell
                .assign(fast_path.total.name(), last.clone())?;
        }
        state.last = last;
        state.pc = next;
        Ok(None)
    }

    fn while_loop_numeric_array_values(
        &self,
        fast_path: &BytecodeWhileLoopFastPath<'_>,
    ) -> Result<Option<Vec<f64>>> {
        let Value::Object(id) = fast_path.array_cell.value(fast_path.array.name())? else {
            return Ok(None);
        };
        let Some(values) = self.objects.packed_array_values_if_array(id)? else {
            return Ok(None);
        };
        let mut numbers = Vec::with_capacity(values.len());
        for value in values {
            let Value::Number(number) = value else {
                return Ok(None);
            };
            numbers.push(number);
        }
        Ok(Some(numbers))
    }

    fn while_loop_continue(fast_path: &BytecodeWhileLoopFastPath<'_>, index: f64) -> Result<bool> {
        let masked = f64::from(number_to_i32(index, "&")? & fast_path.continue_mask_i32);
        Ok(compare_equality(
            fast_path.continue_op,
            masked,
            fast_path.continue_right,
        ))
    }

    fn while_loop_array_element(
        fast_path: &BytecodeWhileLoopFastPath<'_>,
        array_values: &[f64],
        index: f64,
    ) -> Result<Option<f64>> {
        let index = number_to_i32(index, "&")? & fast_path.index_mask_i32;
        let Ok(index) = usize::try_from(index) else {
            return Ok(None);
        };
        let Some(element) = array_values.get(index).copied() else {
            return Ok(None);
        };
        Ok(Some(element))
    }
}

fn compare_number(op: BytecodeNumericCompareOp, left: f64, right: f64) -> bool {
    match op {
        BytecodeNumericCompareOp::Less => left < right,
        BytecodeNumericCompareOp::LessEqual => left <= right,
        BytecodeNumericCompareOp::Greater => left > right,
        BytecodeNumericCompareOp::GreaterEqual => left >= right,
    }
}

fn compare_equality(op: BytecodeNumericEqualityOp, left: f64, right: f64) -> bool {
    let equal = matches!(left.partial_cmp(&right), Some(Ordering::Equal));
    match op {
        BytecodeNumericEqualityOp::Equal | BytecodeNumericEqualityOp::StrictEqual => equal,
        BytecodeNumericEqualityOp::NotEqual | BytecodeNumericEqualityOp::StrictNotEqual => !equal,
    }
}

fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}
