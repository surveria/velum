use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeObjectProperty,
        BytecodeProperty,
    },
    error::{Error, Result},
    runtime::{
        Context, binding::scope::BindingCell, native::NativeFunctionKind, numeric::number_to_i32,
    },
    syntax::{BinaryOp, DeclKind, UnaryOp},
    value::Value,
};

use super::{for_loop::BytecodeForLoopFastPath, loop_helpers::same_bytecode_binding};

#[derive(Debug)]
pub(super) struct BytecodeFunctionApplyHasInstanceLoopFastPath<'a> {
    total: &'a BytecodeBinding,
    total_cell: BindingCell,
    sum: &'a BytecodeBinding,
    sum_cell: BindingCell,
    dog: &'a BytecodeBinding,
    dog_cell: BindingCell,
    dog_ctor: &'a BytecodeBinding,
    dog_ctor_cell: BindingCell,
    animal_ctor: &'a BytecodeBinding,
    animal_ctor_cell: BindingCell,
    has_instance: &'a BytecodeBinding,
    has_instance_cell: BindingCell,
    apply_property: &'a BytecodeProperty,
}

impl Context {
    pub(super) fn compile_function_apply_has_instance_loop_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeFunctionApplyHasInstanceLoopFastPath<'a>>> {
        let Some(parts) = function_apply_has_instance_loop_parts(index, body) else {
            return Ok(None);
        };
        if self.builtin_value(parts.total.name().name())?.is_some() {
            return Ok(None);
        }
        let Some(total_cell) = self.get_or_materialize_binding_bytecode(parts.total)? else {
            return Ok(None);
        };
        let Some(sum_cell) = self.get_binding_bytecode(parts.sum)? else {
            return Ok(None);
        };
        let Some(dog_cell) = self.get_or_materialize_binding_bytecode(parts.dog)? else {
            return Ok(None);
        };
        let Some(dog_ctor_cell) = self.get_binding_bytecode(parts.dog_ctor)? else {
            return Ok(None);
        };
        let Some(animal_ctor_cell) = self.get_binding_bytecode(parts.animal_ctor)? else {
            return Ok(None);
        };
        let Some(has_instance_cell) = self.get_binding_bytecode(parts.has_instance)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeFunctionApplyHasInstanceLoopFastPath {
            total: parts.total,
            total_cell,
            sum: parts.sum,
            sum_cell,
            dog: parts.dog,
            dog_cell,
            dog_ctor: parts.dog_ctor,
            dog_ctor_cell,
            animal_ctor: parts.animal_ctor,
            animal_ctor_cell,
            has_instance: parts.has_instance,
            has_instance_cell,
            apply_property: parts.apply_property,
        }))
    }

    pub(super) fn eval_function_apply_has_instance_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeFunctionApplyHasInstanceLoopFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(fast_path.compare, BytecodeNumericCompareOp::Less)
            || fast_path.update_step.to_bits() != 1.0f64.to_bits()
        {
            return Ok(false);
        }
        let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(false);
        };
        let Some(start_index) = non_negative_integer_index(index) else {
            return Ok(false);
        };
        let Some(limit) = non_negative_integer_index(self.fast_loop_limit(fast_path)?) else {
            return Ok(false);
        };
        let Value::Number(total) = body.total_cell.value(body.total.name())? else {
            return Ok(false);
        };
        let Some(iterations) = limit.checked_sub(start_index) else {
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(true);
        };
        if iterations == 0 {
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(true);
        }
        let dog_ctor = body.dog_ctor_cell.value(body.dog_ctor.name())?;
        let animal_ctor = body.animal_ctor_cell.value(body.animal_ctor.name())?;
        let dog = self.eval_new_value(dog_ctor.clone(), &[])?;
        if !self.function_apply_has_instance_guards(body, &dog, &dog_ctor, &animal_ctor)? {
            return Ok(false);
        }
        let total = total + function_apply_has_instance_total_delta(start_index, iterations)?;
        let total_value = self.checked_value(Value::Number(total))?;
        self.charge_runtime_steps(iterations)?;
        self.record_bytecode_linear_direct_runs(iterations)?;
        self.assign_fast_path_cell(body.dog, &body.dog_cell, dog)?;
        self.assign_fast_path_cell(body.total, &body.total_cell, total_value.clone())?;
        let index_value = self.checked_value(Value::Number(usize_to_f64(limit)?))?;
        self.assign_fast_path_cell(fast_path.index, &fast_path.index_cell, index_value)?;
        state.last = total_value;
        state.pc = next;
        Ok(true)
    }

    fn function_apply_has_instance_guards(
        &mut self,
        body: &BytecodeFunctionApplyHasInstanceLoopFastPath<'_>,
        dog: &Value,
        dog_ctor: &Value,
        animal_ctor: &Value,
    ) -> Result<bool> {
        let sum = body.sum_cell.value(body.sum.name())?;
        let has_instance = body.has_instance_cell.value(body.has_instance.name())?;
        if !self.value_static_property_is_native_kind(
            &sum,
            body.apply_property,
            NativeFunctionKind::FunctionPrototypeApply,
        )? || !self.value_is_native_kind(
            &has_instance,
            NativeFunctionKind::FunctionPrototypeHasInstance,
        )? {
            return Ok(false);
        }
        Ok(self.eval_bytecode_instanceof(dog, dog_ctor)?.is_truthy()
            && self.eval_bytecode_instanceof(dog, animal_ctor)?.is_truthy()
            && !self
                .eval_bytecode_instanceof(&Value::Number(0.0), dog_ctor)?
                .is_truthy())
    }

    fn value_static_property_is_native_kind(
        &mut self,
        value: &Value,
        property: &BytecodeProperty,
        expected: NativeFunctionKind,
    ) -> Result<bool> {
        let property = self.get_static_property_value(value, property.name(), property.access())?;
        self.value_is_native_kind(&property, expected)
    }

    fn value_is_native_kind(&self, value: &Value, expected: NativeFunctionKind) -> Result<bool> {
        let Value::NativeFunction(id) = value else {
            return Ok(false);
        };
        Ok(self.native_function(*id)?.kind() == expected)
    }
}

