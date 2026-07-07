mod array;
mod call;
mod coercion;
mod control;
mod ops;
mod state;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBlock, BytecodeInstruction, BytecodeProgram, BytecodeProperty,
    },
    error::{Error, Result},
    runtime::{Context, assertions::runtime_exception_value, completion::Completion},
    syntax::{StaticString, UpdateOp},
    value::Value,
};

use state::BytecodeState;

impl Context {
    pub(crate) fn eval_bytecode_program(
        &mut self,
        bytecode: &BytecodeProgram,
    ) -> Result<Completion> {
        self.eval_bytecode_block(bytecode.block())
    }

    pub(super) fn eval_bytecode_block(&mut self, block: &BytecodeBlock) -> Result<Completion> {
        let mut state = BytecodeState::new();
        while let Some(instruction) = block.instruction(state.pc)? {
            self.step()?;
            let result = self.eval_bytecode_instruction(&mut state, instruction);
            let completion = match result {
                Ok(completion) => completion,
                Err(error) => {
                    if let Some(value) = runtime_exception_value(&error) {
                        self.checked_value(value.clone())?;
                        Some(Completion::Throw(value))
                    } else {
                        return Err(error);
                    }
                }
            };
            if let Some(completion) = completion {
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(state.last))
    }

    pub(super) fn eval_bytecode_expression(&mut self, block: &BytecodeBlock) -> Result<Value> {
        self.eval_bytecode_block(block)?.into_result()
    }

    fn eval_bytecode_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
    ) -> Result<Option<Completion>> {
        let next = state.next_pc()?;
        match instruction {
            BytecodeInstruction::PushLiteral(_)
            | BytecodeInstruction::PushString(_)
            | BytecodeInstruction::CreateRegExp { .. }
            | BytecodeInstruction::PushUndefined
            | BytecodeInstruction::LoadThis
            | BytecodeInstruction::LoadNewTarget
            | BytecodeInstruction::LoadBinding(_)
            | BytecodeInstruction::StoreBinding(_)
            | BytecodeInstruction::DeclareBinding { .. }
            | BytecodeInstruction::StoreLast
            | BytecodeInstruction::Pop
            | BytecodeInstruction::Unary(_)
            | BytecodeInstruction::NumberUnary(_)
            | BytecodeInstruction::Await
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
            | BytecodeInstruction::NumberBinary(_)
            | BytecodeInstruction::NumberCompare(_)
            | BytecodeInstruction::NumberEquality(_)
            | BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
            | BytecodeInstruction::CompoundArrayIndexProperty { .. }
            | BytecodeInstruction::CompoundComputedProperty { .. }
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
            | BytecodeInstruction::Print { .. }
            | BytecodeInstruction::AssertThrows { .. }
            | BytecodeInstruction::Construct { .. }
            | BytecodeInstruction::ConstructValue { .. }
            | BytecodeInstruction::CreateFunction { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. } => {
                self.eval_bytecode_call_instruction(state, instruction, next)
            }
            BytecodeInstruction::While { .. }
            | BytecodeInstruction::DoWhile { .. }
            | BytecodeInstruction::For { .. }
            | BytecodeInstruction::ForIn { .. }
            | BytecodeInstruction::Switch { .. }
            | BytecodeInstruction::Try { .. }
            | BytecodeInstruction::Label { .. }
            | BytecodeInstruction::ScopedBlock(_)
            | BytecodeInstruction::Jump(_)
            | BytecodeInstruction::JumpIfFalse(_)
            | BytecodeInstruction::JumpIfFalseKeep(_)
            | BytecodeInstruction::JumpIfTrueKeep(_)
            | BytecodeInstruction::Complete(_) => {
                self.eval_bytecode_control_instruction(state, instruction, next)
            }
        }
    }

