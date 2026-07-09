use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericBinaryOp,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    value::Value,
};

use super::{
    for_loop::BytecodeForLoopFastPath,
    loop_helpers::{fast_loop_compare, same_bytecode_binding},
};

#[derive(Debug)]
pub(super) struct BytecodeForArrayAddFastPath<'a> {
    pub(super) target: &'a BytecodeBinding,
    target_cell: BindingCell,
    pub(super) array: &'a BytecodeBinding,
    array_cell: BindingCell,
    pub(super) index: &'a BytecodeBinding,
    index_cell: BindingCell,
}

impl Context {
    pub(super) fn compile_bytecode_for_array_add_fast_path<'a>(
        &mut self,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeForArrayAddFastPath<'a>>> {
        let [
            BytecodeInstruction::LoadBinding(target_read),
            BytecodeInstruction::LoadBinding(array),
            BytecodeInstruction::LoadBinding(index),
            BytecodeInstruction::ComputedMember { .. },
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(target_write),
            BytecodeInstruction::StoreLast,
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if !same_bytecode_binding(target_read, target_write) {
            return Ok(None);
        }
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target_write)? else {
            return Ok(None);
        };
        if self.builtin_value(target_write.name().name())?.is_some() {
            return Ok(None);
        }
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        let Some(index_cell) = self.get_binding_bytecode(index)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeForArrayAddFastPath {
            target: target_write,
            target_cell,
            array,
            array_cell,
            index,
            index_cell,
        }))
    }

    pub(super) fn eval_bytecode_for_array_add_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeForArrayAddFastPath<'_>,
    ) -> Result<bool> {
        let Some(array_values) = self.fast_loop_numeric_array_values_for_simple_add(body)? else {
            return Ok(false);
        };
        let Value::Number(mut index) = body.index_cell.value(body.index.name())? else {
            return Ok(false);
        };
        let Value::Number(mut total) = body.target_cell.value(body.target.name())? else {
            return Ok(false);
        };
        let limit = self.fast_loop_limit(fast_path)?;
        loop {
            self.step()?;
            self.record_bytecode_linear_direct_run()?;
            if !fast_loop_compare(fast_path.compare, index, limit) {
                break;
            }
            let Ok(position) = usize::try_from(number_to_i32(index, "array index")?) else {
                return Ok(false);
            };
            let Some(element) = array_values.get(position).copied() else {
                return Ok(false);
            };
            total += element;
            self.record_bytecode_linear_direct_run()?;
            index += fast_path.update_step;
        }
        let total_value = self.checked_value(Value::Number(total))?;
        self.assign_fast_path_cell(body.target, &body.target_cell, total_value.clone())?;
        let index_value = self.checked_value(Value::Number(index))?;
        self.assign_fast_path_cell(body.index, &body.index_cell, index_value)?;
        state.last = total_value;
        state.pc = next;
        Ok(true)
    }

    pub(super) fn fast_loop_numeric_array_values_for_simple_add(
        &self,
        fast_path: &BytecodeForArrayAddFastPath<'_>,
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

    pub(super) fn eval_bytecode_for_array_add_fast_path(
        &self,
        fast_path: &BytecodeForArrayAddFastPath<'_>,
        array_values: Option<&[f64]>,
    ) -> Result<Value> {
        let Value::Number(left) = fast_path.target_cell.value(fast_path.target.name())? else {
            return Ok(Value::Undefined);
        };
        let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(Value::Undefined);
        };
        let Ok(index) = usize::try_from(number_to_i32(index, "array index")?) else {
            return Ok(Value::Undefined);
        };
        let Some(element) = array_values.and_then(|values| values.get(index).copied()) else {
            return Ok(Value::Undefined);
        };
        let value = self.checked_value(Value::Number(left + element))?;
        self.assign_fast_path_cell(fast_path.target, &fast_path.target_cell, value.clone())?;
        Ok(value)
    }
}
