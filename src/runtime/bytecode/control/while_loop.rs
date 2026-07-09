use std::cmp::Ordering;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeCompletion, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
    },
    error::{Error, Result},
    runtime::{Context, binding::scope::BindingCell, control::Completion, numeric::number_to_i32},
    syntax::DeclKind,
    value::Value,
};

#[derive(Debug)]
pub(super) enum BytecodeWhileLoopFastPath<'a> {
    BreakContinue(BreakContinueWhileLoopFastPath<'a>),
    SimpleArraySum(SimpleArraySumWhileLoopFastPath<'a>),
}

#[derive(Debug)]
pub(super) struct BreakContinueWhileLoopFastPath<'a> {
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

#[derive(Debug)]
pub(super) struct SimpleArraySumWhileLoopFastPath<'a> {
    index: &'a BytecodeBinding,
    index_cell: BindingCell,
    condition_compare: BytecodeNumericCompareOp,
    condition_limit: f64,
    slot: &'a BytecodeBinding,
    slot_cell: BindingCell,
    slot_mask_i32: i32,
    total: &'a BytecodeBinding,
    total_cell: BindingCell,
    array: &'a BytecodeBinding,
    array_cell: BindingCell,
    update_step: f64,
}

struct SimpleArraySumWhileLoopParts<'a> {
    slot_index_read: &'a BytecodeBinding,
    slot_mask: f64,
    slot_write: &'a BytecodeBinding,
    total_read: &'a BytecodeBinding,
    array: &'a BytecodeBinding,
    slot_read: &'a BytecodeBinding,
    total_write: &'a BytecodeBinding,
    update_read: &'a BytecodeBinding,
    update_step: f64,
    update_write: &'a BytecodeBinding,
}

impl Context {
    pub(super) fn compile_bytecode_while_loop_fast_path<'a>(
        &mut self,
        condition: &'a BytecodeBlock,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeWhileLoopFastPath<'a>>> {
        if let Some(fast_path) =
            self.compile_break_continue_while_loop_fast_path(condition, body)?
        {
            return Ok(Some(BytecodeWhileLoopFastPath::BreakContinue(fast_path)));
        }
        if let Some(fast_path) =
            self.compile_simple_array_sum_while_loop_fast_path(condition, body)?
        {
            return Ok(Some(BytecodeWhileLoopFastPath::SimpleArraySum(fast_path)));
        }
        Ok(None)
    }

