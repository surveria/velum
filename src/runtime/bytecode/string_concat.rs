use super::state::BytecodeState;
use crate::{
    bytecode::{BytecodeAddress, BytecodeInstruction},
    error::{Error, Result},
    runtime::{Context, control::Completion},
    value::Value,
};

impl Context {
    pub(super) fn eval_bytecode_string_concat_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::StringConcat { final_result } => {
                self.eval_bytecode_string_concat(state, *final_result, next)
            }
            BytecodeInstruction::StringConcatStatic { text, final_result } => {
                self.eval_bytecode_string_concat_static(state, text.as_str(), *final_result, next)
            }
            _ => Err(Error::runtime("bytecode string concat op mismatch")),
        }
    }

    pub(super) fn eval_bytecode_string_concat(
        &mut self,
        state: &mut BytecodeState,
        final_result: bool,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let right = state.stack.pop()?;
        let left = state.stack.pop()?;
        let value = if self.optional_optimizations_enabled() {
            self.string_concat_step(left, &right, final_result)?
        } else {
            self.add(&left, &right)?
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    pub(super) fn eval_bytecode_string_concat_static(
        &mut self,
        state: &mut BytecodeState,
        text: &str,
        final_result: bool,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let left = state.stack.pop()?;
        let value = if self.optional_optimizations_enabled() {
            self.string_concat_static_step(left, text, final_result)?
        } else {
            self.add(&left, &Value::String(text.to_owned()))?
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }
}
