use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeCompletion, BytecodeInstruction,
        BytecodeNumericCompareOp, BytecodeProperty,
    },
    error::{Error, Result},
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    syntax::{BinaryOp, DeclKind},
    value::{FunctionId, ObjectId, Value},
};

use super::{for_loop::BytecodeForLoopFastPath, loop_helpers::same_bytecode_binding};

#[derive(Debug)]
pub(super) struct BytecodeConstructorPrototypeLoopFastPath<'a> {
    total: &'a BytecodeBinding,
    total_cell: BindingCell,
    constructor: &'a BytecodeBinding,
    constructor_cell: BindingCell,
    bump_property: &'a BytecodeProperty,
    read_property: &'a BytecodeProperty,
}

impl Context {
    pub(super) fn compile_constructor_prototype_loop_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeConstructorPrototypeLoopFastPath<'a>>> {
        let Some(parts) = constructor_prototype_loop_parts(index, body) else {
            return Ok(None);
        };
        if self.builtin_value(parts.total.name().name())?.is_some() {
            return Ok(None);
        }
        let Some(total_cell) = self.get_or_materialize_binding_bytecode(parts.total)? else {
            return Ok(None);
        };
        let Some(constructor_cell) = self.get_binding_bytecode(parts.constructor)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeConstructorPrototypeLoopFastPath {
            total: parts.total,
            total_cell,
            constructor: parts.constructor,
            constructor_cell,
            bump_property: parts.bump_property,
            read_property: parts.read_property,
        }))
    }

    pub(super) fn eval_constructor_prototype_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeConstructorPrototypeLoopFastPath<'_>,
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
        let constructor = body.constructor_cell.value(body.constructor.name())?;
        if !self.constructor_prototype_loop_guards(&constructor, body)? {
            return Ok(false);
        }
        let total = total + constructor_prototype_total_delta(start_index, iterations)?;
        let total_value = self.checked_value(Value::Number(total))?;
        self.charge_runtime_steps(iterations)?;
        self.record_bytecode_linear_direct_runs(iterations)?;
        self.assign_fast_path_cell(body.total, &body.total_cell, total_value.clone())?;
        let index_value = self.checked_value(Value::Number(usize_to_f64(limit)?))?;
        self.assign_fast_path_cell(fast_path.index, &fast_path.index_cell, index_value)?;
        state.last = total_value;
        state.pc = next;
        Ok(true)
    }

    pub(super) fn constructor_prototype_loop_fast_path_ready(
        &self,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeConstructorPrototypeLoopFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(fast_path.compare, BytecodeNumericCompareOp::Less)
            || fast_path.update_step.to_bits() != 1.0f64.to_bits()
            || !matches!(body.total_cell.value(body.total.name())?, Value::Number(_))
        {
            return Ok(false);
        }
        let constructor = body.constructor_cell.value(body.constructor.name())?;
        self.constructor_prototype_loop_guards(&constructor, body)
    }

    fn constructor_prototype_loop_guards(
        &self,
        constructor: &Value,
        body: &BytecodeConstructorPrototypeLoopFastPath<'_>,
    ) -> Result<bool> {
        let Value::Function(constructor_id) = constructor else {
            return Ok(false);
        };
        if !self.constructor_function_matches(*constructor_id)? {
            return Ok(false);
        }
        let Some(prototype) = self.function_constructor_prototype(*constructor_id)? else {
            return Ok(false);
        };
        let Some(bump) = self.own_static_property_value(prototype, body.bump_property)? else {
            return Ok(false);
        };
        let Some(read) = self.own_static_property_value(prototype, body.read_property)? else {
            return Ok(false);
        };
        Ok(self.bump_function_matches(&bump)? && self.read_function_matches(&read)?)
    }

    fn constructor_function_matches(&self, id: FunctionId) -> Result<bool> {
        let function = self.function(id)?;
        if !function.constructable
            || function.is_async
            || function.class_constructor
            || function.bytecode.params().len() != 1
            || function.bytecode.has_parameter_defaults()
        {
            return Ok(false);
        }
        let Some(param) = function.bytecode.params().first() else {
            return Ok(false);
        };
        Ok(constructor_body_matches(
            function.bytecode.body().instructions(),
            param.binding().as_str(),
        ))
    }

    fn bump_function_matches(&self, value: &Value) -> Result<bool> {
        let Value::Function(id) = value else {
            return Ok(false);
        };
        let function = self.function(*id)?;
        if function.is_async
            || function.class_constructor
            || function.bytecode.params().len() != 1
            || function.bytecode.has_parameter_defaults()
        {
            return Ok(false);
        }
        let Some(param) = function.bytecode.params().first() else {
            return Ok(false);
        };
        Ok(bump_body_matches(
            function.bytecode.body().instructions(),
            param.binding().as_str(),
        ))
    }

    fn read_function_matches(&self, value: &Value) -> Result<bool> {
        let Value::Function(id) = value else {
            return Ok(false);
        };
        let function = self.function(*id)?;
        if function.is_async
            || function.class_constructor
            || !function.bytecode.params().is_empty()
            || function.bytecode.has_parameter_defaults()
        {
            return Ok(false);
        }
        Ok(read_body_matches(function.bytecode.body().instructions()))
    }

    fn own_static_property_value(
        &self,
        object: ObjectId,
        property: &BytecodeProperty,
    ) -> Result<Option<Value>> {
        let lookup = self.static_property_lookup(property.name())?;
        self.objects.own_data_property_value(object, lookup)
    }
}

struct ConstructorPrototypeLoopParts<'a> {
    total: &'a BytecodeBinding,
    constructor: &'a BytecodeBinding,
    bump_property: &'a BytecodeProperty,
    read_property: &'a BytecodeProperty,
}

