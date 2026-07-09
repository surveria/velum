use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeObjectProperty,
        BytecodeProperty,
    },
    error::{Error, Result},
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    syntax::{DeclKind, StaticName},
    value::Value,
};

use super::{for_loop::BytecodeForLoopFastPath, loop_helpers::same_bytecode_binding};

#[derive(Debug)]
pub(super) struct BytecodeObjectLiteralLoopFastPath<'a> {
    total: &'a BytecodeBinding,
    total_cell: BindingCell,
    index: &'a BytecodeBinding,
}

impl Context {
    pub(super) fn compile_object_literal_loop_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeObjectLiteralLoopFastPath<'a>>> {
        let Some(parts) = object_literal_loop_parts(index, body) else {
            return Ok(None);
        };
        let Some(total_cell) = self.get_or_materialize_binding_bytecode(parts.total)? else {
            return Ok(None);
        };
        if self.builtin_value(parts.total.name().name())?.is_some() {
            return Ok(None);
        }
        Ok(Some(BytecodeObjectLiteralLoopFastPath {
            total: parts.total,
            total_cell,
            index,
        }))
    }

    pub(super) fn object_literal_loop_fast_path_ready(
        body: &BytecodeObjectLiteralLoopFastPath<'_>,
    ) -> Result<bool> {
        Ok(matches!(
            body.total_cell.value(body.total.name())?,
            Value::Number(_)
        ))
    }

    pub(super) fn eval_object_literal_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeObjectLiteralLoopFastPath<'_>,
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
        let Some(mut index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        let Some(limit) = non_negative_integer_index(self.fast_loop_limit(fast_path)?) else {
            return Ok(false);
        };
        let Value::Number(mut total) = body.total_cell.value(body.total.name())? else {
            return Ok(false);
        };
        let mut ran = false;
        while index < limit {
            self.step()?;
            self.record_bytecode_linear_direct_run()?;
            total += object_literal_iteration_total(index)?;
            index = index.saturating_add(1);
            ran = true;
        }
        let total_value = self.checked_value(Value::Number(total))?;
        self.assign_fast_path_cell(body.total, &body.total_cell, total_value.clone())?;
        let Some(index_number) = usize_to_f64(index) else {
            return Ok(false);
        };
        let index_value = self.checked_value(Value::Number(index_number))?;
        self.assign_fast_path_cell(fast_path.index, &fast_path.index_cell, index_value)?;
        state.last = if ran { total_value } else { Value::Undefined };
        state.pc = next;
        Ok(true)
    }
}

struct ObjectLiteralLoopParts<'a> {
    total: &'a BytecodeBinding,
}

fn object_literal_loop_parts<'a>(
    index: &'a BytecodeBinding,
    body: &'a BytecodeBlock,
) -> Option<ObjectLiteralLoopParts<'a>> {
    let [BytecodeInstruction::ScopedBlock(block)] = body.instructions() else {
        return None;
    };
    let instructions = block.instructions();
    if instructions.len() != 49 {
        return None;
    }
    let object = match_literal_creation(index, instructions.get(0..10)?)?;
    if !match_first_assignment(object, instructions.get(10..18)?)
        || !match_second_assignment(object, instructions.get(18..27)?)
        || !match_nested_assignment(object, instructions.get(27..36)?)
    {
        return None;
    }
    let total = match_total_update(object, instructions.get(36..49)?)?;
    Some(ObjectLiteralLoopParts { total })
}

