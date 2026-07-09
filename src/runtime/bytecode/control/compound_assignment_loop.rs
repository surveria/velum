use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
        BytecodeProperty,
    },
    error::{Error, Result},
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    syntax::{BinaryOp, StaticName},
    value::Value,
};

use super::{for_loop::BytecodeForLoopFastPath, loop_helpers::same_bytecode_binding};

#[derive(Debug)]
pub(super) struct BytecodeCompoundAssignmentLoopFastPath<'a> {
    total: &'a BytecodeBinding,
    total_cell: BindingCell,
    total_mask_i32: i32,
    record: &'a BytecodeBinding,
    record_cell: BindingCell,
    count_property: &'a StaticName,
    record_add: f64,
    rhs_mask_i32: i32,
    values: &'a BytecodeBinding,
    values_cell: BindingCell,
    index: &'a BytecodeBinding,
    array_mask_i32: i32,
    test_mask_i32: i32,
    test_right_i32: i32,
    record_sub: f64,
}

struct ParsedCompoundAssignmentLoop<'a> {
    total: &'a BytecodeBinding,
    total_mask_i32: i32,
    record: &'a BytecodeBinding,
    count_property: &'a StaticName,
    record_add: f64,
    rhs_mask_i32: i32,
    values: &'a BytecodeBinding,
    array_mask_i32: i32,
    test_mask_i32: i32,
    test_right_i32: i32,
    record_sub: f64,
}

