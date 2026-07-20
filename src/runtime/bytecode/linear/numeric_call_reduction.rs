use crate::{
    binding_metadata::BindingOperand,
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    syntax::UpdateOp,
    value::Value,
};

use super::super::state::BytecodeState;

const LOOP_CONTROL_STEP_COST: usize = 1;

#[derive(Debug)]
pub(in crate::runtime::bytecode) struct NumericCallReductionPlan<'a> {
    index: &'a BytecodeBinding,
    index_cell: BindingCell,
    limit: usize,
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    callee: &'a BytecodeBinding,
    callee_cell: BindingCell,
    condition_step_cost: usize,
    caller_iteration_step_cost: usize,
}

impl Context {
    pub(super) fn bind_numeric_call_reduction_plan<'a>(
        &mut self,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
    ) -> Result<Option<NumericCallReductionPlan<'a>>> {
        let (Some(condition), Some(update)) = (condition, update) else {
            return Ok(None);
        };
        let [
            BytecodeInstruction::LoadBinding(condition_index),
            BytecodeInstruction::PushLiteral(Value::Number(limit)),
            BytecodeInstruction::NumberCompare(BytecodeNumericCompareOp::Less),
            BytecodeInstruction::StoreLast,
        ] = condition.instructions()
        else {
            return Ok(None);
        };
        let Some(limit) = non_negative_integer_index(*limit) else {
            return Ok(None);
        };
        let Some(update_index) = numeric_increment_binding(update) else {
            return Ok(None);
        };
        let [
            BytecodeInstruction::LoadBinding(argument),
            BytecodeInstruction::CallBinding {
                callee,
                native: None,
                arg_count: 1,
                ..
            },
            BytecodeInstruction::StoreBinding(target),
            BytecodeInstruction::StoreLast,
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if !same_binding(condition_index, update_index)
            || !same_binding(argument, target)
            || same_binding(condition_index, target)
            || binding_can_use_dynamic_environment(condition_index)
            || binding_can_use_dynamic_environment(target)
            || binding_can_use_dynamic_environment(callee)
            || self.builtin_value(condition_index.name().name())?.is_some()
            || self.builtin_value(target.name().name())?.is_some()
            || self.builtin_value(callee.name().name())?.is_some()
        {
            return Ok(None);
        }
        let Some(index_cell) = self.get_binding_bytecode(condition_index)? else {
            return Ok(None);
        };
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target)? else {
            return Ok(None);
        };
        let Some(callee_cell) = self.get_binding_bytecode(callee)? else {
            return Ok(None);
        };
        if !index_cell.kind().is_mutable() || !target_cell.kind().is_mutable() {
            return Ok(None);
        }
        let condition_step_cost = condition.instructions().len();
        let caller_iteration_step_cost = condition_step_cost
            .checked_add(body.instructions().len())
            .and_then(|cost| cost.checked_add(update.instructions().len()))
            .and_then(|cost| cost.checked_add(LOOP_CONTROL_STEP_COST))
            .ok_or_else(|| crate::Error::limit("numeric call reduction step cost overflowed"))?;
        Ok(Some(NumericCallReductionPlan {
            index: condition_index,
            index_cell,
            limit,
            target,
            target_cell,
            callee,
            callee_cell,
            condition_step_cost,
            caller_iteration_step_cost,
        }))
    }

    pub(super) fn eval_numeric_call_reduction_plan(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
        plan: &NumericCallReductionPlan<'_>,
    ) -> Result<bool> {
        let Value::Number(index) = plan.index_cell.value(plan.index.name())? else {
            return Ok(false);
        };
        let Some(mut index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        if index >= plan.limit {
            self.charge_runtime_steps(plan.condition_step_cost)?;
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(true);
        }
        let Value::Number(mut value) = plan.target_cell.value(plan.target.name())? else {
            return Ok(false);
        };
        let Value::Function(function) = plan.callee_cell.value(plan.callee.name())? else {
            return Ok(false);
        };
        let Some(function_plan) = self.bind_numeric_unary_function_fast_path(function)? else {
            return Ok(false);
        };
        let iteration_step_cost = plan
            .caller_iteration_step_cost
            .checked_add(function_plan.step_count())
            .ok_or_else(|| crate::Error::limit("numeric call reduction step cost overflowed"))?;

        self.enter_call_stack_frame()?;
        let result = (|| {
            let mut last = Value::Undefined;
            while index < plan.limit {
                if let Err(error) = self.charge_runtime_steps(iteration_step_cost) {
                    self.store_numeric_call_reduction_state(plan, index, value)?;
                    return Err(error);
                }
                value = function_plan.evaluate(value)?;
                index = index.saturating_add(1);
                last = Value::Number(value);
                self.record_bytecode_linear_direct_runs(3)?;
            }
            self.charge_runtime_steps(plan.condition_step_cost)?;
            self.store_numeric_call_reduction_state(plan, index, value)?;
            state.last = self.checked_value(last)?;
            state.pc = next;
            Ok(true)
        })();
        self.leave_call_stack_frame();
        result
    }

    fn store_numeric_call_reduction_state(
        &self,
        plan: &NumericCallReductionPlan<'_>,
        index: usize,
        value: f64,
    ) -> Result<()> {
        let index = self.checked_value(Value::Number(usize_to_f64(index)?))?;
        plan.index_cell.assign(plan.index.name(), index)?;
        let value = self.checked_value(Value::Number(value))?;
        plan.target_cell.assign(plan.target.name(), value)
    }
}

fn numeric_increment_binding(update: &BytecodeBlock) -> Option<&BytecodeBinding> {
    match update.instructions() {
        [
            BytecodeInstruction::LoadBinding(read),
            BytecodeInstruction::PushLiteral(Value::Number(step)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(write),
            BytecodeInstruction::StoreLast,
        ] if step.to_bits() == 1.0f64.to_bits() && same_binding(read, write) => Some(write),
        [
            BytecodeInstruction::UpdateBinding {
                name,
                op: UpdateOp::Increment,
                ..
            },
            BytecodeInstruction::StoreLast,
        ] => Some(name),
        _ => None,
    }
}

const fn binding_can_use_dynamic_environment(binding: &BytecodeBinding) -> bool {
    binding.with_environment_count() > 0
        || matches!(
            binding.operand(),
            BindingOperand::EvalVariable { .. } | BindingOperand::Unresolved
        )
}

fn same_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "numeric call reduction index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value = u32::try_from(value)
        .map_err(|_| crate::Error::runtime("numeric call reduction index is too large"))?;
    Ok(f64::from(value))
}
