mod array;
mod call;
mod class;
mod coercion;
mod continuation;
mod control;
mod control_continuation;
mod destructure;
mod destructure_continuation;
mod execution;
mod for_of;
mod in_operator;
mod instruction_stack;
mod linear;
mod ops;
mod private_ops;
mod spread;
pub(in crate::runtime) mod state;
mod string_concat;
mod super_ops;

pub(in crate::runtime) use continuation::BytecodeContinuationFrame;
pub(in crate::runtime) use execution::BytecodeOutcome;

use crate::{
    bytecode::{BytecodeAddress, BytecodeInstruction, BytecodeProperty},
    error::{Error, Result},
    runtime::{Context, control::Completion},
    syntax::UpdateOp,
    value::Value,
};

use state::BytecodeState;

const STRICT_DELETE_FAILURE: &str = "Cannot delete non-configurable property";

fn strict_delete_result(result: Value, strict: bool) -> Result<Value> {
    if strict && result == Value::Bool(false) {
        return Err(Error::type_error(STRICT_DELETE_FAILURE));
    }
    Ok(result)
}

impl Context {
    // Keeping the opcode families together makes the dispatcher exhaustive and
    // keeps every instruction routed through exactly one subsystem.
    #[allow(clippy::too_many_lines)]
    fn eval_bytecode_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
    ) -> Result<Option<Completion>> {
        let next = state.next_pc()?;
        if let Some(result) = self.try_eval_bytecode_private_instruction(state, instruction, next) {
            return result;
        }
        match instruction {
            BytecodeInstruction::BeginPrivateEnvironment { .. }
            | BytecodeInstruction::PushLiteral(_)
            | BytecodeInstruction::PushString(_)
            | BytecodeInstruction::TemplateConcat { .. }
            | BytecodeInstruction::StringConcat { .. }
            | BytecodeInstruction::StringConcatStatic { .. }
            | BytecodeInstruction::CreateRegExp { .. }
            | BytecodeInstruction::PushUndefined
            | BytecodeInstruction::LoadThis
            | BytecodeInstruction::LoadNewTarget
            | BytecodeInstruction::LoadBinding(_)
            | BytecodeInstruction::StoreBinding(_)
            | BytecodeInstruction::StoreAnnexBVar(_)
            | BytecodeInstruction::HoistLexicalBinding { .. }
            | BytecodeInstruction::ResolveBinding(_)
            | BytecodeInstruction::StoreResolvedBinding(_)
            | BytecodeInstruction::DeclareBinding { .. }
            | BytecodeInstruction::StoreLast
            | BytecodeInstruction::Pop
            | BytecodeInstruction::Unary(_)
            | BytecodeInstruction::NumberUnary(_)
            | BytecodeInstruction::Await
            | BytecodeInstruction::GeneratorStart
            | BytecodeInstruction::Yield { .. }
            | BytecodeInstruction::NullishCoalescing { .. }
            | BytecodeInstruction::TypeOfBinding(_)
            | BytecodeInstruction::TypeOfValue => {
                self.eval_bytecode_stack_instruction(state, instruction, next)
            }
            BytecodeInstruction::DeleteBinding(_)
            | BytecodeInstruction::DeleteStaticProperty { .. }
            | BytecodeInstruction::DeleteComputedProperty { .. }
            | BytecodeInstruction::DeleteValue
            | BytecodeInstruction::UpdateBinding { .. }
            | BytecodeInstruction::UpdateStaticProperty { .. }
            | BytecodeInstruction::UpdateArrayIndexProperty { .. }
            | BytecodeInstruction::UpdateComputedProperty { .. }
            | BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::InStaticProperty { .. }
            | BytecodeInstruction::NumberBinary(_)
            | BytecodeInstruction::NumberCompare(_)
            | BytecodeInstruction::NumberEquality(_)
            | BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
            | BytecodeInstruction::CompoundArrayIndexProperty { .. }
            | BytecodeInstruction::CompoundComputedProperty { .. }
            | BytecodeInstruction::LogicalAssignment { .. }
            | BytecodeInstruction::StaticMember { .. }
            | BytecodeInstruction::ArrayLength { .. }
            | BytecodeInstruction::ArrayIndexMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
            | BytecodeInstruction::StaticPropertyAssign { .. }
            | BytecodeInstruction::ArrayIndexAssign { .. }
            | BytecodeInstruction::ComputedPropertyAssign { .. } => {
                self.eval_bytecode_property_instruction(state, instruction, next)
            }
            BytecodeInstruction::CallBinding { .. }
            | BytecodeInstruction::CallValue { .. }
            | BytecodeInstruction::CallStaticMember { .. }
            | BytecodeInstruction::CallComputedMember { .. }
            | BytecodeInstruction::CollectSpreadArgs { .. }
            | BytecodeInstruction::CallBindingSpread { .. }
            | BytecodeInstruction::CallValueSpread
            | BytecodeInstruction::CallStaticMemberSpread { .. }
            | BytecodeInstruction::CallComputedMemberSpread { .. }
            | BytecodeInstruction::ConstructValueSpread
            | BytecodeInstruction::ArrayLiteralSpread { .. }
            | BytecodeInstruction::CreateClass { .. }
            | BytecodeInstruction::CallSuper { .. }
            | BytecodeInstruction::CallSuperSpread
            | BytecodeInstruction::SuperMember { .. }
            | BytecodeInstruction::ComputedSuperMember { .. }
            | BytecodeInstruction::CallSuperMember { .. }
            | BytecodeInstruction::CallSuperMemberSpread { .. }
            | BytecodeInstruction::CallComputedSuperMember { .. }
            | BytecodeInstruction::CallComputedSuperMemberSpread { .. }
            | BytecodeInstruction::SuperPropertyAssign { .. }
            | BytecodeInstruction::UpdateSuperProperty { .. }
            | BytecodeInstruction::CompoundSuperProperty { .. }
            | BytecodeInstruction::Construct { .. }
            | BytecodeInstruction::ConstructValue { .. }
            | BytecodeInstruction::CreateFunction { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. } => {
                self.eval_bytecode_call_instruction(state, instruction, next)
            }
            BytecodeInstruction::While { .. }
            | BytecodeInstruction::DoWhile { .. }
            | BytecodeInstruction::With { .. }
            | BytecodeInstruction::For { .. }
            | BytecodeInstruction::ForIn { .. }
            | BytecodeInstruction::ForOf { .. }
            | BytecodeInstruction::DestructurePattern { .. }
            | BytecodeInstruction::Switch { .. }
            | BytecodeInstruction::Try { .. }
            | BytecodeInstruction::Label { .. }
            | BytecodeInstruction::ScopedBlock { .. }
            | BytecodeInstruction::Jump(_)
            | BytecodeInstruction::JumpIfFalse(_)
            | BytecodeInstruction::JumpIfFalseKeep(_)
            | BytecodeInstruction::JumpIfTrueKeep(_)
            | BytecodeInstruction::Complete(_) => {
                self.eval_bytecode_control_instruction(state, instruction, next)
            }
            _ => Err(Error::runtime("private bytecode dispatch mismatch")),
        }
    }

    fn eval_bytecode_property_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::DeleteBinding(_)
            | BytecodeInstruction::DeleteStaticProperty { .. }
            | BytecodeInstruction::DeleteComputedProperty { .. }
            | BytecodeInstruction::DeleteValue
            | BytecodeInstruction::UpdateBinding { .. }
            | BytecodeInstruction::UpdateStaticProperty { .. }
            | BytecodeInstruction::UpdateArrayIndexProperty { .. }
            | BytecodeInstruction::UpdateComputedProperty { .. }
            | BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::InStaticProperty { .. }
            | BytecodeInstruction::NumberBinary(_)
            | BytecodeInstruction::NumberCompare(_)
            | BytecodeInstruction::NumberEquality(_) => {
                self.eval_bytecode_mutation_instruction(state, instruction, next)
            }
            BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
            | BytecodeInstruction::CompoundArrayIndexProperty { .. }
            | BytecodeInstruction::CompoundComputedProperty { .. }
            | BytecodeInstruction::LogicalAssignment { .. }
            | BytecodeInstruction::StaticMember { .. }
            | BytecodeInstruction::ArrayLength { .. }
            | BytecodeInstruction::ArrayIndexMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
            | BytecodeInstruction::StaticPropertyAssign { .. }
            | BytecodeInstruction::ArrayIndexAssign { .. }
            | BytecodeInstruction::ComputedPropertyAssign { .. } => {
                self.eval_bytecode_member_instruction(state, instruction, next)
            }
            _ => Err(Error::runtime("bytecode property instruction mismatch")),
        }
    }

    fn eval_bytecode_mutation_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::DeleteBinding(binding) => {
                self.eval_bytecode_delete_binding(state, binding, next)
            }
            BytecodeInstruction::DeleteStaticProperty { property, strict } => {
                let object = state.stack.pop()?;
                let deleted =
                    self.delete_static_property_value(&object, property.name(), property.access())?;
                state.stack.push(strict_delete_result(deleted, *strict)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::DeleteComputedProperty {
                property: operand,
                strict,
            } => {
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                let property = self.dynamic_property_key(&property)?;
                let deleted = self.delete_cached_dynamic_property_value(
                    &object,
                    &property,
                    operand.access(),
                )?;
                state.stack.push(strict_delete_result(deleted, *strict)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::DeleteValue => {
                state.stack.pop()?;
                state.stack.push(Value::Bool(true));
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::UpdateBinding { name, op, prefix } => {
                state
                    .stack
                    .push(self.eval_bytecode_update_binding(name, *op, *prefix)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::UpdateStaticProperty {
                property,
                op,
                prefix,
                strict,
            } => {
                let object = state.stack.pop()?;
                state.stack.push(self.eval_bytecode_update_static_property(
                    &object,
                    property.name(),
                    property.access(),
                    *op,
                    *prefix,
                    *strict,
                )?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::UpdateArrayIndexProperty {
                property,
                index,
                op,
                prefix,
                strict,
            } => {
                let object = state.stack.pop()?;
                state.stack.push(self.eval_bytecode_array_index_update(
                    &object, property, *index, *op, *prefix, *strict,
                )?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::UpdateComputedProperty {
                property,
                op,
                prefix,
                strict,
            } => self.eval_bytecode_update_computed_property_instruction(
                state, *property, *op, *prefix, *strict, next,
            ),
            BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::InStaticProperty { .. }
            | BytecodeInstruction::NumberBinary(_)
            | BytecodeInstruction::NumberCompare(_)
            | BytecodeInstruction::NumberEquality(_) => {
                self.eval_bytecode_binary_instruction(state, instruction, next)
            }
            BytecodeInstruction::CompoundStoreBinding { name, op } => {
                let right = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_binding_compound_assignment(*op, name, &right)?);
                state.pc = next;
                Ok(None)
            }
            _ => Err(Error::runtime("bytecode mutation instruction mismatch")),
        }
    }

    fn eval_bytecode_delete_binding(
        &mut self,
        state: &mut BytecodeState,
        binding: &crate::bytecode::BytecodeBinding,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let deleted = if let Some(reference) = self.resolve_with_binding(binding)? {
            reference.delete(self, binding)?
        } else if self.binding_exists_or_materialize_bytecode(binding)? {
            false
        } else {
            self.delete_unresolved_global_property(binding.name().name())?
        };
        state.stack.push(Value::Bool(deleted));
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_binary_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::Binary {
                op,
                property_access,
            } => {
                let right = state.stack.pop()?;
                let left = state.stack.pop()?;
                state.stack.push(self.eval_bytecode_binary(
                    *op,
                    &left,
                    &right,
                    *property_access,
                )?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::InStaticProperty { property, access } => {
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_in_static_property(&object, property, *access)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::NumberBinary(op) => {
                let right = state.stack.pop()?;
                let left = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_number_binary(*op, &left, &right)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::NumberCompare(op) => {
                let right = state.stack.pop()?;
                let left = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_number_compare(*op, &left, &right)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::NumberEquality(op) => {
                let right = state.stack.pop()?;
                let left = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_number_equality(*op, &left, &right)?);
                state.pc = next;
                Ok(None)
            }
            _ => Err(Error::runtime("bytecode binary instruction mismatch")),
        }
    }

    fn eval_bytecode_update_computed_property_instruction(
        &mut self,
        state: &mut BytecodeState,
        property: crate::bytecode::BytecodeDynamicProperty,
        op: UpdateOp,
        prefix: bool,
        strict: bool,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let key = state.stack.pop()?;
        let object = state.stack.pop()?;
        if let Some(value) = self.eval_dynamic_array_index_update(
            &object,
            &key,
            property.access(),
            op,
            prefix,
            strict,
        )? {
            state.stack.push(value);
            state.pc = next;
            return Ok(None);
        }
        if matches!(object, Value::Undefined | Value::Null) {
            return Err(Error::type_error(
                "Cannot read properties of undefined or null",
            ));
        }
        let key = self.dynamic_property_key(&key)?;
        state.stack.push(self.eval_bytecode_update_dynamic_property(
            &object,
            key,
            property.access(),
            op,
            prefix,
            strict,
        )?);
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_member_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
            | BytecodeInstruction::CompoundArrayIndexProperty { .. }
            | BytecodeInstruction::CompoundComputedProperty { .. } => {
                self.eval_bytecode_compound_member_instruction(state, instruction, next)
            }
            BytecodeInstruction::LogicalAssignment { op, target, value } => {
                state
                    .stack
                    .push(self.eval_bytecode_logical_assignment(*op, target, value)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::StaticMember { property } => {
                self.eval_bytecode_static_member_instruction(state, property, next)
            }
            BytecodeInstruction::ArrayLength { property } => {
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_array_length(&object, property)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::ArrayIndexMember { property, index } => {
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_array_index_member(&object, property, *index)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::ComputedMember { property: operand } => {
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                if let Some(value) = self.eval_dynamic_array_index_member(&object, &property)? {
                    state.stack.push(value);
                    state.pc = next;
                    return Ok(None);
                }
                let key = self.dynamic_property_key(&property)?;
                state.stack.push(self.get_cached_dynamic_property_value(
                    &object,
                    &key,
                    operand.access(),
                )?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::StaticPropertyAssign { .. }
            | BytecodeInstruction::ArrayIndexAssign { .. }
            | BytecodeInstruction::ComputedPropertyAssign { .. } => {
                self.eval_bytecode_property_assign_instruction(state, instruction, next)
            }
            _ => Err(Error::runtime("bytecode member instruction mismatch")),
        }
    }

    fn eval_bytecode_compound_member_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::CompoundStoreBinding { name, op } => {
                let right = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_binding_compound_assignment(*op, name, &right)?);
            }
            BytecodeInstruction::CompoundStaticProperty {
                property,
                op,
                strict,
            } => {
                let right = state.stack.pop()?;
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_static_compound_assignment(
                        *op,
                        &object,
                        property.name(),
                        property.access(),
                        &right,
                        *strict,
                    )?);
            }
            BytecodeInstruction::CompoundArrayIndexProperty {
                property,
                index,
                op,
                strict,
            } => {
                let right = state.stack.pop()?;
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_array_index_compound_assignment(
                        *op, &object, property, *index, &right, *strict,
                    )?);
            }
            BytecodeInstruction::CompoundComputedProperty {
                property: operand,
                op,
                strict,
            } => {
                let right = state.stack.pop()?;
                let key = state.stack.pop()?;
                let object = state.stack.pop()?;
                if let Some(value) = self.eval_dynamic_array_index_compound_assignment(
                    *op,
                    &object,
                    &key,
                    operand.access(),
                    &right,
                    *strict,
                )? {
                    state.stack.push(value);
                    state.pc = next;
                    return Ok(None);
                }
                let key = self.dynamic_property_key(&key)?;
                state
                    .stack
                    .push(self.eval_bytecode_dynamic_compound_assignment(
                        *op,
                        &object,
                        key,
                        operand.access(),
                        &right,
                        *strict,
                    )?);
            }
            _ => return Err(Error::runtime("bytecode compound member mismatch")),
        }
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_property_assign_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::StaticPropertyAssign { property, strict } => {
                let value = state.stack.pop()?;
                let object = state.stack.pop()?;
                self.set_bytecode_static_property_reference(
                    &object,
                    property.name(),
                    property.access(),
                    value.clone(),
                    *strict,
                )?;
                state.stack.push(value);
            }
            BytecodeInstruction::ArrayIndexAssign {
                property,
                index,
                strict,
            } => {
                let value = state.stack.pop()?;
                let object = state.stack.pop()?;
                self.set_bytecode_array_index_property(
                    &object,
                    property,
                    *index,
                    value.clone(),
                    *strict,
                )?;
                state.stack.push(value);
            }
            BytecodeInstruction::ComputedPropertyAssign {
                property: operand,
                strict,
            } => {
                let value = state.stack.pop()?;
                let key = state.stack.pop()?;
                let object = state.stack.pop()?;
                if self.set_dynamic_array_index_property(&object, &key, value.clone(), *strict)? {
                    state.stack.push(value);
                    state.pc = next;
                    return Ok(None);
                }
                let mut key = self.dynamic_property_key(&key)?;
                self.set_bytecode_dynamic_property_reference(
                    &object,
                    &mut key,
                    operand.access(),
                    value.clone(),
                    *strict,
                )?;
                state.stack.push(value);
            }
            _ => return Err(Error::runtime("bytecode property assignment mismatch")),
        }
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_static_member_instruction(
        &mut self,
        state: &mut BytecodeState,
        property: &BytecodeProperty,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let object = state.stack.pop()?;
        state.stack.push(self.get_static_property_value(
            &object,
            property.name(),
            property.access(),
        )?);
        state.pc = next;
        Ok(None)
    }
}
