use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericCompareOp, BytecodeTryFinallyFastPath,
    },
    error::{Error, Result},
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    value::Value,
};

use super::{for_loop::BytecodeForLoopFastPath, loop_helpers::same_bytecode_binding};

#[derive(Debug)]
pub(super) struct BytecodeForTryFinallyFastPath<'a> {
    body: &'a BytecodeTryFinallyFastPath,
    total_cell: BindingCell,
}

impl Context {
    pub(super) fn compile_try_finally_loop_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeForTryFinallyFastPath<'a>>> {
        let [
            BytecodeInstruction::Try {
                try_fast_path: Some(try_fast_path),
                ..
            },
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if !same_bytecode_binding(index, &try_fast_path.index)
            || self
                .builtin_value(try_fast_path.total.name().name())?
                .is_some()
        {
            return Ok(None);
        }
        let Some(total_cell) = self.get_or_materialize_binding_bytecode(&try_fast_path.total)?
        else {
            return Ok(None);
        };
        Ok(Some(BytecodeForTryFinallyFastPath {
            body: try_fast_path,
            total_cell,
        }))
    }

    pub(super) fn try_finally_loop_fast_path_ready(
        body: &BytecodeForTryFinallyFastPath<'_>,
    ) -> Result<bool> {
        Ok(matches!(
            body.total_cell.value(body.body.total.name())?,
            Value::Number(_)
        ))
    }

    pub(super) fn eval_try_finally_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeForTryFinallyFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(fast_path.compare, BytecodeNumericCompareOp::Less)
            || fast_path.update_step.to_bits() != 1.0f64.to_bits()
            || !same_bytecode_binding(fast_path.index, &body.body.index)
        {
            return Ok(false);
        }
        let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(false);
        };
        let Some(start_index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        let Some(limit) = non_negative_integer_index(self.fast_loop_limit(fast_path)?) else {
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
        let Value::Number(total) = body.total_cell.value(body.body.total.name())? else {
            return Ok(false);
        };
        let outcome = try_finally_loop_outcome(total, body.body, start_index, iterations)?;
        self.charge_runtime_steps(iterations)?;
        self.record_bytecode_linear_direct_runs(iterations)?;
        let total_value = self.checked_value(Value::Number(outcome.total))?;
        self.assign_fast_path_cell(&body.body.total, &body.total_cell, total_value)?;
        let index_value = self.checked_value(Value::Number(usize_to_f64(limit)?))?;
        self.assign_fast_path_cell(fast_path.index, &fast_path.index_cell, index_value)?;
        state.last = self.checked_value(Value::Number(outcome.last_branch_value))?;
        state.pc = next;
        Ok(true)
    }
}

struct TryFinallyLoopOutcome {
    total: f64,
    last_branch_value: f64,
}

fn try_finally_loop_outcome(
    initial_total: f64,
    fast_path: &BytecodeTryFinallyFastPath,
    start_index: usize,
    iterations: usize,
) -> Result<TryFinallyLoopOutcome> {
    let throw_count = masked_match_count(
        start_index,
        iterations,
        fast_path.index_mask,
        fast_path.throw_right,
    )?;
    let normal_count = iterations
        .checked_sub(throw_count)
        .ok_or_else(|| Error::limit("try finally loop count underflowed"))?;
    let total = add_scaled_total(
        add_scaled_total(
            add_scaled_total(initial_total, fast_path.throw_value, throw_count)?,
            fast_path.try_add,
            normal_count,
        )?,
        fast_path.finally_add,
        iterations,
    )?;
    let previous_iterations = iterations
        .checked_sub(1)
        .ok_or_else(|| Error::limit("try finally loop final iteration underflowed"))?;
    let previous_throw_count = masked_match_count(
        start_index,
        previous_iterations,
        fast_path.index_mask,
        fast_path.throw_right,
    )?;
    let previous_normal_count = previous_iterations
        .checked_sub(previous_throw_count)
        .ok_or_else(|| Error::limit("try finally loop previous count underflowed"))?;
    let total_before_last = add_scaled_total(
        add_scaled_total(
            add_scaled_total(initial_total, fast_path.throw_value, previous_throw_count)?,
            fast_path.try_add,
            previous_normal_count,
        )?,
        fast_path.finally_add,
        previous_iterations,
    )?;
    let last_index = start_index
        .checked_add(previous_iterations)
        .ok_or_else(|| Error::limit("try finally loop index overflowed"))?;
    let last_branch_add =
        if masked_index_matches(last_index, fast_path.index_mask, fast_path.throw_right)? {
            fast_path.throw_value
        } else {
            fast_path.try_add
        };
    Ok(TryFinallyLoopOutcome {
        total,
        last_branch_value: total_before_last + last_branch_add,
    })
}

fn add_scaled_total(total: f64, increment: f64, count: usize) -> Result<f64> {
    Ok(increment.mul_add(usize_to_f64(count)?, total))
}

fn masked_match_count(
    start_index: usize,
    iterations: usize,
    mask: f64,
    right: f64,
) -> Result<usize> {
    let mask = usize::try_from(number_to_i32(mask, "try finally mask")?)
        .map_err(|_| Error::runtime("try finally mask is negative"))?;
    let period = mask
        .checked_add(1)
        .and_then(usize::checked_next_power_of_two)
        .ok_or_else(|| Error::limit("try finally mask overflowed"))?;
    let matches_per_period = (0..period)
        .filter(|offset| {
            let masked = *offset & mask;
            usize_to_f64(masked).is_ok_and(|value| value.to_bits() == right.to_bits())
        })
        .count();
    let mut count = matches_per_period
        .checked_mul(iterations / period)
        .ok_or_else(|| Error::limit("try finally match count overflowed"))?;
    for offset in 0..(iterations % period) {
        let index = start_index
            .checked_add(offset)
            .ok_or_else(|| Error::limit("try finally index overflowed"))?;
        if masked_index_matches_usize(index, mask, right)? {
            count = count
                .checked_add(1)
                .ok_or_else(|| Error::limit("try finally match count overflowed"))?;
        }
    }
    Ok(count)
}

fn masked_index_matches(index: usize, mask: f64, right: f64) -> Result<bool> {
    let mask = usize::try_from(number_to_i32(mask, "try finally mask")?)
        .map_err(|_| Error::runtime("try finally mask is negative"))?;
    masked_index_matches_usize(index, mask, right)
}

fn masked_index_matches_usize(index: usize, mask: usize, right: f64) -> Result<bool> {
    let masked = index & mask;
    Ok(usize_to_f64(masked)?.to_bits() == right.to_bits())
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "try finally loop index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value = u32::try_from(value)
        .map_err(|_| Error::limit("try finally loop value exceeds f64 range"))?;
    Ok(f64::from(value))
}
