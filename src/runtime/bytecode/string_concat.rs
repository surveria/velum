use super::state::BytecodeState;
use crate::{
    bytecode::{BytecodeAddress, BytecodeInstruction},
    error::{Error, Result},
    runtime::{Context, control::Completion},
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
        state
            .stack
            .push(self.string_concat_step(left, &right, final_result)?);
        state.pc = next;
        Ok(None)
    }

    pub(super) fn eval_bytecode_string_concat_static(
        &self,
        state: &mut BytecodeState,
        text: &str,
        final_result: bool,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let left = state.stack.pop()?;
        state
            .stack
            .push(self.string_concat_static_step(left, text, final_result)?);
        state.pc = next;
        Ok(None)
    }
}
