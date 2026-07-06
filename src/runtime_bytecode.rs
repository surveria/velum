#[path = "runtime_bytecode_control.rs"]
mod runtime_bytecode_control;
#[path = "runtime_bytecode_ops.rs"]
mod runtime_bytecode_ops;
#[path = "runtime_bytecode_state.rs"]
mod runtime_bytecode_state;

use crate::{
    bytecode::{BytecodeAddress, BytecodeBlock, BytecodeInstruction, BytecodeProgram},
    error::{Error, Result},
    runtime::Context,
    runtime_assertions::runtime_exception_value,
    runtime_call_args::RuntimeCallArgs,
    runtime_completion::Completion,
    value::Value,
};

use runtime_bytecode_state::BytecodeState;

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
            | BytecodeInstruction::PushUndefined
            | BytecodeInstruction::LoadThis
            | BytecodeInstruction::LoadBinding(_)
            | BytecodeInstruction::StoreBinding(_)
            | BytecodeInstruction::DeclareBinding { .. }
            | BytecodeInstruction::StoreLast
            | BytecodeInstruction::Pop
            | BytecodeInstruction::Unary(_)
            | BytecodeInstruction::TypeOfBinding(_)
            | BytecodeInstruction::TypeOfValue => {
                self.eval_bytecode_stack_instruction(state, instruction, next)
            }
            BytecodeInstruction::DeleteBinding(_)
            | BytecodeInstruction::DeleteStaticProperty { .. }
            | BytecodeInstruction::DeleteComputedProperty
            | BytecodeInstruction::DeleteValue
            | BytecodeInstruction::UpdateBinding { .. }
            | BytecodeInstruction::UpdateStaticProperty { .. }
            | BytecodeInstruction::UpdateComputedProperty { .. }
            | BytecodeInstruction::Binary { .. }
            | BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
            | BytecodeInstruction::CompoundComputedProperty { .. }
            | BytecodeInstruction::StaticMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
            | BytecodeInstruction::StaticPropertyAssign { .. }
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
            | BytecodeInstruction::CreateFunction { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. } => {
                self.eval_bytecode_call_instruction(state, instruction, next)
            }
            BytecodeInstruction::If { .. }
            | BytecodeInstruction::While { .. }
            | BytecodeInstruction::For { .. }
            | BytecodeInstruction::ForIn { .. }
            | BytecodeInstruction::Switch { .. }
            | BytecodeInstruction::Try { .. }
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
            BytecodeInstruction::LoadBinding(binding) => {
                state.stack.push(self.eval_bytecode_identifier(binding)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::StoreBinding(binding) => {
                let value = state.stack.pop()?;
                self.assign_static_or_builtin(binding, value.clone())?;
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
            BytecodeInstruction::Unary(op) => {
                let value = state.stack.pop()?;
                state.stack.push(Self::eval_bytecode_unary(*op, &value)?);
                state.pc = next;
                Ok(None)
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

    fn eval_bytecode_property_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::DeleteBinding(_)
            | BytecodeInstruction::DeleteStaticProperty { .. }
            | BytecodeInstruction::DeleteComputedProperty
            | BytecodeInstruction::DeleteValue
            | BytecodeInstruction::UpdateBinding { .. }
            | BytecodeInstruction::UpdateStaticProperty { .. }
            | BytecodeInstruction::UpdateComputedProperty { .. }
            | BytecodeInstruction::Binary { .. } => {
                self.eval_bytecode_mutation_instruction(state, instruction, next)
            }
            BytecodeInstruction::CompoundStoreBinding { .. }
            | BytecodeInstruction::CompoundStaticProperty { .. }
            | BytecodeInstruction::CompoundComputedProperty { .. }
            | BytecodeInstruction::StaticMember { .. }
            | BytecodeInstruction::ComputedMember { .. }
            | BytecodeInstruction::StaticPropertyAssign { .. }
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
                let exists = self.binding_exists_or_materialize_static(binding)?;
                state.stack.push(Value::Bool(!exists));
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::DeleteStaticProperty { property } => {
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.delete_static_property_value(&object, property)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::DeleteComputedProperty => {
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                let property = self.dynamic_property_key(&property)?;
                state
                    .stack
                    .push(self.delete_dynamic_property_value(&object, &property)?);
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
                access,
                op,
                prefix,
            } => {
                let object = state.stack.pop()?;
                state.stack.push(self.eval_bytecode_update_static_property(
                    &object, property, *access, *op, *prefix,
                )?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::UpdateComputedProperty { access, op, prefix } => {
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                let property = self.dynamic_property_key(&property)?;
                state.stack.push(self.eval_bytecode_update_dynamic_property(
                    &object, property, *access, *op, *prefix,
                )?);
                state.pc = next;
                Ok(None)
            }
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

    fn eval_bytecode_member_instruction(
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
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CompoundStaticProperty {
                property,
                access,
                op,
            } => {
                let right = state.stack.pop()?;
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_static_compound_assignment(
                        *op, &object, property, *access, &right,
                    )?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CompoundComputedProperty { access, op } => {
                let right = state.stack.pop()?;
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                let property = self.dynamic_property_key(&property)?;
                state
                    .stack
                    .push(self.eval_bytecode_dynamic_compound_assignment(
                        *op, &object, property, *access, &right,
                    )?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::StaticMember { property, access } => {
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.get_static_property_value(&object, property, *access)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::ComputedMember { access } => {
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                let property = self.dynamic_property_key(&property)?;
                state
                    .stack
                    .push(self.get_cached_dynamic_property_value(&object, &property, *access)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::StaticPropertyAssign { property, access } => {
                let value = state.stack.pop()?;
                let object = state.stack.pop()?;
                self.set_static_property_value(&object, property, *access, value.clone())?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::ComputedPropertyAssign { access } => {
                let value = state.stack.pop()?;
                let property = state.stack.pop()?;
                let object = state.stack.pop()?;
                let mut property = self.dynamic_property_key(&property)?;
                self.set_cached_dynamic_property_value(
                    &object,
                    &mut property,
                    *access,
                    value.clone(),
                )?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            _ => Err(Error::runtime("bytecode member instruction mismatch")),
        }
    }

    fn eval_bytecode_call_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::CallBinding { .. }
            | BytecodeInstruction::CallValue { .. }
            | BytecodeInstruction::CallStaticMember { .. }
            | BytecodeInstruction::CallComputedMember { .. }
            | BytecodeInstruction::Print { .. }
            | BytecodeInstruction::AssertThrows { .. } => {
                self.eval_bytecode_invocation_instruction(state, instruction, next)
            }
            BytecodeInstruction::Construct { .. }
            | BytecodeInstruction::CreateFunction { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. } => {
                self.eval_bytecode_creation_instruction(state, instruction, next)
            }
            _ => Err(Error::runtime("bytecode call instruction mismatch")),
        }
    }

    fn eval_bytecode_invocation_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::CallBinding { callee, arg_count } => {
                let args = state.stack.pop_many(*arg_count)?;
                state
                    .stack
                    .push(self.eval_identifier_call_value(callee, &args)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CallValue { arg_count } => {
                let args = state.stack.pop_many(*arg_count)?;
                let callee = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_call_value(callee, &args, Value::Undefined)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CallStaticMember {
                property,
                access,
                arg_count,
            } => {
                let args = state.stack.pop_many(*arg_count)?;
                let this_value = state.stack.pop()?;
                let callee = self.get_static_property_value(&this_value, property, *access)?;
                state
                    .stack
                    .push(self.eval_call_value(callee, &args, this_value)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CallComputedMember { access, arg_count } => {
                let args = state.stack.pop_many(*arg_count)?;
                let property = state.stack.pop()?;
                let this_value = state.stack.pop()?;
                let property = self.dynamic_property_key(&property)?;
                let callee =
                    self.get_cached_dynamic_property_value(&this_value, &property, *access)?;
                state
                    .stack
                    .push(self.eval_call_value(callee, &args, this_value)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::Print { arg_count } => {
                let args = state.stack.pop_many(*arg_count)?;
                state
                    .stack
                    .push(self.eval_print_call(RuntimeCallArgs::values(&args))?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::AssertThrows {
                expected,
                has_message,
            } => {
                let message = if *has_message {
                    Some(state.stack.pop()?)
                } else {
                    None
                };
                let callback = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_assert_throws(*expected, &callback, message)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::Construct { .. }
            | BytecodeInstruction::CreateFunction { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. } => {
                self.eval_bytecode_creation_instruction(state, instruction, next)
            }
            _ => Err(Error::runtime("bytecode invocation instruction mismatch")),
        }
    }

    fn eval_bytecode_creation_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::Construct {
                constructor,
                arg_count,
            } => {
                let args = state.stack.pop_many(*arg_count)?;
                state.stack.push(self.eval_new_value(constructor, &args)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CreateFunction {
                id,
                name,
                params,
                bytecode,
                constructable,
            } => {
                let function = self.create_bytecode_function(
                    *id,
                    name.as_ref(),
                    params,
                    bytecode,
                    *constructable,
                )?;
                state.stack.push(function);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::ArrayLiteral { len } => {
                let values = state.stack.pop_many(*len)?;
                state.stack.push(self.create_array_from_elements(values)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::ObjectLiteral { properties } => {
                let values = state.stack.pop_many(properties.len())?;
                state
                    .stack
                    .push(self.create_bytecode_object_literal(properties, values)?);
                state.pc = next;
                Ok(None)
            }
            _ => Err(Error::runtime("bytecode creation instruction mismatch")),
        }
    }
}