fn constructor_prototype_loop_parts<'a>(
    index: &'a BytecodeBinding,
    body: &'a BytecodeBlock,
) -> Option<ConstructorPrototypeLoopParts<'a>> {
    let [BytecodeInstruction::ScopedBlock(block)] = body.instructions() else {
        return None;
    };
    let instructions = block.instructions();
    if instructions.len() != 21 {
        return None;
    }
    let (camera, constructor) = match_camera_construction(index, instructions.get(0..3)?)?;
    let (total, bump_property) = match_bump_call(camera, instructions.get(3..8)?)?;
    let (second_total, read_property) = match_read_call(camera, instructions.get(8..12)?)?;
    let third_total = match_bump_in_branch(camera, instructions.get(12..21)?)?;
    if same_bytecode_binding(total, second_total) && same_bytecode_binding(total, third_total) {
        return Some(ConstructorPrototypeLoopParts {
            total,
            constructor,
            bump_property,
            read_property,
        });
    }
    None
}

fn match_camera_construction<'a>(
    index: &'a BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<(&'a BytecodeBinding, &'a BytecodeBinding)> {
    let [
        BytecodeInstruction::LoadBinding(argument),
        BytecodeInstruction::Construct {
            constructor,
            native: None,
            arg_count: 1,
        },
        BytecodeInstruction::DeclareBinding {
            name: camera,
            kind: DeclKind::Let,
            has_init: true,
        },
    ] = instructions
    else {
        return None;
    };
    if same_bytecode_binding(index, argument) {
        return Some((camera, constructor));
    }
    None
}

fn match_bump_call<'a>(
    camera: &BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<(&'a BytecodeBinding, &'a BytecodeProperty)> {
    let [
        BytecodeInstruction::LoadBinding(receiver),
        BytecodeInstruction::PushLiteral(Value::Number(1.0)),
        BytecodeInstruction::CallStaticMember {
            property,
            native: None,
            arg_count: 1,
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
    if property.name().as_str() == "bump" && same_bytecode_binding(camera, receiver) {
        return Some((total, property));
    }
    None
}

fn match_read_call<'a>(
    camera: &BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<(&'a BytecodeBinding, &'a BytecodeProperty)> {
    let [
        BytecodeInstruction::LoadBinding(receiver),
        BytecodeInstruction::CallStaticMember {
            property,
            native: None,
            arg_count: 0,
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
    if property.name().as_str() == "read" && same_bytecode_binding(camera, receiver) {
        return Some((total, property));
    }
    None
}

fn match_bump_in_branch<'a>(
    camera: &BytecodeBinding,
    instructions: &'a [BytecodeInstruction],
) -> Option<&'a BytecodeBinding> {
    let [
        BytecodeInstruction::LoadBinding(receiver),
        BytecodeInstruction::InStaticProperty { property, .. },
        BytecodeInstruction::JumpIfFalse(alternate),
        BytecodeInstruction::PushLiteral(Value::Number(1.0)),
        BytecodeInstruction::CompoundStoreBinding {
            name: total,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::Jump(end),
        BytecodeInstruction::PushUndefined,
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return None;
    };
    if property.as_str() == "bump"
        && alternate.index() == 19
        && end.index() == 21
        && same_bytecode_binding(camera, receiver)
    {
        return Some(total);
    }
    None
}

fn constructor_body_matches(instructions: &[BytecodeInstruction], param: &str) -> bool {
    let [
        BytecodeInstruction::LoadThis,
        BytecodeInstruction::LoadBinding(value),
        BytecodeInstruction::StaticPropertyAssign { property },
        BytecodeInstruction::StoreLast,
    ] = instructions
    else {
        return false;
    };
    value.name().as_str() == param && property.name().as_str() == "value"
}

fn bump_body_matches(instructions: &[BytecodeInstruction], param: &str) -> bool {
    let [
        BytecodeInstruction::LoadThis,
        BytecodeInstruction::LoadBinding(delta),
        BytecodeInstruction::CompoundStaticProperty {
            property,
            op: BinaryOp::Add,
        },
        BytecodeInstruction::StoreLast,
        BytecodeInstruction::LoadThis,
        BytecodeInstruction::StaticMember {
            property: returned_property,
        },
        BytecodeInstruction::Complete(completion),
    ] = instructions
    else {
        return false;
    };
    delta.name().as_str() == param
        && property.name().as_str() == "value"
        && returned_property.name().as_str() == "value"
        && matches!(completion, BytecodeCompletion::Return)
}

fn read_body_matches(instructions: &[BytecodeInstruction]) -> bool {
    let [
        BytecodeInstruction::LoadThis,
        BytecodeInstruction::StaticMember { property },
        BytecodeInstruction::Complete(BytecodeCompletion::Return),
    ] = instructions
    else {
        return false;
    };
    property.name().as_str() == "value"
}

fn constructor_prototype_total_delta(start: usize, iterations: usize) -> Result<f64> {
    let end = start
        .checked_add(iterations)
        .ok_or_else(|| Error::limit("constructor prototype loop iteration range overflowed"))?;
    let mut total = 0.0;
    for index in start..end {
        total += 2.0_f64.mul_add(usize_to_f64(index)?, 3.0);
    }
    Ok(total)
}

fn non_negative_integer_index(value: f64) -> Option<usize> {
    if !value.is_finite() || value.is_sign_negative() || value.trunc().to_bits() != value.to_bits()
    {
        return None;
    }
    let value = number_to_i32(value, "constructor prototype loop index").ok()?;
    usize::try_from(value).ok()
}

fn usize_to_f64(value: usize) -> Result<f64> {
    let value = u32::try_from(value)
        .map_err(|_| Error::limit("constructor prototype loop value exceeds f64 range"))?;
    Ok(f64::from(value))
}
