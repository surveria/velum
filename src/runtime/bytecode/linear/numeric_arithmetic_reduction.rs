use alloc::{boxed::Box, vec::Vec};

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

use super::{super::state::BytecodeState, apply_number_binary};

const LOOP_CONTROL_STEP_COST: usize = 1;
const MAX_NUMERIC_EXPRESSION_INSTRUCTIONS: usize = 32;

#[derive(Debug)]
pub(in crate::runtime::bytecode) struct NumericArithmeticReductionPlan<'a> {
    index: &'a BytecodeBinding,
    index_cell: BindingCell,
    limit: usize,
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    expression: NumericLoopExpression,
    condition_step_cost: usize,
    iteration_step_cost: usize,
}

#[derive(Debug)]
enum NumericLoopExpression {
    Index,
    Target,
    Literal(f64),
    Binary {
        op: BytecodeNumericBinaryOp,
        left: Box<Self>,
        right: Box<Self>,
    },
}

#[derive(Debug, Clone, Copy, Default)]
struct NumericLoopSources {
    index: bool,
    target: bool,
}

impl NumericLoopSources {
    const fn merge(self, other: Self) -> Self {
        Self {
            index: self.index || other.index,
            target: self.target || other.target,
        }
    }

    const fn is_recurrence(self) -> bool {
        self.index && self.target
    }
}

impl NumericLoopExpression {
    fn compile(
        instructions: &[BytecodeInstruction],
        index: &BytecodeBinding,
        target: &BytecodeBinding,
    ) -> Option<Self> {
        if instructions.is_empty() || instructions.len() > MAX_NUMERIC_EXPRESSION_INSTRUCTIONS {
            return None;
        }
        let mut stack = Vec::new();
        for instruction in instructions {
            match instruction {
                BytecodeInstruction::LoadBinding(binding) if same_binding(binding, index) => {
                    stack.push(Self::Index);
                }
                BytecodeInstruction::LoadBinding(binding) if same_binding(binding, target) => {
                    stack.push(Self::Target);
                }
                BytecodeInstruction::PushLiteral(Value::Number(value)) => {
                    stack.push(Self::Literal(*value));
                }
                BytecodeInstruction::NumberBinary(op) => {
                    let right = stack.pop()?;
                    let left = stack.pop()?;
                    stack.push(Self::Binary {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                    });
                }
                _ => return None,
            }
        }
        let expression = stack.pop()?;
        if !stack.is_empty() || !expression.sources().is_recurrence() {
            return None;
        }
        Some(expression)
    }

    fn sources(&self) -> NumericLoopSources {
        match self {
            Self::Index => NumericLoopSources {
                index: true,
                target: false,
            },
            Self::Target => NumericLoopSources {
                index: false,
                target: true,
            },
            Self::Literal(_) => NumericLoopSources::default(),
            Self::Binary { left, right, .. } => left.sources().merge(right.sources()),
        }
    }

    fn evaluate(&self, index: f64, target: f64) -> Result<f64> {
        match self {
            Self::Index => Ok(index),
            Self::Target => Ok(target),
            Self::Literal(value) => Ok(*value),
            Self::Binary { op, left, right } => apply_number_binary(
                *op,
                left.evaluate(index, target)?,
                right.evaluate(index, target)?,
            ),
        }
    }
}

impl Context {
    pub(super) fn bind_numeric_arithmetic_reduction_plan<'a>(
        &mut self,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
    ) -> Result<Option<NumericArithmeticReductionPlan<'a>>> {
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
        let Some((BytecodeInstruction::StoreLast, body_prefix)) = body.instructions().split_last()
        else {
            return Ok(None);
        };
        let Some((BytecodeInstruction::StoreBinding(target), expression_instructions)) =
            body_prefix.split_last()
        else {
            return Ok(None);
        };
        if !same_binding(condition_index, update_index)
            || same_binding(condition_index, target)
            || binding_can_use_dynamic_environment(condition_index)
            || binding_can_use_dynamic_environment(target)
            || self.builtin_value(condition_index.name().name())?.is_some()
            || self.builtin_value(target.name().name())?.is_some()
        {
            return Ok(None);
        }
        let Some(expression) =
            NumericLoopExpression::compile(expression_instructions, condition_index, target)
        else {
            return Ok(None);
        };
        let Some(index_cell) = self.get_binding_bytecode(condition_index)? else {
            return Ok(None);
        };
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target)? else {
            return Ok(None);
        };
        if !index_cell.kind().is_mutable() || !target_cell.kind().is_mutable() {
            return Ok(None);
        }
        let condition_step_cost = condition.instructions().len();
        let iteration_step_cost = condition_step_cost
            .checked_add(body.instructions().len())
            .and_then(|cost| cost.checked_add(update.instructions().len()))
            .and_then(|cost| cost.checked_add(LOOP_CONTROL_STEP_COST))
            .ok_or_else(|| {
                crate::Error::limit("numeric arithmetic reduction step cost overflowed")
            })?;
        Ok(Some(NumericArithmeticReductionPlan {
            index: condition_index,
            index_cell,
            limit,
            target,
            target_cell,
            expression,
            condition_step_cost,
            iteration_step_cost,
        }))
    }

    pub(super) fn eval_numeric_arithmetic_reduction_plan(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
        plan: &NumericArithmeticReductionPlan<'_>,
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
        let Value::Number(mut target) = plan.target_cell.value(plan.target.name())? else {
            return Ok(false);
        };
        let mut last = Value::Undefined;
        while index < plan.limit {
            if let Err(error) = self.charge_runtime_steps(plan.iteration_step_cost) {
                self.store_numeric_arithmetic_reduction_state(plan, index, target)?;
                return Err(error);
            }
            let index_number = usize_to_f64(index)?;
            target = plan.expression.evaluate(index_number, target)?;
            index = index.saturating_add(1);
            last = Value::Number(target);
            self.record_bytecode_linear_direct_runs(3)?;
        }
        self.charge_runtime_steps(plan.condition_step_cost)?;
        self.store_numeric_arithmetic_reduction_state(plan, index, target)?;
        state.last = self.checked_value(last)?;
        state.pc = next;
        Ok(true)
    }

    fn store_numeric_arithmetic_reduction_state(
        &self,
        plan: &NumericArithmeticReductionPlan<'_>,
        index: usize,
        target: f64,
    ) -> Result<()> {
        let index = self.checked_value(Value::Number(usize_to_f64(index)?))?;
        plan.index_cell.assign(plan.index.name(), index)?;
        let target = self.checked_value(Value::Number(target))?;
        plan.target_cell.assign(plan.target.name(), target)
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
    let Ok(index) = number_to_i32(value, "numeric arithmetic reduction index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value = u32::try_from(value)
        .map_err(|_| crate::Error::runtime("numeric arithmetic reduction index is too large"))?;
    Ok(f64::from(value))
}
