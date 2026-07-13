use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericArrayReductionRole, BytecodeNumericBinaryOp, BytecodeNumericCompareOp,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    syntax::UpdateOp,
    value::Value,
};

use super::super::state::BytecodeState;

const CONDITION_STEP_COST: usize = 5;
const ITERATION_STEP_COST: usize = 18;

#[derive(Debug)]
pub(in crate::runtime::bytecode) struct NumericArrayReductionPlan<'a> {
    index: &'a BytecodeBinding,
    index_cell: BindingCell,
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    array: &'a BytecodeBinding,
    array_cell: BindingCell,
}

impl Context {
    pub(in crate::runtime::bytecode) fn bind_numeric_array_reduction_plan<'a>(
        &mut self,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
    ) -> Result<Option<NumericArrayReductionPlan<'a>>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        let (Some(condition), Some(update)) = (condition, update) else {
            return Ok(None);
        };
        if condition.linear_template().reduction_role()
            != Some(BytecodeNumericArrayReductionRole::Condition)
            || update.linear_template().reduction_role()
                != Some(BytecodeNumericArrayReductionRole::Update)
            || body.linear_template().reduction_role()
                != Some(BytecodeNumericArrayReductionRole::Body)
        {
            return Ok(None);
        }
        let [
            BytecodeInstruction::LoadBinding(condition_index),
            BytecodeInstruction::LoadBinding(condition_array),
            BytecodeInstruction::ArrayLength { .. },
            BytecodeInstruction::NumberCompare(BytecodeNumericCompareOp::Less),
            BytecodeInstruction::StoreLast,
        ] = condition.instructions()
        else {
            return Ok(None);
        };
        let Some(update_index) = numeric_increment_binding(update) else {
            return Ok(None);
        };
        let [
            BytecodeInstruction::LoadBinding(target_read),
            BytecodeInstruction::LoadBinding(body_array),
            BytecodeInstruction::LoadBinding(body_index),
            BytecodeInstruction::ComputedMember { .. },
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(target_write),
            BytecodeInstruction::StoreLast,
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if !same_binding(condition_index, update_index)
            || !same_binding(condition_index, body_index)
            || !same_binding(condition_array, body_array)
            || !same_binding(target_read, target_write)
            || same_binding(condition_index, target_write)
        {
            return Ok(None);
        }
        if self.builtin_value(condition_index.name().name())?.is_some()
            || self.builtin_value(condition_array.name().name())?.is_some()
            || self.builtin_value(target_write.name().name())?.is_some()
        {
            return Ok(None);
        }
        let Some(index_cell) = self.get_binding_bytecode(condition_index)? else {
            return Ok(None);
        };
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target_write)? else {
            return Ok(None);
        };
        let Some(array_cell) = self.get_binding_bytecode(condition_array)? else {
            return Ok(None);
        };
        Ok(Some(NumericArrayReductionPlan {
            index: condition_index,
            index_cell,
            target: target_write,
            target_cell,
            array: condition_array,
            array_cell,
        }))
    }

    pub(in crate::runtime::bytecode) fn eval_numeric_array_reduction_plan(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
        plan: &NumericArrayReductionPlan<'_>,
    ) -> Result<bool> {
        let Value::Number(index) = plan.index_cell.value(plan.index.name())? else {
            return Ok(false);
        };
        let Value::Number(mut total) = plan.target_cell.value(plan.target.name())? else {
            return Ok(false);
        };
        let Value::Object(array_id) = plan.array_cell.value(plan.array.name())? else {
            return Ok(false);
        };
        let Some(values) = self
            .objects
            .packed_default_array_values_if_array(array_id)?
        else {
            return Ok(false);
        };
        let Some(mut index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        if index > values.len() {
            return Ok(false);
        }
        let mut last = Value::Undefined;
        while let Some(value) = values.get(index) {
            let Value::Number(number) = value else {
                return Ok(false);
            };
            if let Err(error) = self.charge_runtime_steps(ITERATION_STEP_COST) {
                self.store_numeric_array_reduction_state(plan, index, total)?;
                return Err(error);
            }
            total += number;
            index = index.saturating_add(1);
            last = self.checked_value(Value::Number(total))?;
            self.record_bytecode_linear_direct_runs(3)?;
        }
        self.charge_runtime_steps(CONDITION_STEP_COST)?;
        self.store_numeric_array_reduction_state(plan, index, total)?;
        state.last = last;
        state.pc = next;
        Ok(true)
    }

    fn store_numeric_array_reduction_state(
        &self,
        plan: &NumericArrayReductionPlan<'_>,
        index: usize,
        total: f64,
    ) -> Result<()> {
        let index = self.checked_value(Value::Number(usize_to_f64(index)?))?;
        plan.index_cell.assign(plan.index.name(), index)?;
        let total = self.checked_value(Value::Number(total))?;
        plan.target_cell.assign(plan.target.name(), total)
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

fn same_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "numeric array reduction index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value = u32::try_from(value)
        .map_err(|_| crate::Error::runtime("numeric array reduction index is too large"))?;
    Ok(f64::from(value))
}