    fn eval_bytecode_stack_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::PushLiteral(value) => {
                state.stack.push(self.runtime_value(value.clone())?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::PushString(value) => {
                state.stack.push(self.static_string_value(value)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CreateRegExp { pattern, flags } => {
                self.eval_bytecode_create_regexp(state, pattern, flags, next)
            }
            BytecodeInstruction::PushUndefined => {
                state.stack.push(Value::Undefined);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::LoadThis => {
                state.stack.push(self.current_this()?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::LoadNewTarget => {
                state.stack.push(self.current_new_target()?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::LoadBinding(binding) => {
                state.stack.push(self.eval_bytecode_identifier(binding)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::StoreBinding(binding) => {
                let value = state.stack.pop()?;
                self.assign_bytecode_or_builtin(binding, value.clone())?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::DeclareBinding {
                name,
                kind,
                has_init,
            } => {
                let value = if *has_init {
                    Some(state.stack.pop()?)
                } else {
                    None
                };
                self.eval_bytecode_declaration(name, *kind, value)?;
                state.last = Value::Undefined;
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::StoreLast => {
                state.last = state.stack.pop()?;
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::Pop => {
                state.stack.pop()?;
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::Unary(_) | BytecodeInstruction::NumberUnary(_) => {
                self.eval_bytecode_unary_instruction(state, instruction, next)
            }
            BytecodeInstruction::Await => {
                let value = state.stack.pop()?;
                match self.eval_bytecode_await(value)? {
                    Completion::Normal(value) => {
                        state.stack.push(value);
                        state.pc = next;
                        Ok(None)
                    }
                    Completion::Throw(value) => Ok(Some(Completion::Throw(value))),
                    completion @ (Completion::Return(_)
                    | Completion::Break { .. }
                    | Completion::Continue(_)) => completion.into_result().map(|_| None),
                }
            }
            BytecodeInstruction::TypeOfBinding(binding) => {
                state
                    .stack
                    .push(self.eval_bytecode_typeof_binding(binding)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::TypeOfValue => {
                let value = state.stack.pop()?;
                state.stack.push(self.heap_string_value(value.type_name())?);
                state.pc = next;
                Ok(None)
            }
            _ => Err(Error::runtime("bytecode stack instruction mismatch")),
        }
    }

    fn eval_bytecode_create_regexp(
        &mut self,
        state: &mut BytecodeState,
        pattern: &StaticString,
        flags: &StaticString,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        state
            .stack
            .push(self.create_regexp_literal(pattern.as_str(), flags.as_str())?);
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_unary_instruction(
        &self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::Unary(op) => {
                let value = state.stack.pop()?;
                state.stack.push(Self::eval_bytecode_unary(*op, &value)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::NumberUnary(op) => {
                let value = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_number_unary(*op, &value)?);
                state.pc = next;
                Ok(None)
            }
            _ => Err(Error::runtime("bytecode unary instruction mismatch")),
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
            | BytecodeInstruction::NumberBinary(_)
            | BytecodeInstruction::NumberCompare(_)
            | BytecodeInstruction::NumberEquality(_) => {
                self.eval_bytecode_mutation_instruction(state, instruction, next)
            }
            BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
            | BytecodeInstruction::CompoundArrayIndexProperty { .. }
            | BytecodeInstruction::CompoundComputedProperty { .. }
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
                let exists = self.binding_exists_or_materialize_bytecode(binding)?;
                state.stack.push(Value::Bool(!exists));
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::DeleteStaticProperty { property } => {
                let object = state.stack.pop()?;
                state.stack.push(self.delete_static_property_value(
                    &object,
                    property.name(),
                    property.access(),
                )?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::DeleteComputedProperty { property: operand } => {
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                let property = self.dynamic_property_key(&property)?;
                state.stack.push(self.delete_cached_dynamic_property_value(
                    &object,
                    &property,
                    operand.access(),
                )?);
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
            } => {
                let object = state.stack.pop()?;
                state.stack.push(self.eval_bytecode_update_static_property(
                    &object,
                    property.name(),
                    property.access(),
                    *op,
                    *prefix,
                )?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::UpdateArrayIndexProperty {
                property,
                index,
                op,
                prefix,
            } => {
                let object = state.stack.pop()?;
                state.stack.push(
                    self.eval_bytecode_array_index_update(&object, property, *index, *op, *prefix)?,
                );
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::UpdateComputedProperty {
                property,
                op,
                prefix,
            } => self.eval_bytecode_update_computed_property_instruction(
                state, *property, *op, *prefix, next,
            ),
            BytecodeInstruction::Binary { .. }
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
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let key = state.stack.pop()?;
        let object = state.stack.pop()?;
        if let Some(value) =
            self.eval_dynamic_array_index_update(&object, &key, property.access(), op, prefix)?
        {
            state.stack.push(value);
            state.pc = next;
            return Ok(None);
        }
        let key = self.dynamic_property_key(&key)?;
        state.stack.push(self.eval_bytecode_update_dynamic_property(
            &object,
            key,
            property.access(),
            op,
            prefix,
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
            BytecodeInstruction::CompoundStaticProperty { property, op } => {
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
                    )?);
            }
            BytecodeInstruction::CompoundArrayIndexProperty {
                property,
                index,
                op,
            } => {
                let right = state.stack.pop()?;
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_array_index_compound_assignment(
                        *op, &object, property, *index, &right,
                    )?);
            }
            BytecodeInstruction::CompoundComputedProperty {
                property: operand,
                op,
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
            BytecodeInstruction::StaticPropertyAssign { property } => {
                let value = state.stack.pop()?;
                let object = state.stack.pop()?;
                self.set_static_property_value(
                    &object,
                    property.name(),
                    property.access(),
                    value.clone(),
                )?;
                state.stack.push(value);
            }
            BytecodeInstruction::ArrayIndexAssign { property, index } => {
                let value = state.stack.pop()?;
                let object = state.stack.pop()?;
                self.set_bytecode_array_index_property(&object, property, *index, value.clone())?;
                state.stack.push(value);
            }
            BytecodeInstruction::ComputedPropertyAssign { property: operand } => {
                let value = state.stack.pop()?;
                let key = state.stack.pop()?;
                let object = state.stack.pop()?;
                if self.set_dynamic_array_index_property(&object, &key, value.clone())? {
                    state.stack.push(value);
                    state.pc = next;
                    return Ok(None);
                }
                let mut key = self.dynamic_property_key(&key)?;
                self.set_cached_dynamic_property_value(
                    &object,
                    &mut key,
                    operand.access(),
                    value.clone(),
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
