#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    api::native_call::NativeCallTarget,
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericArrayReductionRole, BytecodeNumericBinaryOp, BytecodeNumericCompareOp,
        BytecodePreparedNativeCall, BytecodeProperty,
    },
    error::Result,
    runtime::{
        Context,
        binding::scope::BindingCell,
        numeric::number_to_i32,
        object::{CacheableNativePropertyValue, CacheablePropertyValue},
    },
    syntax::UpdateOp,
    value::Value,
};

use super::super::state::BytecodeState;

const CONDITION_STEP_COST: usize = 5;
const LITERAL_CONDITION_STEP_COST: usize = 4;
const ARRAY_ITERATION_STEP_COST: usize = 18;
const STRING_ITERATION_STEP_COST: usize = 20;
const PROPERTY_TERM_INSTRUCTION_COUNT: usize = 6;
const UPDATE_AND_LOOP_STEP_COST: usize = 6;
const CHAR_CODE_AT_PROPERTY: &str = "charCodeAt";
const LENGTH_PROPERTY: &str = "length";

#[derive(Debug)]
pub(in crate::runtime::bytecode) enum NumericReductionPlan<'a> {
    Array(NumericArrayReductionPlan<'a>),
    Property(NumericPropertyReductionPlan<'a>),
    String(NumericStringReductionPlan<'a>),
}

#[derive(Debug)]
pub(in crate::runtime::bytecode) struct NumericArrayReductionPlan<'a> {
    index: &'a BytecodeBinding,
    index_cell: BindingCell,
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    array: &'a BytecodeBinding,
    array_cell: BindingCell,
}

#[derive(Debug)]
pub(in crate::runtime::bytecode) struct NumericStringReductionPlan<'a> {
    index: &'a BytecodeBinding,
    index_cell: BindingCell,
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    text: &'a BytecodeBinding,
    text_cell: BindingCell,
    property: &'a BytecodeProperty,
}

#[derive(Debug)]
pub(in crate::runtime::bytecode) struct NumericPropertyReductionPlan<'a> {
    index: &'a BytecodeBinding,
    index_cell: BindingCell,
    limit: usize,
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    terms: Vec<NumericPropertyReductionTerm<'a>>,
    iteration_step_cost: usize,
}

#[derive(Debug)]
struct NumericPropertyReductionTerm<'a> {
    object: &'a BytecodeBinding,
    object_cell: BindingCell,
    property: &'a BytecodeProperty,
}

