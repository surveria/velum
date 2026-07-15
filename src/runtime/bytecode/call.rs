use crate::{
    api::native_call::NativeCallTarget,
    bytecode::{
        BytecodeAddress, BytecodeCallSite, BytecodeDynamicProperty, BytecodeInstruction,
        BytecodeObjectProperty, BytecodePreparedNativeCall, BytecodeProperty,
    },
    error::{Error, Result},
    runtime::{CallReference, Context, control::Completion, function::BytecodeFunctionInit},
    value::Value,
};

use super::state::BytecodeState;

impl Context {
    fn eval_bytecode_identifier_tail_call(
        &mut self,
        callee: &crate::bytecode::BytecodeBinding,
        native: Option<NativeCallTarget>,
        strict: bool,
        args: &[Value],
    ) -> Result<Completion> {
        let reference = self.eval_bytecode_identifier_call_reference(callee, native, strict)?;
        if matches!(
            &reference,
            CallReference::DirectNative {
                target: NativeCallTarget::Eval,
                ..
            }
        ) {
            return match self.eval_call_reference_completion(reference, args)? {
                Completion::Normal(value) => Ok(Completion::Return(value)),
                Completion::Throw(value) => Ok(Completion::Throw(value)),
                other => Err(Error::runtime(format!(
                    "direct eval tail call produced invalid completion {other:?}"
                ))),
            };
        }
        Ok(Completion::TailCall(
            reference.into_tail_call(args.to_vec()),
        ))
    }

