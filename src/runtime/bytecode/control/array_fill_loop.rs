use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    value::Value,
};

use super::for_loop::{BytecodeForLoopFastPath, fast_loop_compare, same_bytecode_binding};

#[derive(Debug)]
pub(super) struct BytecodeForArrayFillFastPath<'a> {
    seed: &'a BytecodeBinding,
    seed_cell: BindingCell,
    pub(super) array: &'a BytecodeBinding,
    array_cell: BindingCell,
    pub(super) index: &'a BytecodeBinding,
    multiplier: f64,
    increment: f64,
    state_modulus: f64,
    element_modulus: f64,
}

impl Context {
    pub(super) fn compile_bytecode_for_array_fill_fast_path<'a>(
        &mut self,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeForArrayFillFastPath<'a>>> {
        let [
            BytecodeInstruction::LoadBinding(seed_read),
            BytecodeInstruction::PushLiteral(Value::Number(multiplier)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Mul),
            BytecodeInstruction::PushLiteral(Value::Number(increment)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::PushLiteral(Value::Number(state_modulus)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Rem),
            BytecodeInstruction::StoreBinding(seed_write),
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::LoadBinding(array),
            BytecodeInstruction::LoadBinding(index),
            BytecodeInstruction::LoadBinding(seed_element_read),
            BytecodeInstruction::PushLiteral(Value::Number(element_modulus)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Rem),
            BytecodeInstruction::ComputedPropertyAssign { .. },
            BytecodeInstruction::StoreLast,
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if !same_bytecode_binding(seed_read, seed_write)
            || !same_bytecode_binding(seed_write, seed_element_read)
        {
            return Ok(None);
        }
        if self.builtin_value(seed_write.name().name())?.is_some() {
            return Ok(None);
        }
        let Some(seed_cell) = self.get_binding_bytecode(seed_write)? else {
            return Ok(None);
        };
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeForArrayFillFastPath {
            seed: seed_write,
            seed_cell,
            array,
            array_cell,
            index,
            multiplier: *multiplier,
            increment: *increment,
            state_modulus: *state_modulus,
            element_modulus: *element_modulus,
        }))
    }

    pub(super) fn bytecode_for_array_fill_fast_path_ready(
        body: &BytecodeForArrayFillFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(body.seed_cell.value(body.seed.name())?, Value::Number(_)) {
            return Ok(false);
        }
        Ok(matches!(
            body.array_cell.value(body.array.name())?,
            Value::Object(_)
        ))
    }

    pub(super) fn eval_bytecode_for_array_fill_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeForArrayFillFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(fast_path.compare, BytecodeNumericCompareOp::Less)
            || fast_path.update_step.to_bits() != 1.0f64.to_bits()
        {
            return Ok(false);
        }
        let Value::Number(mut index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(false);
        };
        let Value::Number(mut seed) = body.seed_cell.value(body.seed.name())? else {
            return Ok(false);
        };
        let Value::Object(array_id) = body.array_cell.value(body.array.name())? else {
            return Ok(false);
        };
        let limit = self.fast_loop_limit(fast_path)?;
        let Some(start) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        let mut values = Vec::new();
        while fast_loop_compare(fast_path.compare, index, limit) {
            self.step()?;
            self.record_bytecode_linear_direct_run()?;
            let product = seed * body.multiplier;
            seed = (product + body.increment) % body.state_modulus;
            values.push(seed % body.element_modulus);
            index += fast_path.update_step;
        }
        let Some(last) = values.last().copied() else {
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(true);
        };
        if !self.objects.append_packed_default_numbers_if_array(
            array_id,
            start,
            &values,
            self.limits.max_object_properties,
        )? {
            return Ok(false);
        }
        let seed_value = self.checked_value(Value::Number(seed))?;
        self.assign_fast_path_cell(body.seed, &body.seed_cell, seed_value)?;
        let index_value = self.checked_value(Value::Number(index))?;
        self.assign_fast_path_cell(fast_path.index, &fast_path.index_cell, index_value)?;
        state.last = self.checked_value(Value::Number(last))?;
        state.pc = next;
        Ok(true)
    }
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "array fill index") else {
        return None;
    };
    usize::try_from(index).ok()
}
