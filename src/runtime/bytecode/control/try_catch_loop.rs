use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeCatchFastPath,
        BytecodeDirectThrow, BytecodeInstruction, BytecodeNumericCompareOp,
    },
    error::{Error, Result},
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    value::Value,
};

use super::{for_loop::BytecodeForLoopFastPath, loop_helpers::same_bytecode_binding};

#[derive(Debug)]
pub(super) struct BytecodeForTryCatchFastPath<'a> {
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    addend: f64,
}

impl Context {
    pub(super) fn compile_try_catch_loop_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeForTryCatchFastPath<'a>>> {
        let [
            BytecodeInstruction::Try {
                body_scoped: false,
                body_direct_throw: Some(BytecodeDirectThrow::String(thrown)),
                catch: Some(catch),
                finally_body: None,
                finally_scoped: false,
                ..
            },
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if catch.body_scoped {
            return Ok(None);
        }
        let Some(BytecodeCatchFastPath::StrictStringIncrement {
            test,
            expected,
            target,
            addend,
        }) = catch.body_fast_path.as_ref()
        else {
            return Ok(None);
        };
        if thrown != expected
            || !catch
                .param
                .as_ref()
                .is_some_and(|param| same_bytecode_binding(param, test))
            || same_bytecode_binding(index, target)
            || self.builtin_value(target.name().name())?.is_some()
        {
            return Ok(None);
        }
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeForTryCatchFastPath {
            target,
            target_cell,
            addend: *addend,
        }))
    }

    pub(super) fn try_catch_loop_fast_path_ready(
        body: &BytecodeForTryCatchFastPath<'_>,
    ) -> Result<bool> {
        Ok(matches!(
            body.target_cell.value(body.target.name())?,
            Value::Number(_)
        ))
    }

    pub(super) fn eval_try_catch_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeForTryCatchFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(fast_path.compare, BytecodeNumericCompareOp::Less)
            || fast_path.update_step.to_bits() != 1.0f64.to_bits()
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
        let Value::Number(total) = body.target_cell.value(body.target.name())? else {
            return Ok(false);
        };
        let total = body.addend.mul_add(usize_to_f64(iterations)?, total);
        let total_value = self.checked_value(Value::Number(total))?;
        self.charge_runtime_steps(iterations)?;
        self.record_bytecode_linear_direct_runs(iterations)?;
        self.assign_fast_path_cell(body.target, &body.target_cell, total_value.clone())?;
        let index_value = self.checked_value(Value::Number(usize_to_f64(limit)?))?;
        self.assign_fast_path_cell(fast_path.index, &fast_path.index_cell, index_value)?;
        state.last = total_value;
        state.pc = next;
        Ok(true)
    }
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "try catch loop index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value =
        u32::try_from(value).map_err(|_| Error::limit("try catch loop value exceeds f64 range"))?;
    Ok(f64::from(value))
}
