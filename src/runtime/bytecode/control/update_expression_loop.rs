use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
        BytecodeProperty,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    syntax::{StaticName, UpdateOp},
    value::Value,
};

use super::{for_loop::BytecodeForLoopFastPath, loop_helpers::same_bytecode_binding};

#[derive(Debug)]
pub(super) struct BytecodeUpdateExpressionLoopFastPath<'a> {
    total: &'a BytecodeBinding,
    total_cell: BindingCell,
    record: &'a BytecodeBinding,
    record_cell: BindingCell,
    value_property: &'a StaticName,
    values: &'a BytecodeBinding,
    values_cell: BindingCell,
    index: &'a BytecodeBinding,
    array_mask_i32: i32,
    test_mask_i32: i32,
    test_right_i32: i32,
}

impl Context {
    pub(super) fn compile_update_expression_loop_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeUpdateExpressionLoopFastPath<'a>>> {
        let [
            BytecodeInstruction::UpdateBinding {
                name: total,
                op: UpdateOp::Increment,
                prefix: false,
            },
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::LoadBinding(record_inc),
            BytecodeInstruction::UpdateStaticProperty {
                property: value_inc,
                op: UpdateOp::Increment,
                prefix: false,
            },
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::LoadBinding(values),
            BytecodeInstruction::LoadBinding(array_index),
            BytecodeInstruction::PushLiteral(Value::Number(array_mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::UpdateComputedProperty {
                property: _,
                op: UpdateOp::Increment,
                prefix: true,
            },
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::LoadBinding(test_index),
            BytecodeInstruction::PushLiteral(Value::Number(test_mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::PushLiteral(Value::Number(test_right)),
            BytecodeInstruction::NumberEquality(BytecodeNumericEqualityOp::StrictEqual),
            BytecodeInstruction::JumpIfFalse(alternate),
            BytecodeInstruction::LoadBinding(record_dec),
            BytecodeInstruction::UpdateStaticProperty {
                property: value_dec,
                op: UpdateOp::Decrement,
                prefix: true,
            },
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::Jump(end),
            BytecodeInstruction::PushUndefined,
            BytecodeInstruction::StoreLast,
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if alternate.index() != 21
            || end.index() != 23
            || !same_bytecode_binding(index, array_index)
            || !same_bytecode_binding(index, test_index)
            || !same_bytecode_binding(record_inc, record_dec)
            || !same_bytecode_property(value_inc, value_dec)
        {
            return Ok(None);
        }
        let Ok(array_mask_i32) = number_to_i32(*array_mask, "update expression array mask") else {
            return Ok(None);
        };
        let Ok(test_mask_i32) = number_to_i32(*test_mask, "update expression test mask") else {
            return Ok(None);
        };
        let Ok(test_right_i32) = number_to_i32(*test_right, "update expression test value") else {
            return Ok(None);
        };
        let Some(total_cell) = self.get_binding_bytecode(total)? else {
            return Ok(None);
        };
        let Some(record_cell) = self.get_binding_bytecode(record_inc)? else {
            return Ok(None);
        };
        let Some(values_cell) = self.get_binding_bytecode(values)? else {
            return Ok(None);
        };
        if self.builtin_value(total.name().name())?.is_some()
            || self.builtin_value(record_inc.name().name())?.is_some()
            || self.builtin_value(values.name().name())?.is_some()
        {
            return Ok(None);
        }
        Ok(Some(BytecodeUpdateExpressionLoopFastPath {
            total,
            total_cell,
            record: record_inc,
            record_cell,
            value_property: value_inc.name(),
            values,
            values_cell,
            index,
            array_mask_i32,
            test_mask_i32,
            test_right_i32,
        }))
    }

    pub(super) fn update_expression_loop_fast_path_ready(
        body: &BytecodeUpdateExpressionLoopFastPath<'_>,
    ) -> Result<bool> {
        Ok(matches!(
            body.total_cell.value(body.total.name())?,
            Value::Number(_)
        ))
    }

    pub(super) fn eval_update_expression_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeUpdateExpressionLoopFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(fast_path.compare, BytecodeNumericCompareOp::Less)
            || fast_path.update_step.to_bits() != 1.0f64.to_bits()
            || !same_bytecode_binding(fast_path.index, body.index)
        {
            return Ok(false);
        }
        let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(false);
        };
        let limit = self.fast_loop_limit(fast_path)?;
        let Some(mut index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        let Some(limit) = non_negative_integer_index(limit) else {
            return Ok(false);
        };
        let Value::Number(mut total) = body.total_cell.value(body.total.name())? else {
            return Ok(false);
        };
        let Value::Object(record_id) = body.record_cell.value(body.record.name())? else {
            return Ok(false);
        };
        let Some(record_key) = self.lookup_static_name_atom(body.value_property)? else {
            return Ok(false);
        };
        let record_lookup = crate::runtime::object::PropertyLookup::from_key(
            body.value_property.as_str(),
            crate::runtime::object::PropertyKey::new(record_key),
        );
        let Some(Value::Number(mut record_value)) = self
            .objects
            .own_data_property_value(record_id, record_lookup)?
        else {
            return Ok(false);
        };
        let Value::Object(values_id) = body.values_cell.value(body.values.name())? else {
            return Ok(false);
        };
        let Some(mut values) = self
            .objects
            .packed_numeric_array_values_if_array(values_id)?
        else {
            return Ok(false);
        };
        let mut last = Value::Undefined;
        while index < limit {
            self.step()?;
            self.record_bytecode_linear_direct_run()?;
            total += 1.0;
            record_value += 1.0;
            let array_index = masked_index(index, body.array_mask_i32)?;
            let Some(value) = values.get_mut(array_index) else {
                return Ok(false);
            };
            *value += 1.0;
            if (number_to_i32(usize_to_f64(index)?, "update expression test")? & body.test_mask_i32)
                == body.test_right_i32
            {
                record_value -= 1.0;
                last = self.checked_value(Value::Number(record_value))?;
            } else {
                last = Value::Undefined;
            }
            index = index.saturating_add(1);
        }
        let total_value = self.checked_value(Value::Number(total))?;
        let record_value = self.checked_value(Value::Number(record_value))?;
        if !self
            .objects
            .set_own_data_property_value(record_id, record_lookup, record_value)?
        {
            return Ok(false);
        }
        if !self.objects.set_packed_numeric_array_values_if_array(
            values_id,
            &values,
            self.limits.max_object_properties,
        )? {
            return Ok(false);
        }
        self.assign_fast_path_cell(body.total, &body.total_cell, total_value)?;
        let index_value = self.checked_value(Value::Number(usize_to_f64(index)?))?;
        self.assign_fast_path_cell(fast_path.index, &fast_path.index_cell, index_value)?;
        state.last = last;
        state.pc = next;
        Ok(true)
    }
}

fn same_bytecode_property(left: &BytecodeProperty, right: &BytecodeProperty) -> bool {
    left.name().as_str() == right.name().as_str()
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "update expression loop index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value = u32::try_from(value)
        .map_err(|_| crate::error::Error::limit("update expression index exceeds f64 range"))?;
    Ok(f64::from(value))
}

fn masked_index(index: usize, mask: i32) -> Result<usize> {
    let index = number_to_i32(usize_to_f64(index)?, "update expression array index")?;
    usize::try_from(index & mask)
        .map_err(|_| crate::error::Error::runtime("update expression array index is negative"))
}
