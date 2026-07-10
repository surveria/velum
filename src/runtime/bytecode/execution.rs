use crate::{
    SourceSpan,
    bytecode::{BytecodeBlock, BytecodeProgram},
    error::{Error, Result},
    runtime::{Context, control::Completion, control::runtime_exception_value},
    value::Value,
};

use super::BytecodeState;

pub(in crate::runtime) struct BytecodeOutcome {
    completion: Completion,
    span: Option<SourceSpan>,
}

impl BytecodeOutcome {
    pub(in crate::runtime) fn completion(self) -> Completion {
        self.completion
    }

    pub(in crate::runtime) const fn span(&self) -> Option<SourceSpan> {
        self.span
    }

    pub(in crate::runtime) const fn is_normal(&self) -> bool {
        matches!(self.completion, Completion::Normal(_))
    }
}

impl Context {
    pub(in crate::runtime) fn eval_bytecode_program(
        &mut self,
        bytecode: &BytecodeProgram,
    ) -> Result<BytecodeOutcome> {
        let mut state = BytecodeState::new();
        self.eval_bytecode_block_outcome_with_state(bytecode.block(), &mut state)
    }

    pub(in crate::runtime) fn eval_bytecode_block(
        &mut self,
        block: &BytecodeBlock,
    ) -> Result<Completion> {
        let mut state = BytecodeState::new();
        self.eval_bytecode_block_outcome_with_state(block, &mut state)
            .map(BytecodeOutcome::completion)
    }

    pub(in crate::runtime) fn eval_bytecode_block_with_state(
        &mut self,
        block: &BytecodeBlock,
        state: &mut BytecodeState,
    ) -> Result<Completion> {
        self.eval_bytecode_block_outcome_with_state(block, state)
            .map(BytecodeOutcome::completion)
    }

    fn eval_bytecode_block_outcome_with_state(
        &mut self,
        block: &BytecodeBlock,
        state: &mut BytecodeState,
    ) -> Result<BytecodeOutcome> {
        state.reset();
        while let Some(step) = block.step(state.pc)? {
            let span = step.span();
            self.step().map_err(|error| error.with_runtime_span(span))?;
            let completion = match self.eval_bytecode_instruction(state, step.instruction()) {
                Ok(completion) => completion,
                Err(error) => self.bytecode_error_completion(error, span)?,
            };
            if let Some(completion) = completion {
                if let Completion::Throw(value) = &completion {
                    self.annotate_error_value_span(value, span)?;
                }
                return Ok(BytecodeOutcome {
                    completion,
                    span: Some(span),
                });
            }
        }
        Ok(BytecodeOutcome {
            completion: Completion::Normal(state.last.clone()),
            span: None,
        })
    }

    pub(super) fn eval_bytecode_expression(&mut self, block: &BytecodeBlock) -> Result<Value> {
        self.eval_bytecode_block(block)?.into_result()
    }

    pub(in crate::runtime::bytecode) fn bytecode_error_completion(
        &mut self,
        error: Error,
        span: SourceSpan,
    ) -> Result<Option<Completion>> {
        let error = error.with_runtime_span(span);
        let Some(value) = runtime_exception_value(self, &error)
            .map_err(|conversion| conversion.with_runtime_span(span))?
        else {
            return Err(error);
        };
        self.checked_value(value.clone())
            .map_err(|validation| validation.with_runtime_span(span))?;
        self.annotate_error_value_span(&value, span)?;
        Ok(Some(Completion::Throw(value)))
    }

    pub(in crate::runtime::bytecode) fn annotate_error_value_span(
        &mut self,
        value: &Value,
        span: SourceSpan,
    ) -> Result<()> {
        let Value::Object(id) = value else {
            return Ok(());
        };
        self.objects
            .set_error_source_span_if_missing(*id, span)
            .map_err(|error| error.with_runtime_span(span))
    }
}
