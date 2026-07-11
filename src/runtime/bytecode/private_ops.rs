use crate::{
    bytecode::{BytecodeAddress, BytecodeInstruction, BytecodePrivateName},
    error::{Error, Result},
    runtime::{Context, control::Completion},
    value::Value,
};

use super::state::BytecodeState;

impl Context {
    pub(super) fn eval_bytecode_private_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::PrivateMember { property } => {
                let name = self.resolve_private_name(property)?;
                let receiver = state.stack.pop()?;
                let value = self.read_private_slot(&receiver, name)?;
                state.stack.push(value);
            }
            BytecodeInstruction::PrivateAssign { property } => {
                let name = self.resolve_private_name(property)?;
                let value = state.stack.pop()?;
                let receiver = state.stack.pop()?;
                self.write_private_slot(&receiver, name, value.clone())?;
                state.stack.push(value);
            }
            BytecodeInstruction::CompoundPrivateProperty { property, op } => {
                let name = self.resolve_private_name(property)?;
                let right = state.stack.pop()?;
                let receiver = state.stack.pop()?;
                let left = self.read_private_slot(&receiver, name)?;
                let value = self.eval_bytecode_binary(*op, &left, &right, None)?;
                self.write_private_slot(&receiver, name, value.clone())?;
                state.stack.push(value);
            }
            BytecodeInstruction::UpdatePrivateProperty {
                property,
                op,
                prefix,
            } => {
                let name = self.resolve_private_name(property)?;
                let receiver = state.stack.pop()?;
                let old = self.read_private_slot(&receiver, name)?;
                let updated = Self::updated_bytecode_number(&old, *op)?;
                self.write_private_slot(&receiver, name, updated.clone())?;
                state.stack.push(if *prefix { updated } else { old });
            }
            BytecodeInstruction::PrivateIn { property } => {
                let name = self.resolve_private_name(property)?;
                let receiver = state.stack.pop()?;
                state
                    .stack
                    .push(Value::Bool(self.has_private_slot(&receiver, name)?));
            }
            _ => return Err(Error::runtime("private bytecode instruction mismatch")),
        }
        state.pc = next;
        Ok(None)
    }

    pub(super) fn eval_bytecode_call_private_member(
        &mut self,
        state: &mut BytecodeState,
        property: &BytecodePrivateName,
        arg_count: usize,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let name = self.resolve_private_name(property)?;
        let args = state.stack.tail(arg_count)?.to_vec();
        let receiver = state.stack.value_before_tail(arg_count, 0)?.clone();
        let callee = self.read_private_slot(&receiver, name)?;
        let completion = self.call(&callee, &args, receiver)?;
        let Completion::Normal(value) = completion else {
            return Ok(Some(completion));
        };
        state.stack.drop_tail(arg_count)?;
        state.stack.pop()?;
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    pub(super) fn eval_bytecode_call_private_member_spread(
        &mut self,
        state: &mut BytecodeState,
        property: &BytecodePrivateName,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let name = self.resolve_private_name(property)?;
        let packed = state.stack.pop()?;
        let args = self.spread_call_arguments(&packed)?;
        let receiver = state.stack.pop()?;
        let callee = self.read_private_slot(&receiver, name)?;
        let completion = self.call(&callee, &args, receiver)?;
        let Completion::Normal(value) = completion else {
            return Ok(Some(completion));
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }
}