    pub(super) fn eval_bytecode_call_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::CallBinding { .. }
            | BytecodeInstruction::TailCallBinding { .. }
            | BytecodeInstruction::CallValue { .. }
            | BytecodeInstruction::CallValueWithReceiver { .. }
            | BytecodeInstruction::TailCallValue { .. }
            | BytecodeInstruction::TailCallValueWithReceiver { .. }
            | BytecodeInstruction::CallStaticMember { .. }
            | BytecodeInstruction::CallComputedMember { .. } => {
                self.eval_bytecode_invocation_instruction(state, instruction, next)
            }
            BytecodeInstruction::CallPrivateMember {
                property,
                arg_count,
            } => self.eval_bytecode_call_private_member(state, property, *arg_count, next),
            BytecodeInstruction::CollectSpreadArgs { spread_flags } => {
                self.eval_bytecode_collect_spread_args(state, spread_flags, next)
            }
            BytecodeInstruction::ArrayLiteralSpread {
                spread_flags,
                holes,
            } => self.eval_bytecode_array_literal_spread(state, spread_flags, holes, next),
            BytecodeInstruction::CallBindingSpread {
                callee,
                native,
                strict,
            } => self.eval_bytecode_call_binding_spread(state, callee, *native, *strict, next),
            BytecodeInstruction::CallValueSpread => {
                self.eval_bytecode_call_value_spread(state, next)
            }
            BytecodeInstruction::CallValueWithReceiverSpread => {
                self.eval_bytecode_call_value_with_receiver_spread(state, next)
            }
            BytecodeInstruction::CallStaticMemberSpread { property } => {
                self.eval_bytecode_call_static_member_spread(state, property, next)
            }
            BytecodeInstruction::CallComputedMemberSpread { property } => {
                self.eval_bytecode_call_computed_member_spread(state, *property, next)
            }
            BytecodeInstruction::CallPrivateMemberSpread { property } => {
                self.eval_bytecode_call_private_member_spread(state, property, next)
            }
            BytecodeInstruction::ConstructValueSpread => {
                self.eval_bytecode_construct_value_spread(state, next)
            }
            BytecodeInstruction::CreateClass { class } => {
                let result = self.eval_bytecode_create_class(state, class, next);
                self.leave_private_environment(state)?;
                result
            }
            BytecodeInstruction::PrepareSuperConstructor => {
                self.eval_bytecode_prepare_super_constructor(state, next)
            }
            BytecodeInstruction::CallSuper { arg_count } => {
                self.eval_bytecode_call_super(state, *arg_count, next)
            }
            BytecodeInstruction::CallSuperSpread => {
                self.eval_bytecode_call_super_spread(state, next)
            }
            BytecodeInstruction::SuperMember { property } => {
                self.eval_bytecode_super_member(state, property, next)
            }
            BytecodeInstruction::ComputedSuperMember {
                expression,
                property,
            } => self.eval_bytecode_computed_super_member(state, expression, *property, next),
            BytecodeInstruction::CallSuperMember {
                property,
                arg_count,
            } => self.eval_bytecode_call_super_member(state, property, *arg_count, next),
            BytecodeInstruction::CallSuperMemberSpread { property } => {
                self.eval_bytecode_call_super_member_spread(state, property, next)
            }
            BytecodeInstruction::CallComputedSuperMember {
                property,
                arg_count,
            } => self.eval_bytecode_call_computed_super_member(state, *property, *arg_count, next),
            BytecodeInstruction::CallComputedSuperMemberSpread { property } => {
                self.eval_bytecode_call_computed_super_member_spread(state, *property, next)
            }
            BytecodeInstruction::SuperPropertyAssign { .. }
            | BytecodeInstruction::UpdateSuperProperty { .. }
            | BytecodeInstruction::CompoundSuperProperty { .. } => {
                self.eval_bytecode_super_mutation_instruction(state, instruction, next)
            }
            BytecodeInstruction::Construct { .. }
            | BytecodeInstruction::ConstructValue { .. }
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
            BytecodeInstruction::TailCallBinding {
                callee,
                native,
                strict,
                arg_count,
            } => {
                let args = state.stack.tail(*arg_count)?;
                self.eval_bytecode_identifier_tail_call(callee, *native, *strict, args)
                    .map(Some)
            }
            BytecodeInstruction::TailCallValue { arg_count } => {
                let args = state.stack.tail(*arg_count)?.to_vec();
                let callee = state.stack.value_before_tail(*arg_count, 0)?.clone();
                Ok(Some(Completion::TailCall(
                    crate::runtime::control::TailCall::new(callee, args, Value::Undefined),
                )))
            }
            BytecodeInstruction::TailCallValueWithReceiver { arg_count } => {
                self.eval_bytecode_tail_call_value_with_receiver(state, *arg_count)
            }
            BytecodeInstruction::CallBinding {
                callee,
                native,
                strict,
                arg_count,
            } => {
                let args = state.stack.tail(*arg_count)?;
                let completion =
                    self.eval_bytecode_identifier_call_completion(callee, *native, *strict, args)?;
                let Completion::Normal(value) = completion else {
                    return Ok(Some(completion));
                };
                state.stack.drop_tail(*arg_count)?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CallValue { site, arg_count } => {
                let args = state.stack.tail(*arg_count)?;
                let callee = state.stack.value_before_tail(*arg_count, 0)?.clone();
                let completion =
                    self.eval_cached_call_completion(*site, &callee, args, Value::Undefined)?;
                let Completion::Normal(value) = completion else {
                    return Ok(Some(completion));
                };
                state.stack.drop_tail(*arg_count)?;
                state.stack.pop()?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CallValueWithReceiver {
                site,
                native,
                arg_count,
            } => {
                self.eval_bytecode_call_value_with_receiver(state, *site, *native, *arg_count, next)
            }
            BytecodeInstruction::CallStaticMember {
                property,
                native,
                arg_count,
            } => {
                let completion = self.eval_bytecode_static_member_call_completion(
                    state, property, *native, *arg_count,
                )?;
                let Completion::Normal(value) = completion else {
                    return Ok(Some(completion));
                };
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
                let completion = self.eval_bytecode_computed_member_call_completion(
                    state, *operand, *native, *arg_count,
                )?;
                let Completion::Normal(value) = completion else {
                    return Ok(Some(completion));
                };
                state.pc = next;
                state.stack.push(value);
                Ok(None)
            }
            BytecodeInstruction::Construct { .. }
            | BytecodeInstruction::ConstructValue { .. }
            | BytecodeInstruction::CreateFunction { .. }
            | BytecodeInstruction::ArrayLiteral { .. }
            | BytecodeInstruction::ObjectLiteral { .. } => {
                self.eval_bytecode_creation_instruction(state, instruction, next)
            }
            _ => Err(Error::runtime("bytecode invocation instruction mismatch")),
        }
    }

    fn eval_bytecode_tail_call_value_with_receiver(
        &mut self,
        state: &mut BytecodeState,
        arg_count: usize,
    ) -> Result<Option<Completion>> {
        let args = state.stack.tail(arg_count)?.to_vec();
        let callee = state.stack.value_before_tail(arg_count, 0)?.clone();
        let receiver = state.stack.value_before_tail(arg_count, 1)?.clone();
        Ok(Some(Completion::TailCall(
            crate::runtime::control::TailCall::new(callee, args, receiver),
        )))
    }