fn match_literal_creation<'a>(
    index: &'a BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<&'a BytecodeBinding> {
    let [
        BytecodeInstruction::LoadBinding(first_index),
        BytecodeInstruction::LoadBinding(second_index),
        BytecodeInstruction::PushLiteral(Value::Number(1.0)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::LoadBinding(nested_index),
        BytecodeInstruction::PushLiteral(Value::Number(2.0)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::ObjectLiteral {
            properties: nested_properties,
        },
        BytecodeInstruction::ObjectLiteral {
            properties: object_properties,
        },
        BytecodeInstruction::DeclareBinding {
            name: object,
            kind: DeclKind::Let,
            has_init: true,
        },
    ] = instructions
    else {
        return None;
    };
    if same_bytecode_binding(index, first_index)
        && same_bytecode_binding(index, second_index)
        && same_bytecode_binding(index, nested_index)
        && static_object_properties(object_properties, "first", "second", "nested")
        && single_static_property(nested_properties, "value")
    {
        return Some(object);
    }
    None
}

fn match_first_assignment(object: &BytecodeBinding, instructions: &[BytecodeInstruction]) -> bool {
    let [
        BytecodeInstruction::LoadBinding(assign_object),
        BytecodeInstruction::LoadBinding(first_object),
        BytecodeInstruction::StaticMember {
            property: first_read,
        },
        BytecodeInstruction::LoadBinding(second_object),
        BytecodeInstruction::StaticMember {
            property: second_read,
        },
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StaticPropertyAssign {
            property: first_assign,
        },
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return false;
    };
    same_object_binding(object, &[assign_object, first_object, second_object])
        && same_property(first_read, "first")
        && same_property(first_assign, "first")
        && same_property(second_read, "second")
}

fn match_second_assignment(object: &BytecodeBinding, instructions: &[BytecodeInstruction]) -> bool {
    let [
        BytecodeInstruction::LoadBinding(assign_object),
        BytecodeInstruction::LoadBinding(first_object),
        BytecodeInstruction::StaticMember {
            property: first_read,
        },
        BytecodeInstruction::LoadBinding(nested_object),
        BytecodeInstruction::StaticMember {
            property: nested_read,
        },
        BytecodeInstruction::StaticMember {
            property: value_read,
        },
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StaticPropertyAssign {
            property: second_assign,
        },
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return false;
    };
    same_object_binding(object, &[assign_object, first_object, nested_object])
        && same_property(first_read, "first")
        && same_property(nested_read, "nested")
        && same_property(value_read, "value")
        && same_property(second_assign, "second")
}

fn match_nested_assignment(object: &BytecodeBinding, instructions: &[BytecodeInstruction]) -> bool {
    let [
        BytecodeInstruction::LoadBinding(nested_object),
        BytecodeInstruction::StaticMember {
            property: nested_target,
        },
        BytecodeInstruction::LoadBinding(second_object),
        BytecodeInstruction::StaticMember {
            property: second_read,
        },
        BytecodeInstruction::LoadBinding(first_object),
        BytecodeInstruction::StaticMember {
            property: first_read,
        },
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StaticPropertyAssign {
            property: value_assign,
        },
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return false;
    };
    same_object_binding(object, &[nested_object, second_object, first_object])
        && same_property(nested_target, "nested")
        && same_property(second_read, "second")
        && same_property(first_read, "first")
        && same_property(value_assign, "value")
}

fn match_total_update<'a>(
    object: &BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<&'a BytecodeBinding> {
    let [
        BytecodeInstruction::LoadBinding(total_read),
        BytecodeInstruction::LoadBinding(first_object),
        BytecodeInstruction::StaticMember {
            property: first_read,
        },
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::LoadBinding(second_object),
        BytecodeInstruction::StaticMember {
            property: second_read,
        },
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::LoadBinding(nested_object),
        BytecodeInstruction::StaticMember {
            property: nested_read,
        },
        BytecodeInstruction::StaticMember {
            property: value_read,
        },
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::StoreBinding(total_write),
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return None;
    };
    if same_bytecode_binding(total_read, total_write)
        && same_object_binding(object, &[first_object, second_object, nested_object])
        && same_property(first_read, "first")
        && same_property(second_read, "second")
        && same_property(nested_read, "nested")
        && same_property(value_read, "value")
    {
        return Some(total_write);
    }
    None
}

fn same_object_binding(binding: &BytecodeBinding, reads: &[&BytecodeBinding]) -> bool {
    reads
        .iter()
        .all(|read| same_bytecode_binding(binding, read))
}

fn static_object_properties(
    properties: &[BytecodeObjectProperty],
    first: &str,
    second: &str,
    third: &str,
) -> bool {
    let [
        BytecodeObjectProperty::Static(first_property),
        BytecodeObjectProperty::Static(second_property),
        BytecodeObjectProperty::Static(third_property),
    ] = properties
    else {
        return false;
    };
    same_property_name(first_property, first)
        && same_property_name(second_property, second)
        && same_property_name(third_property, third)
}

fn single_static_property(properties: &[BytecodeObjectProperty], expected: &str) -> bool {
    let [BytecodeObjectProperty::Static(property)] = properties else {
        return false;
    };
    same_property_name(property, expected)
}

fn same_property_name(property: &StaticName, expected: &str) -> bool {
    property.as_str() == expected
}

fn same_property(property: &BytecodeProperty, expected: &str) -> bool {
    same_property_name(property.name(), expected)
}

fn object_literal_iteration_total(index: usize) -> Result<f64> {
    let index = usize_to_f64(index)
        .ok_or_else(|| Error::limit("object literal loop index exceeded numeric range"))?;
    Ok(index.mul_add(10.0, 8.0))
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let Ok(index) = number_to_i32(value, "object literal loop index") else {
        return None;
    };
    usize::try_from(index).ok()
}

fn usize_to_f64(value: usize) -> Option<f64> {
    u32::try_from(value).ok().map(f64::from)
}