    fn compile_break_continue_while_loop_fast_path<'a>(
        &mut self,
        condition: &'a BytecodeBlock,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BreakContinueWhileLoopFastPath<'a>>> {
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
        Ok(Some(BreakContinueWhileLoopFastPath {
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

    fn compile_simple_array_sum_while_loop_fast_path<'a>(
        &mut self,
        condition: &'a BytecodeBlock,
        body: &'a BytecodeBlock,
    ) -> Result<Option<SimpleArraySumWhileLoopFastPath<'a>>> {
        let [
            BytecodeInstruction::LoadBinding(condition_index),
            BytecodeInstruction::PushLiteral(Value::Number(condition_limit)),
            BytecodeInstruction::NumberCompare(condition_compare),
            BytecodeInstruction::StoreLast,
        ] = condition.instructions()
        else {
            return Ok(None);
        };
        let Some(parts) = simple_array_sum_while_body_parts(body) else {
            return Ok(None);
        };
        if !same_bytecode_binding(condition_index, parts.slot_index_read)
            || !same_bytecode_binding(condition_index, parts.update_read)
            || !same_bytecode_binding(condition_index, parts.update_write)
            || !same_bytecode_binding(parts.slot_write, parts.slot_read)
            || !same_bytecode_binding(parts.total_read, parts.total_write)
        {
            return Ok(None);
        }
        let Ok(slot_mask_i32) = number_to_i32(parts.slot_mask, "&") else {
            return Ok(None);
        };
        if slot_mask_i32 < 0 || parts.update_step <= 0.0 {
            return Ok(None);
        }
        let Some(index_cell) = self.get_binding_bytecode(condition_index)? else {
            return Ok(None);
        };
        let Some(slot_cell) = self.get_or_materialize_binding_bytecode(parts.slot_write)? else {
            return Ok(None);
        };
        let Some(total_cell) = self.get_or_materialize_binding_bytecode(parts.total_write)? else {
            return Ok(None);
        };
        let Some(array_cell) = self.get_binding_bytecode(parts.array)? else {
            return Ok(None);
        };
        if self.builtin_value(condition_index.name().name())?.is_some()
            || self
                .builtin_value(parts.slot_write.name().name())?
                .is_some()
            || self
                .builtin_value(parts.total_write.name().name())?
                .is_some()
        {
            return Ok(None);
        }
        Ok(Some(SimpleArraySumWhileLoopFastPath {
            index: condition_index,
            index_cell,
            condition_compare: *condition_compare,
            condition_limit: *condition_limit,
            slot: parts.slot_write,
            slot_cell,
            slot_mask_i32,
            total: parts.total_write,
            total_cell,
            array: parts.array,
            array_cell,
            update_step: parts.update_step,
        }))
    }

    pub(super) fn bytecode_while_loop_fast_path_ready(
        &self,
        fast_path: &BytecodeWhileLoopFastPath<'_>,
    ) -> Result<bool> {
        match fast_path {
            BytecodeWhileLoopFastPath::BreakContinue(fast_path) => Ok(matches!(
                fast_path.index_cell.value(fast_path.index.name())?,
                Value::Number(_)
            ) && matches!(
                fast_path.total_cell.value(fast_path.total.name())?,
                Value::Number(_)
            ) && self
                .while_loop_numeric_array_values(&fast_path.array_cell, fast_path.array)?
                .is_some()),
            BytecodeWhileLoopFastPath::SimpleArraySum(fast_path) => {
                let Some(values) =
                    self.while_loop_numeric_array_values(&fast_path.array_cell, fast_path.array)?
                else {
                    return Ok(false);
                };
                let Ok(max_slot) = usize::try_from(fast_path.slot_mask_i32) else {
                    return Ok(false);
                };
                Ok(matches!(
                    fast_path.index_cell.value(fast_path.index.name())?,
                    Value::Number(index) if index >= 0.0
                ) && matches!(
                    fast_path.total_cell.value(fast_path.total.name())?,
                    Value::Number(_)
                ) && max_slot < values.len())
            }
        }
    }

    pub(super) fn eval_bytecode_while_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeWhileLoopFastPath<'_>,
    ) -> Result<Option<Completion>> {
        match fast_path {
            BytecodeWhileLoopFastPath::BreakContinue(fast_path) => {
                self.eval_break_continue_while_loop_fast_path(state, next, fast_path)
            }
            BytecodeWhileLoopFastPath::SimpleArraySum(fast_path) => {
                self.eval_simple_array_sum_while_loop_fast_path(state, next, fast_path)
            }
        }
    }

    fn eval_break_continue_while_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BreakContinueWhileLoopFastPath<'_>,
    ) -> Result<Option<Completion>> {
        let mut last = Value::Undefined;
        let Some(array_values) =
            self.while_loop_numeric_array_values(&fast_path.array_cell, fast_path.array)?
        else {
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

    fn eval_simple_array_sum_while_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &SimpleArraySumWhileLoopFastPath<'_>,
    ) -> Result<Option<Completion>> {
        let Some(array_values) =
            self.while_loop_numeric_array_values(&fast_path.array_cell, fast_path.array)?
        else {
            return Ok(None);
        };
        let Ok(max_slot) = usize::try_from(fast_path.slot_mask_i32) else {
            return Ok(None);
        };
        if max_slot >= array_values.len() {
            return Ok(None);
        }
        let Value::Number(mut index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(None);
        };
        if index < 0.0 {
            return Ok(None);
        }
        let Value::Number(mut total) = fast_path.total_cell.value(fast_path.total.name())? else {
            return Ok(None);
        };
        if self.eval_simple_array_sum_while_loop_batch(
            state,
            next,
            fast_path,
            &array_values,
            index,
            total,
        )? {
            return Ok(None);
        }
        let mut final_slot = None;
        while compare_number(
            fast_path.condition_compare,
            index,
            fast_path.condition_limit,
        ) {
            self.step()?;
            self.record_bytecode_linear_direct_run()?;
            let slot_i32 = number_to_i32(index, "&")? & fast_path.slot_mask_i32;
            let Ok(slot_index) = usize::try_from(slot_i32) else {
                return Ok(None);
            };
            let Some(element) = array_values.get(slot_index).copied() else {
                return Ok(None);
            };
            total += element;
            index += fast_path.update_step;
            final_slot = Some(slot_i32);
        }
        let mut last = Value::Undefined;
        if let Some(slot_i32) = final_slot {
            let slot_value = self.checked_value(Value::Number(f64::from(slot_i32)))?;
            fast_path
                .slot_cell
                .assign(fast_path.slot.name(), slot_value)?;
            let total_value = self.checked_value(Value::Number(total))?;
            fast_path
                .total_cell
                .assign(fast_path.total.name(), total_value)?;
            last = self.checked_value(Value::Number(index))?;
            fast_path
                .index_cell
                .assign(fast_path.index.name(), last.clone())?;
        }
        state.last = last;
        state.pc = next;
        Ok(None)
    }

    fn eval_simple_array_sum_while_loop_batch(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &SimpleArraySumWhileLoopFastPath<'_>,
        array_values: &[f64],
        index: f64,
        total: f64,
    ) -> Result<bool> {
        if !matches!(fast_path.condition_compare, BytecodeNumericCompareOp::Less)
            || fast_path.update_step.to_bits() != 1.0f64.to_bits()
        {
            return Ok(false);
        }
        let Some(start_index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        let Some(limit) = non_negative_integer_index(fast_path.condition_limit) else {
            return Ok(false);
        };
        let Some(iterations) = limit.checked_sub(start_index) else {
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(true);
        };
        if iterations == 0 {
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(true);
        }
        self.charge_runtime_steps(iterations)?;
        self.record_bytecode_linear_direct_runs(iterations)?;
        let (total, final_slot) = simple_array_sum_total(
            total,
            array_values,
            fast_path.slot_mask_i32,
            start_index,
            iterations,
        )?;
        let final_index = start_index
            .checked_add(iterations)
            .ok_or_else(|| Error::limit("while loop index overflowed"))?;
        let slot_value = self.checked_value(Value::Number(f64::from(final_slot)))?;
        fast_path
            .slot_cell
            .assign(fast_path.slot.name(), slot_value)?;
        let total_value = self.checked_value(Value::Number(total))?;
        fast_path
            .total_cell
            .assign(fast_path.total.name(), total_value)?;
        let index_value = self.checked_value(Value::Number(usize_to_f64(final_index)?))?;
        fast_path
            .index_cell
            .assign(fast_path.index.name(), index_value.clone())?;
        state.last = index_value;
        state.pc = next;
        Ok(true)
    }

    fn while_loop_numeric_array_values(
        &self,
        array_cell: &BindingCell,
        array: &BytecodeBinding,
    ) -> Result<Option<Vec<f64>>> {
        let Value::Object(id) = array_cell.value(array.name())? else {
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

    fn while_loop_continue(
        fast_path: &BreakContinueWhileLoopFastPath<'_>,
        index: f64,
    ) -> Result<bool> {
        let masked = f64::from(number_to_i32(index, "&")? & fast_path.continue_mask_i32);
        Ok(compare_equality(
            fast_path.continue_op,
            masked,
            fast_path.continue_right,
        ))
    }

    fn while_loop_array_element(
        fast_path: &BreakContinueWhileLoopFastPath<'_>,
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

fn simple_array_sum_while_body_parts(
    body: &BytecodeBlock,
) -> Option<SimpleArraySumWhileLoopParts<'_>> {
    let [
        BytecodeInstruction::LoadBinding(slot_index_read),
        BytecodeInstruction::PushLiteral(Value::Number(slot_mask)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
        rest @ ..,
    ] = body.instructions()
    else {
        return None;
    };
    let (slot_write, rest) = simple_array_sum_slot_write(rest)?;
    let [
        BytecodeInstruction::LoadBinding(total_read),
        BytecodeInstruction::LoadBinding(array),
        BytecodeInstruction::LoadBinding(slot_read),
        BytecodeInstruction::ComputedMember { .. },
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StoreBinding(total_write),
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::LoadBinding(update_read),
        BytecodeInstruction::PushLiteral(Value::Number(update_step)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StoreBinding(update_write),
        BytecodeInstruction::StoreLast,
    ] = rest
    else {
        return None;
    };
    Some(SimpleArraySumWhileLoopParts {
        slot_index_read,
        slot_mask: *slot_mask,
        slot_write,
        total_read,
        array,
        slot_read,
        total_write,
        update_read,
        update_step: *update_step,
        update_write,
    })
}

fn simple_array_sum_slot_write(
    instructions: &[BytecodeInstruction],
) -> Option<(&BytecodeBinding, &[BytecodeInstruction])> {
    if let [
        BytecodeInstruction::StoreBinding(slot_write),
        BytecodeInstruction::StoreLast,
        rest @ ..,
    ] = instructions
    {
        return Some((slot_write, rest));
    }
    if let [
        BytecodeInstruction::DeclareBinding {
            name: slot_write,
            kind: DeclKind::Var,
            has_init: true,
        },
        rest @ ..,
    ] = instructions
    {
        return Some((slot_write, rest));
    }
    None
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

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "while loop index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn simple_array_sum_total(
    mut total: f64,
    values: &[f64],
    mask_i32: i32,
    start_index: usize,
    iterations: usize,
) -> Result<(f64, i32)> {
    let mask = usize::try_from(mask_i32)
        .map_err(|_| Error::runtime("while loop array mask is negative"))?;
    let period = mask
        .checked_add(1)
        .and_then(usize::checked_next_power_of_two)
        .ok_or_else(|| Error::limit("while loop array mask overflowed"))?;
    let mut cycle_total = 0.0;
    for offset in 0..period {
        let slot = offset & mask;
        let Some(value) = values.get(slot).copied() else {
            return Err(Error::runtime("while loop array slot is out of bounds"));
        };
        cycle_total += value;
    }
    let full_cycles = iterations / period;
    total += cycle_total * usize_to_f64(full_cycles)?;
    let remainder = iterations % period;
    for offset in 0..remainder {
        let index = start_index
            .checked_add(offset)
            .ok_or_else(|| Error::limit("while loop index overflowed"))?;
        let slot = index & mask;
        let Some(value) = values.get(slot).copied() else {
            return Err(Error::runtime("while loop array slot is out of bounds"));
        };
        total += value;
    }
    let final_index = start_index
        .checked_add(iterations)
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| Error::limit("while loop final index overflowed"))?;
    let final_slot = final_index & mask;
    let final_slot = i32::try_from(final_slot)
        .map_err(|_| Error::limit("while loop final slot exceeds i32 range"))?;
    Ok((total, final_slot))
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value =
        u32::try_from(value).map_err(|_| Error::limit("while loop index exceeds f64 range"))?;
    Ok(f64::from(value))
}