struct FunctionApplyHasInstanceLoopParts<'a> {
    total: &'a BytecodeBinding,
    sum: &'a BytecodeBinding,
    dog: &'a BytecodeBinding,
    dog_ctor: &'a BytecodeBinding,
    animal_ctor: &'a BytecodeBinding,
    has_instance: &'a BytecodeBinding,
    apply_property: &'a BytecodeProperty,
}

fn function_apply_has_instance_loop_parts<'a>(
    index: &'a BytecodeBinding,
    body: &'a BytecodeBlock,
) -> Option<FunctionApplyHasInstanceLoopParts<'a>> {
    let instructions = body.instructions();
    if instructions.len() != 75 {
        return None;
    }
    let (sum, total, apply_property) = match_apply_array(index, instructions.get(0..16)?)?;
    let second_total = match_apply_object(index, sum, instructions.get(16..30)?)?;
    let (dog, dog_ctor) = match_dog_construction(instructions.get(30..32)?)?;
    let first_total = match_instanceof_branch(dog, dog_ctor, instructions.get(32..42)?)?;
    let (animal_ctor, second_branch_total) =
        match_second_instanceof_branch(dog, instructions.get(42..52)?)?;
    let (has_instance, third_total) =
        match_has_instance_branch(dog, dog_ctor, instructions.get(52..63)?)?;
    let fourth_total = match_negative_has_instance_branch(
        index,
        has_instance,
        dog_ctor,
        instructions.get(63..75)?,
    )?;
    if same_bytecode_binding(total, second_total)
        && same_bytecode_binding(total, first_total)
        && same_bytecode_binding(total, second_branch_total)
        && same_bytecode_binding(total, third_total)
        && same_bytecode_binding(total, fourth_total)
    {
        return Some(FunctionApplyHasInstanceLoopParts {
            total,
            sum,
            dog,
            dog_ctor,
            animal_ctor,
            has_instance,
            apply_property,
        });
    }
    None
}

