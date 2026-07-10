use crate::{
    bytecode::BytecodeBlock,
    error::Result,
    runtime::{
        Context,
        control::{Completion, runtime_exception_value},
    },
    value::Value,
};

use super::{BytecodeLinearOp, BytecodeLinearPlan};

impl Context {
    pub(in crate::runtime::bytecode) fn eval_bytecode_linear_direct_condition(
        &mut self,
        block: &BytecodeBlock,
        plan: Option<&BytecodeLinearPlan<'_>>,
    ) -> Result<Option<Completion>> {
        let Some(op) = single_full_block_op(block, plan) else {
            return Ok(None);
        };
        if !matches!(
            op,
            BytecodeLinearOp::CompareBindingNumber { .. }
                | BytecodeLinearOp::InStaticPropertyBinding {
                    store_last: true,
                    ..
                }
        ) {
            return Ok(None);
        }
        self.eval_bytecode_linear_direct_completion(op)
    }

    pub(super) fn eval_bytecode_linear_direct_expression(
        &mut self,
        block: &BytecodeBlock,
        plan: Option<&BytecodeLinearPlan<'_>>,
    ) -> Result<Option<Value>> {
        let Some(op) = single_full_block_op(block, plan) else {
            return Ok(None);
        };
        if !matches!(
            op,
            BytecodeLinearOp::StoreBindingFromBindingNumberBinary { .. }
                | BytecodeLinearOp::UpdateBindingStoreLast { .. }
        ) {
            return Ok(None);
        }
        self.eval_bytecode_linear_direct_completion(op)?
            .map(Completion::into_result)
            .transpose()
    }

    fn eval_bytecode_linear_direct_completion(
        &mut self,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<Option<Completion>> {
        self.record_bytecode_linear_direct_run()?;
        self.step()?;
        match self.eval_bytecode_linear_direct_value(op) {
            Ok(Some(value)) => Ok(Some(Completion::Normal(value))),
            Ok(None) => Ok(None),
            Err(error) => {
                if let Some(value) = runtime_exception_value(self, &error)? {
                    self.checked_value(value.clone())?;
                    return Ok(Some(Completion::Throw(value)));
                }
                Err(error)
            }
        }
    }

    fn eval_bytecode_linear_direct_value(
        &mut self,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<Option<Value>> {
        match op {
            BytecodeLinearOp::CompareBindingNumber {
                binding,
                cell,
                op,
                right,
            } => {
                let left = self.runtime_value(cell.value(binding.name())?)?;
                self.eval_bytecode_number_compare(*op, &left, &Value::Number(*right))
                    .map(Some)
            }
            BytecodeLinearOp::StoreBindingFromBindingNumberBinary {
                source,
                source_cell,
                target,
                target_cell,
                op,
                right,
            } => {
                let left = self.runtime_value(source_cell.value(source.name())?)?;
                let value = self.eval_bytecode_number_binary(*op, &left, &Value::Number(*right))?;
                self.assign_bytecode_cell(target, target_cell, value.clone())?;
                Ok(Some(value))
            }
            BytecodeLinearOp::UpdateBindingStoreLast {
                binding,
                cell,
                op,
                prefix,
            } => {
                let old_value = cell.value(binding.name())?;
                let new_value = Self::updated_bytecode_number(&old_value, *op)?;
                self.checked_value(new_value.clone())?;
                self.assign_bytecode_cell(binding, cell, new_value.clone())?;
                Ok(Some(if *prefix { new_value } else { old_value }))
            }
            BytecodeLinearOp::InStaticPropertyBinding {
                binding,
                cell,
                property,
                access,
                store_last: true,
            } => {
                let object = self.checked_value(cell.value(binding.name())?)?;
                self.eval_bytecode_in_static_property(&object, property, *access)
                    .map(Some)
            }
            BytecodeLinearOp::PushLiteral(_)
            | BytecodeLinearOp::PushUndefined
            | BytecodeLinearOp::LoadBinding { .. }
            | BytecodeLinearOp::StoreBinding { .. }
            | BytecodeLinearOp::DeclareVarBinding { .. }
            | BytecodeLinearOp::StoreLast
            | BytecodeLinearOp::Pop
            | BytecodeLinearOp::UpdateBinding { .. }
            | BytecodeLinearOp::NumberBinary(_)
            | BytecodeLinearOp::NumberCompare(_)
            | BytecodeLinearOp::NumberEquality(_)
            | BytecodeLinearOp::CompoundStoreBinding { .. }
            | BytecodeLinearOp::DeclareVarFromBindingNumberBinary { .. }
            | BytecodeLinearOp::AddArrayElementToBinding { .. }
            | BytecodeLinearOp::InStaticPropertyBinding { .. }
            | BytecodeLinearOp::InArrayIndexMaskBinding { .. }
            | BytecodeLinearOp::NumericBindingChain(_)
            | BytecodeLinearOp::NumericCompoundBinding(_)
            | BytecodeLinearOp::NumericCompoundChain(_)
            | BytecodeLinearOp::PropertyMutation(_)
            | BytecodeLinearOp::ArrayLength(_)
            | BytecodeLinearOp::ArrayIndexMember { .. }
            | BytecodeLinearOp::ComputedMember(_) => Ok(None),
        }
    }
}

fn single_full_block_op<'plan, 'bytecode>(
    block: &BytecodeBlock,
    plan: Option<&'plan BytecodeLinearPlan<'bytecode>>,
) -> Option<&'plan BytecodeLinearOp<'bytecode>> {
    plan?.single_full_block_op(block)
}