impl Context {
    pub(in crate::runtime::bytecode) fn bind_numeric_reduction_plan<'a>(
        &mut self,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
    ) -> Result<Option<NumericReductionPlan<'a>>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        if let Some(plan) = self.bind_numeric_array_reduction_plan(condition, update, body)? {
            return Ok(Some(NumericReductionPlan::Array(plan)));
        }
        if let Some(plan) = self.bind_numeric_string_reduction_plan(condition, update, body)? {
            return Ok(Some(NumericReductionPlan::String(plan)));
        }
        self.bind_numeric_property_reduction_plan(condition, update, body)
            .map(|plan| plan.map(NumericReductionPlan::Property))
    }

    fn bind_numeric_array_reduction_plan<'a>(
        &mut self,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
    ) -> Result<Option<NumericArrayReductionPlan<'a>>> {
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
        if !index_cell.kind().is_mutable() || !target_cell.kind().is_mutable() {
            return Ok(None);
        }
        Ok(Some(NumericArrayReductionPlan {
            index: condition_index,
            index_cell,
            target: target_write,
            target_cell,
            array: condition_array,
            array_cell,
        }))
    }

    fn bind_numeric_string_reduction_plan<'a>(
        &mut self,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
    ) -> Result<Option<NumericStringReductionPlan<'a>>> {
        let (Some(condition), Some(update)) = (condition, update) else {
            return Ok(None);
        };
        let [
            BytecodeInstruction::LoadBinding(condition_index),
            BytecodeInstruction::LoadBinding(condition_text),
            BytecodeInstruction::ArrayLength {
                property: length_property,
            },
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
            BytecodeInstruction::LoadBinding(body_text),
            BytecodeInstruction::Duplicate,
            BytecodeInstruction::StaticMember { property },
            BytecodeInstruction::LoadBinding(body_index),
            BytecodeInstruction::CallValueWithReceiver {
                native:
                    Some(BytecodePreparedNativeCall::Direct {
                        target: NativeCallTarget::StringPrototypeCharCodeAt,
                        ..
                    }),
                arg_count: 1,
                ..
            },
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(target_write),
            BytecodeInstruction::StoreLast,
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if length_property.name().as_str() != LENGTH_PROPERTY
            || property.name().as_str() != CHAR_CODE_AT_PROPERTY
            || !same_binding(condition_index, update_index)
            || !same_binding(condition_index, body_index)
            || !same_binding(condition_text, body_text)
            || !same_binding(target_read, target_write)
            || same_binding(condition_index, target_write)
        {
            return Ok(None);
        }
        if self.builtin_value(condition_index.name().name())?.is_some()
            || self.builtin_value(condition_text.name().name())?.is_some()
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
        let Some(text_cell) = self.get_binding_bytecode(condition_text)? else {
            return Ok(None);
        };
        if !index_cell.kind().is_mutable() || !target_cell.kind().is_mutable() {
            return Ok(None);
        }
        Ok(Some(NumericStringReductionPlan {
            index: condition_index,
            index_cell,
            target: target_write,
            target_cell,
            text: condition_text,
            text_cell,
            property,
        }))
    }

    fn bind_numeric_property_reduction_plan<'a>(
        &mut self,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
    ) -> Result<Option<NumericPropertyReductionPlan<'a>>> {
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
        if !same_binding(condition_index, update_index) {
            return Ok(None);
        }
        let mut chunks = body
            .instructions()
            .chunks_exact(PROPERTY_TERM_INSTRUCTION_COUNT);
        if !chunks.remainder().is_empty() {
            return Ok(None);
        }
        let mut target = None;
        let mut terms = Vec::new();
        for chunk in &mut chunks {
            let [
                BytecodeInstruction::LoadBinding(target_read),
                BytecodeInstruction::LoadBinding(object),
                BytecodeInstruction::StaticMember { property },
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
                BytecodeInstruction::StoreBinding(target_write),
                BytecodeInstruction::StoreLast,
            ] = chunk
            else {
                return Ok(None);
            };
            let expected_target = target.get_or_insert(target_write);
            if !same_binding(target_read, target_write)
                || !same_binding(expected_target, target_write)
                || same_binding(condition_index, target_write)
                || self.builtin_value(object.name().name())?.is_some()
            {
                return Ok(None);
            }
            let Some(object_cell) = self.get_binding_bytecode(object)? else {
                return Ok(None);
            };
            terms.push(NumericPropertyReductionTerm {
                object,
                object_cell,
                property,
            });
        }
        if terms.len() < 2 || self.builtin_value(condition_index.name().name())?.is_some() {
            return Ok(None);
        }
        let Some(target) = target else {
            return Ok(None);
        };
        if self.builtin_value(target.name().name())?.is_some() {
            return Ok(None);
        }
        let Some(index_cell) = self.get_binding_bytecode(condition_index)? else {
            return Ok(None);
        };
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target)? else {
            return Ok(None);
        };
        if !index_cell.kind().is_mutable() || !target_cell.kind().is_mutable() {
            return Ok(None);
        }
        let iteration_step_cost = terms
            .len()
            .checked_mul(PROPERTY_TERM_INSTRUCTION_COUNT)
            .and_then(|cost| cost.checked_add(LITERAL_CONDITION_STEP_COST))
            .and_then(|cost| cost.checked_add(UPDATE_AND_LOOP_STEP_COST))
            .ok_or_else(|| {
                crate::Error::limit("numeric property reduction step cost overflowed")
            })?;
        Ok(Some(NumericPropertyReductionPlan {
            index: condition_index,
            index_cell,
            limit,
            target,
            target_cell,
            terms,
            iteration_step_cost,
        }))
    }

    pub(in crate::runtime::bytecode) fn eval_numeric_reduction_plan(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
        plan: &NumericReductionPlan<'_>,
    ) -> Result<bool> {
        match plan {
            NumericReductionPlan::Array(plan) => {
                self.eval_numeric_array_reduction_plan(state, next, plan)
            }
            NumericReductionPlan::Property(plan) => {
                self.eval_numeric_property_reduction_plan(state, next, plan)
            }
            NumericReductionPlan::String(plan) => {
                self.eval_numeric_string_reduction_plan(state, next, plan)
            }
        }
    }

    fn eval_numeric_array_reduction_plan(
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
            if let Err(error) = self.charge_runtime_steps(ARRAY_ITERATION_STEP_COST) {
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

    fn eval_numeric_string_reduction_plan(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
        plan: &NumericStringReductionPlan<'_>,
    ) -> Result<bool> {
        let Value::Number(index) = plan.index_cell.value(plan.index.name())? else {
            return Ok(false);
        };
        let Value::Number(mut total) = plan.target_cell.value(plan.target.name())? else {
            return Ok(false);
        };
        let Value::String(text) = plan.text_cell.value(plan.text.name())? else {
            return Ok(false);
        };
        let Some(mut index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        let units = text.as_utf16();
        if index < units.len() && !self.string_char_code_at_reduction_is_current(plan)? {
            return Ok(false);
        }
        let mut last = Value::Undefined;
        while let Some(unit) = units.get(index) {
            if let Err(error) = self.charge_runtime_steps(STRING_ITERATION_STEP_COST) {
                self.store_numeric_string_reduction_state(plan, index, total)?;
                return Err(error);
            }
            total += f64::from(*unit);
            index = index.saturating_add(1);
            last = self.checked_value(Value::Number(total))?;
            self.record_bytecode_linear_direct_runs(3)?;
        }
        self.charge_runtime_steps(CONDITION_STEP_COST)?;
        self.store_numeric_string_reduction_state(plan, index, total)?;
        state.last = last;
        state.pc = next;
        Ok(true)
    }

    fn eval_numeric_property_reduction_plan(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
        plan: &NumericPropertyReductionPlan<'_>,
    ) -> Result<bool> {
        let Value::Number(index) = plan.index_cell.value(plan.index.name())? else {
            return Ok(false);
        };
        let Value::Number(mut total) = plan.target_cell.value(plan.target.name())? else {
            return Ok(false);
        };
        let Some(mut index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        if index >= plan.limit {
            self.charge_runtime_steps(LITERAL_CONDITION_STEP_COST)?;
            self.store_numeric_property_reduction_state(plan, index, total)?;
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(true);
        }
        let mut values = Vec::with_capacity(plan.terms.len());
        for term in &plan.terms {
            let Value::Object(object) = term.object_cell.value(term.object.name())? else {
                return Ok(false);
            };
            let lookup = self.static_property_lookup(term.property.name())?;
            let candidate = self.objects.cacheable_property_lookup(object, lookup)?;
            let value = self
                .objects
                .read_cacheable_property_value_for(object, candidate)?;
            let CacheablePropertyValue::Hit(Value::Number(value)) = value else {
                return Ok(false);
            };
            values.push(value);
        }
        while index < plan.limit {
            if let Err(error) = self.charge_runtime_steps(plan.iteration_step_cost) {
                self.store_numeric_property_reduction_state(plan, index, total)?;
                return Err(error);
            }
            for value in &values {
                total += value;
            }
            index = index.saturating_add(1);
            self.record_bytecode_linear_direct_runs(3)?;
        }
        self.charge_runtime_steps(LITERAL_CONDITION_STEP_COST)?;
        self.store_numeric_property_reduction_state(plan, index, total)?;
        state.last = self.checked_value(Value::Number(total))?;
        state.pc = next;
        Ok(true)
    }

    fn string_char_code_at_reduction_is_current(
        &mut self,
        plan: &NumericStringReductionPlan<'_>,
    ) -> Result<bool> {
        let prototype = self.string_constructor_prototype()?;
        let lookup = self.static_property_lookup(plan.property.name())?;
        let candidate = self.objects.cacheable_property_lookup(prototype, lookup)?;
        let value = self
            .objects
            .read_cacheable_native_property_value_for(prototype, candidate)?;
        let CacheableNativePropertyValue::Native { function, .. } = value else {
            return Ok(false);
        };
        Ok(self
            .direct_native_call_kind(function, NativeCallTarget::StringPrototypeCharCodeAt)
            .is_some())
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

    fn store_numeric_string_reduction_state(
        &self,
        plan: &NumericStringReductionPlan<'_>,
        index: usize,
        total: f64,
    ) -> Result<()> {
        let index = self.checked_value(Value::Number(usize_to_f64(index)?))?;
        plan.index_cell.assign(plan.index.name(), index)?;
        let total = self.checked_value(Value::Number(total))?;
        plan.target_cell.assign(plan.target.name(), total)
    }

    fn store_numeric_property_reduction_state(
        &self,
        plan: &NumericPropertyReductionPlan<'_>,
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