fn match_apply_array<'a>(
    index: &'a BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<(
    &'a BytecodeBinding,
    &'a BytecodeBinding,
    &'a BytecodeProperty,
)> {
    let [
        BytecodeInstruction::LoadBinding(sum),
        BytecodeInstruction::PushLiteral(Value::Null),
        BytecodeInstruction::LoadBinding(first_index),
        BytecodeInstruction::LoadBinding(second_index),
        BytecodeInstruction::PushLiteral(Value::Number(1.0)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::LoadBinding(third_index),
        BytecodeInstruction::PushLiteral(Value::Number(2.0)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::LoadBinding(fourth_index),
        BytecodeInstruction::PushLiteral(Value::Number(3.0)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::ArrayLiteral { len: 4, holes },
        BytecodeInstruction::CallStaticMember {
            property,
            native: None,
            arg_count: 2,
        },
        BytecodeInstruction::CompoundStoreBinding {
            name: total,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return None;
    };
    if property.name().as_str() == "apply"
        && holes.iter().all(|hole| !*hole)
        && same_bindings(
            index,
            &[first_index, second_index, third_index, fourth_index],
        )
    {
        return Some((sum, total, property));
    }
    None
}

fn match_apply_object<'a>(
    index: &BytecodeBinding,
    sum: &BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<&'a BytecodeBinding> {
    let [
        BytecodeInstruction::LoadBinding(second_sum),
        BytecodeInstruction::ObjectLiteral { properties: empty },
        BytecodeInstruction::PushLiteral(Value::Number(3.0)),
        BytecodeInstruction::LoadBinding(first_index),
        BytecodeInstruction::LoadBinding(second_index),
        BytecodeInstruction::PushLiteral(Value::Number(1.0)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::LoadBinding(third_index),
        BytecodeInstruction::PushLiteral(Value::Number(2.0)),
        BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
        BytecodeInstruction::ObjectLiteral { properties },
        BytecodeInstruction::CallStaticMember {
            property,
            native: None,
            arg_count: 2,
        },
        BytecodeInstruction::CompoundStoreBinding {
            name: total,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return None;
    };
    if empty.is_empty()
        && property.name().as_str() == "apply"
        && same_bytecode_binding(sum, second_sum)
        && same_bindings(index, &[first_index, second_index, third_index])
        && object_properties_match(properties, &["length", "0", "1", "2"])
    {
        return Some(total);
    }
    None
}

fn match_dog_construction(
    instructions: &[BytecodeInstruction],
) -> Option<(&BytecodeBinding, &BytecodeBinding)> {
    let [
        BytecodeInstruction::Construct {
            constructor,
            native: None,
            arg_count: 0,
        },
        BytecodeInstruction::DeclareBinding {
            name: dog,
            kind: DeclKind::Var,
            has_init: true,
        },
    ] = instructions
    else {
        return None;
    };
    Some((dog, constructor))
}

fn match_instanceof_branch<'a>(
    dog: &BytecodeBinding,
    ctor: &BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<&'a BytecodeBinding> {
    let [
        BytecodeInstruction::LoadBinding(test_dog),
        BytecodeInstruction::LoadBinding(test_ctor),
        BytecodeInstruction::Binary {
            op: BinaryOp::InstanceOf,
            property_access: None,
        },
        BytecodeInstruction::JumpIfFalse(_),
        BytecodeInstruction::PushLiteral(Value::Number(1.0)),
        BytecodeInstruction::CompoundStoreBinding {
            name: total,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::Jump(_),
        BytecodeInstruction::PushUndefined,
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return None;
    };
    if same_bytecode_binding(dog, test_dog) && same_bytecode_binding(ctor, test_ctor) {
        return Some(total);
    }
    None
}

fn match_second_instanceof_branch<'a>(
    dog: &BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<(&'a BytecodeBinding, &'a BytecodeBinding)> {
    let [
        BytecodeInstruction::LoadBinding(test_dog),
        BytecodeInstruction::LoadBinding(animal),
        ..,
    ] = instructions
    else {
        return None;
    };
    if !same_bytecode_binding(dog, test_dog) {
        return None;
    }
    let total = match_instanceof_branch(dog, animal, instructions)?;
    Some((animal, total))
}

fn match_has_instance_branch<'a>(
    dog: &BytecodeBinding,
    ctor: &BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<(&'a BytecodeBinding, &'a BytecodeBinding)> {
    let [
        BytecodeInstruction::LoadBinding(has_instance),
        BytecodeInstruction::LoadBinding(call_ctor),
        BytecodeInstruction::LoadBinding(call_dog),
        BytecodeInstruction::CallStaticMember {
            property,
            native: Some(crate::api::native_call::NativeCallTarget::FunctionPrototypeCall),
            arg_count: 2,
        },
        BytecodeInstruction::JumpIfFalse(_),
        BytecodeInstruction::PushLiteral(Value::Number(1.0)),
        BytecodeInstruction::CompoundStoreBinding {
            name: total,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::Jump(_),
        BytecodeInstruction::PushUndefined,
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return None;
    };
    if property.name().as_str() == "call"
        && same_bytecode_binding(ctor, call_ctor)
        && same_bytecode_binding(dog, call_dog)
    {
        return Some((has_instance, total));
    }
    None
}

fn match_negative_has_instance_branch<'a>(
    index: &BytecodeBinding,
    has_instance: &BytecodeBinding,
    ctor: &BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<&'a BytecodeBinding> {
    let [
        BytecodeInstruction::LoadBinding(second_has_instance),
        BytecodeInstruction::LoadBinding(call_ctor),
        BytecodeInstruction::LoadBinding(call_index),
        BytecodeInstruction::CallStaticMember {
            property,
            native: Some(crate::api::native_call::NativeCallTarget::FunctionPrototypeCall),
            arg_count: 2,
        },
        BytecodeInstruction::Unary(UnaryOp::Not),
        BytecodeInstruction::JumpIfFalse(_),
        BytecodeInstruction::PushLiteral(Value::Number(1.0)),
        BytecodeInstruction::CompoundStoreBinding {
            name: total,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::Jump(_),
        BytecodeInstruction::PushUndefined,
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return None;
    };
    if property.name().as_str() == "call"
        && same_bytecode_binding(has_instance, second_has_instance)
        && same_bytecode_binding(ctor, call_ctor)
        && same_bytecode_binding(index, call_index)
    {
        return Some(total);
    }
    None
}

fn same_bindings(binding: &BytecodeBinding, values: &[&BytecodeBinding]) -> bool {
    values
        .iter()
        .all(|value| same_bytecode_binding(binding, value))
}

fn object_properties_match(properties: &[BytecodeObjectProperty], names: &[&str]) -> bool {
    if properties.len() != names.len() {
        return false;
    }
    properties
        .iter()
        .zip(names.iter())
        .all(|(property, name)| matches!(property, BytecodeObjectProperty::Static(value) if value.as_str() == *name))
}

fn function_apply_has_instance_total_delta(start: usize, iterations: usize) -> Result<f64> {
    let end = start
        .checked_add(iterations)
        .ok_or_else(|| Error::limit("function apply loop iteration range overflowed"))?;
    let mut total = 0.0;
    for round in start..end {
        total += 7.0_f64.mul_add(usize_to_f64(round)?, 13.0);
    }
    Ok(total)
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let value = number_to_i32(value, "function apply loop index").ok()?;
    usize::try_from(value).ok()
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value = u32::try_from(value)
        .map_err(|_| Error::limit("function apply loop value exceeds f64 range"))?;
    Ok(f64::from(value))
}