impl Context {
    pub(super) fn compile_compound_assignment_loop_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeCompoundAssignmentLoopFastPath<'a>>> {
        let Some(parsed) = parse_compound_assignment_loop(index, body) else {
            return Ok(None);
        };
        let Some(total_cell) = self.get_binding_bytecode(parsed.total)? else {
            return Ok(None);
        };
        let Some(record_cell) = self.get_binding_bytecode(parsed.record)? else {
            return Ok(None);
        };
        let Some(values_cell) = self.get_binding_bytecode(parsed.values)? else {
            return Ok(None);
        };
        if self.builtin_value(parsed.total.name().name())?.is_some()
            || self.builtin_value(parsed.record.name().name())?.is_some()
            || self.builtin_value(parsed.values.name().name())?.is_some()
        {
            return Ok(None);
        }
        Ok(Some(BytecodeCompoundAssignmentLoopFastPath {
            total: parsed.total,
            total_cell,
            total_mask_i32: parsed.total_mask_i32,
            record: parsed.record,
            record_cell,
            count_property: parsed.count_property,
            record_add: parsed.record_add,
            rhs_mask_i32: parsed.rhs_mask_i32,
            values: parsed.values,
            values_cell,
            index,
            array_mask_i32: parsed.array_mask_i32,
            test_mask_i32: parsed.test_mask_i32,
            test_right_i32: parsed.test_right_i32,
            record_sub: parsed.record_sub,
        }))
    }

    pub(super) fn compound_assignment_loop_fast_path_ready(
        body: &BytecodeCompoundAssignmentLoopFastPath<'_>,
    ) -> Result<bool> {
        Ok(matches!(
            body.total_cell.value(body.total.name())?,
            Value::Number(_)
        ))
    }

    pub(super) fn eval_compound_assignment_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeCompoundAssignmentLoopFastPath<'_>,
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
        let Some(record_key) = self.lookup_static_name_atom(body.count_property)? else {
            return Ok(false);
        };
        let record_lookup = crate::runtime::object::PropertyLookup::from_key(
            body.count_property.as_str(),
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
            total += masked_index_value(index, body.total_mask_i32)?;
            record_value += body.record_add;
            let array_index = masked_index(index, body.array_mask_i32)?;
            let Some(value) = values.get_mut(array_index) else {
                return Ok(false);
            };
            *value += f64::from(
                number_to_i32(record_value, "compound assignment rhs")? & body.rhs_mask_i32,
            );
            if (number_to_i32(usize_to_f64(index)?, "compound assignment test")?
                & body.test_mask_i32)
                == body.test_right_i32
            {
                record_value -= body.record_sub;
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

fn parse_compound_assignment_loop<'a>(
    index: &'a BytecodeBinding,
    body: &'a BytecodeBlock,
) -> Option<ParsedCompoundAssignmentLoop<'a>> {
    let [
        BytecodeInstruction::LoadBinding(total_index),
        BytecodeInstruction::PushLiteral(Value::Number(total_mask)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
        BytecodeInstruction::CompoundStoreBinding {
            name: total,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::LoadBinding(record_add_object),
        BytecodeInstruction::PushLiteral(Value::Number(record_add)),
        BytecodeInstruction::CompoundStaticProperty {
            property: count_add,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::LoadBinding(values),
        BytecodeInstruction::LoadBinding(array_index),
        BytecodeInstruction::PushLiteral(Value::Number(array_mask)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
        BytecodeInstruction::LoadBinding(rhs_record),
        BytecodeInstruction::StaticMember {
            property: rhs_count,
        },
        BytecodeInstruction::PushLiteral(Value::Number(rhs_mask)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
        BytecodeInstruction::CompoundComputedProperty {
            property: _,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::LoadBinding(test_index),
        BytecodeInstruction::PushLiteral(Value::Number(test_mask)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
        BytecodeInstruction::PushLiteral(Value::Number(test_right)),
        BytecodeInstruction::NumberEquality(BytecodeNumericEqualityOp::StrictEqual),
        BytecodeInstruction::JumpIfFalse(alternate),
        BytecodeInstruction::LoadBinding(record_sub_object),
        BytecodeInstruction::PushLiteral(Value::Number(record_sub)),
        BytecodeInstruction::CompoundStaticProperty {
            property: count_sub,
            op: BinaryOp::Sub,
        },
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::Jump(end),
        BytecodeInstruction::PushUndefined,
        BytecodeInstruction::StoreLast,
    ] = body.instructions()
    else {
        return None;
    };
    if alternate.index() != 30
        || end.index() != 32
        || !same_bytecode_binding(index, total_index)
        || !same_bytecode_binding(index, array_index)
        || !same_bytecode_binding(index, test_index)
        || !same_bytecode_binding(record_add_object, rhs_record)
        || !same_bytecode_binding(record_add_object, record_sub_object)
        || !same_bytecode_property(count_add, rhs_count)
        || !same_bytecode_property(count_add, count_sub)
    {
        return None;
    }
    Some(ParsedCompoundAssignmentLoop {
        total,
        total_mask_i32: number_to_i32(*total_mask, "compound assignment total mask").ok()?,
        record: record_add_object,
        count_property: count_add.name(),
        record_add: *record_add,
        rhs_mask_i32: number_to_i32(*rhs_mask, "compound assignment rhs mask").ok()?,
        values,
        array_mask_i32: number_to_i32(*array_mask, "compound assignment array mask").ok()?,
        test_mask_i32: number_to_i32(*test_mask, "compound assignment test mask").ok()?,
        test_right_i32: number_to_i32(*test_right, "compound assignment test value").ok()?,
        record_sub: *record_sub,
    })
}

fn same_bytecode_property(left: &BytecodeProperty, right: &BytecodeProperty) -> bool {
    left.name().as_str() == right.name().as_str()
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "compound assignment loop index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value = u32::try_from(value)
        .map_err(|_| Error::limit("compound assignment index exceeds f64 range"))?;
    Ok(f64::from(value))
}

fn masked_index(index: usize, mask: i32) -> Result<usize> {
    let index = number_to_i32(usize_to_f64(index)?, "compound assignment array index")?;
    usize::try_from(index & mask)
        .map_err(|_| Error::runtime("compound assignment array index is negative"))
}

fn masked_index_value(index: usize, mask: i32) -> Result<f64> {
    let index = number_to_i32(usize_to_f64(index)?, "compound assignment total index")?;
    Ok(f64::from(index & mask))
}