    fn eval_bytecode_call_value_with_receiver(
        &mut self,
        state: &mut BytecodeState,
        site: BytecodeCallSite,
        native: Option<BytecodePreparedNativeCall>,
        arg_count: usize,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let args = state.stack.tail(arg_count)?;
        let callee = state.stack.value_before_tail(arg_count, 0)?.clone();
        let receiver = state.stack.value_before_tail(arg_count, 1)?.clone();
        let completion = match native {
            Some(BytecodePreparedNativeCall::Direct { target, access }) => self
                .eval_direct_native_property_call(target, access, &callee, args, &receiver)
                .map(Completion::Normal)?,
            Some(BytecodePreparedNativeCall::Cached { access }) => self
                .eval_cached_native_property_call(access, &callee, args, &receiver)
                .map(Completion::Normal)?,
            None => self.eval_cached_call_completion(site, &callee, args, receiver)?,
        };
        let Completion::Normal(value) = completion else {
            return Ok(Some(completion));
        };
        state.stack.drop_tail(arg_count)?;
        state.stack.pop()?;
        state.stack.pop()?;
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_computed_member_call_completion(
        &mut self,
        state: &mut BytecodeState,
        operand: BytecodeDynamicProperty,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    ) -> Result<Completion> {
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
            return Ok(Completion::Normal(value));
        }
        let callee = self.get_cached_dynamic_property_value(this_value, &key, operand.access())?;
        let completion = if let Some(target) = native {
            let value = self.eval_direct_native_property_call(
                target,
                operand.access(),
                &callee,
                args,
                this_value,
            )?;
            Completion::Normal(value)
        } else {
            self.call(&callee, args, this_value.clone())?
        };
        let Completion::Normal(value) = completion else {
            return Ok(completion);
        };
        state.stack.drop_tail(arg_count)?;
        state.stack.pop()?;
        state.stack.pop()?;
        Ok(Completion::Normal(value))
    }

    fn eval_bytecode_static_member_call_completion(
        &mut self,
        state: &BytecodeState,
        property: &BytecodeProperty,
        native: Option<NativeCallTarget>,
        arg_count: usize,
    ) -> Result<Completion> {
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
                return Ok(Completion::Normal(value));
            }
            let callee =
                self.get_static_property_value(this_value, property.name(), property.access())?;
            return self
                .eval_direct_native_property_call(
                    target,
                    property.access(),
                    &callee,
                    args,
                    this_value,
                )
                .map(Completion::Normal);
        }
        let callee =
            self.get_static_property_value(this_value, property.name(), property.access())?;
        self.call(&callee, args, this_value.clone())
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
            BytecodeInstruction::ConstructValue { native, arg_count } => {
                let args = state.stack.tail(*arg_count)?;
                let constructor = state.stack.value_before_tail(*arg_count, 0)?.clone();
                let value = self.eval_new_value_with_native(&constructor, *native, args)?;
                state.stack.drop_tail(*arg_count)?;
                state.stack.pop()?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::CreateFunction {
                id,
                name,
                bytecode,
                constructable,
                kind,
                new_target_mode,
            } => {
                let function = self.create_bytecode_function(&BytecodeFunctionInit {
                    static_function_id: *id,
                    name: name.as_ref(),
                    bytecode,
                    constructable: *constructable,
                    kind: *kind,
                    class_constructor: false,
                    prototype_parent: None,
                    new_target_mode: *new_target_mode,
                })?;
                state.stack.push(function);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::ArrayLiteral { len, holes } => {
                let value = {
                    let value_count =
                        len.saturating_sub(holes.iter().filter(|hole| **hole).count());
                    let values = state.stack.drain_tail(value_count)?;
                    self.create_array_literal_from_elements(values, *len, holes)?
                };
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::ObjectLiteral { properties } => {
                let value_count = object_literal_stack_value_count(properties)?;
                let values = state.stack.pop_many(value_count)?;
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

fn object_literal_stack_value_count(properties: &[BytecodeObjectProperty]) -> Result<usize> {
    let mut count = 0_usize;
    for property in properties {
        count = count
            .checked_add(property.stack_value_count())
            .ok_or_else(|| Error::limit("object literal stack value count overflowed"))?;
    }
    Ok(count)
}
