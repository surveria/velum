use crate::{
    api::native_call::NativeCallTarget,
    bytecode::{BytecodeAddress, BytecodeDynamicProperty, BytecodeInstruction, BytecodeProperty},
    error::{Error, Result},
    runtime::{
        Context, call_args::RuntimeCallArgs, completion::Completion, function::BytecodeFunctionInit,
    },
    value::Value,
};

use super::state::BytecodeState;

impl Context {
    pub(super) fn eval_bytecode_call_instruction(
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
            BytecodeInstruction::CallBinding {
                callee,
                native,
                arg_count,
            } => {
                let args = state.stack.tail(*arg_count)?;
                let value = self.eval_bytecode_identifier_call_value(callee, *native, args)?;
                state.stack.drop_tail(*arg_count)?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CallValue { site, arg_count } => {
                let args = state.stack.tail(*arg_count)?;
                let callee = state.stack.value_before_tail(*arg_count, 0)?.clone();
                let value = self.eval_cached_call_value(*site, callee, args, Value::Undefined)?;
                state.stack.drop_tail(*arg_count)?;
                state.stack.pop()?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CallStaticMember {
                property,
                native,
                arg_count,
            } => {
                let value =
                    self.eval_bytecode_static_member_call(state, property, *native, *arg_count)?;
                state.stack.drop_tail(*arg_count)?;
                state.stack.pop()?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CallComputedMember {
                property: operand,
                native,
                arg_count,
            } => {
                let value =
                    self.eval_bytecode_computed_member_call(state, *operand, *native, *arg_count)?;
                state.pc = next;
                state.stack.push(value);
                Ok(None)
            }
            BytecodeInstruction::Print { arg_count } => {
                let args = state.stack.tail(*arg_count)?;
                let value = self.eval_print_call(RuntimeCallArgs::values(args))?;
                state.stack.drop_tail(*arg_count)?;
                state.stack.push(value);
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

    fn eval_bytecode_computed_member_call(
        &mut self,
        state: &mut BytecodeState,
        operand: BytecodeDynamicProperty,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    ) -> Result<Value> {
        let args = state.stack.tail(arg_count)?;
        let property = state.stack.value_before_tail(arg_count, 0)?;
        let this_value = state.stack.value_before_tail(arg_count, 1)?;
        let key = self.dynamic_property_key(property)?;
        if native.is_none()
            && let Some(value) = self.eval_cached_native_dynamic_member_call(
                &key,
                operand.access(),
                args,
                this_value,
            )?
        {
            state.stack.drop_tail(arg_count)?;
            state.stack.pop()?;
            state.stack.pop()?;
            return Ok(value);
        }
        let callee = self.get_cached_dynamic_property_value(this_value, &key, operand.access())?;
        let value = if let Some(target) = native {
            self.eval_direct_native_property_call(
                target,
                operand.access(),
                callee,
                args,
                this_value,
            )?
        } else {
            self.eval_call_value(callee, args, this_value.clone())?
        };
        state.stack.drop_tail(arg_count)?;
        state.stack.pop()?;
        state.stack.pop()?;
        Ok(value)
    }

    fn eval_bytecode_static_member_call(
        &mut self,
        state: &BytecodeState,
        property: &BytecodeProperty,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    ) -> Result<Value> {
        let args = state.stack.tail(arg_count)?;
        let this_value = state.stack.value_before_tail(arg_count, 0)?;
        if let Some(target) = native {
            if let Some(value) = self.eval_cached_direct_native_static_member_call(
                target,
                property.name(),
                property.access(),
                args,
                this_value,
            )? {
                return Ok(value);
            }
            let callee =
                self.get_static_property_value(this_value, property.name(), property.access())?;
            return self.eval_direct_native_property_call(
                target,
                property.access(),
                callee,
                args,
                this_value,
            );
        }
        let callee =
            self.get_static_property_value(this_value, property.name(), property.access())?;
        self.eval_call_value(callee, args, this_value.clone())
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
                native,
                arg_count,
            } => {
                let args = state.stack.tail(*arg_count)?;
                let value = self.eval_bytecode_new_value(constructor, *native, args)?;
                state.stack.drop_tail(*arg_count)?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CreateFunction {
                id,
                name,
                params,
                bytecode,
                constructable,
                is_async,
                new_target_mode,
            } => {
                let function = self.create_bytecode_function(&BytecodeFunctionInit {
                    static_function_id: *id,
                    name: name.as_ref(),
                    params,
                    bytecode,
                    constructable: *constructable,
                    is_async: *is_async,
                    new_target_mode: *new_target_mode,
                })?;
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
