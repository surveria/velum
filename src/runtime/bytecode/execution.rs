use crate::{
    SourceSpan,
    bytecode::{BytecodeBlock, BytecodeProgram},
    error::{Error, Result},
    runtime::{Context, control::Completion, control::runtime_exception_value, roots::VmRootKind},
    value::{FunctionId, Value},
};

use super::BytecodeState;

pub(in crate::runtime) enum BytecodeOutcome {
    Completed {
        completion: Completion,
        span: Option<SourceSpan>,
    },
    Suspended {
        awaited: crate::runtime::promise::PromiseId,
        span: Option<SourceSpan>,
    },
}

impl BytecodeOutcome {
    pub(in crate::runtime) fn completion(self) -> Completion {
        match self {
            Self::Completed { completion, .. } => completion,
            Self::Suspended { awaited, .. } => Completion::Suspended(awaited),
        }
    }

    pub(in crate::runtime) const fn span(&self) -> Option<SourceSpan> {
        match self {
            Self::Completed { span, .. } | Self::Suspended { span, .. } => *span,
        }
    }

    pub(in crate::runtime) const fn is_normal(&self) -> bool {
        matches!(
            self,
            Self::Completed {
                completion: Completion::Normal(_),
                ..
            }
        )
    }

    pub(in crate::runtime) const fn is_suspended(&self) -> bool {
        matches!(self, Self::Suspended { .. })
    }
}

impl Context {
    pub(in crate::runtime) fn eval_bytecode_program(
        &mut self,
        bytecode: &BytecodeProgram,
    ) -> Result<BytecodeOutcome> {
        let local_base = self.locals.len();
        let activation_base = self.activation_frames.len();
        let mut state = BytecodeState::new();
        let outcome = self.eval_bytecode_block_outcome_with_state(bytecode.block(), &mut state)?;
        if outcome.is_suspended() {
            self.discard_execution_suffix(local_base, activation_base)?;
            return Err(Error::runtime(
                "top-level await requires an asynchronous evaluation API",
            ));
        }
        Ok(outcome)
    }

    pub(in crate::runtime) fn eval_bytecode_block(
        &mut self,
        block: &BytecodeBlock,
    ) -> Result<Completion> {
        if let Some(completion) = self.take_resumed_bytecode_child(block)? {
            return Ok(completion);
        }
        let mut state = BytecodeState::new();
        self.eval_bytecode_block_outcome_with_state(block, &mut state)
            .map(BytecodeOutcome::completion)
    }

    pub(in crate::runtime) fn eval_bytecode_block_with_state(
        &mut self,
        block: &BytecodeBlock,
        state: &mut BytecodeState,
    ) -> Result<Completion> {
        state.prepare_run()?;
        self.run_bytecode_state(block, state)
            .map(BytecodeOutcome::completion)
    }

    pub(in crate::runtime) fn eval_bytecode_function_body<const CAN_SUSPEND: bool>(
        &mut self,
        function: FunctionId,
        block: &BytecodeBlock,
    ) -> Result<Completion> {
        self.ensure_running_function_continuation(function)?;
        if !CAN_SUSPEND {
            let mut state = BytecodeState::new();
            return self.run_synchronous_bytecode_state(block, &mut state);
        }
        let activation_index = self
            .activation_frames
            .len()
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("function bytecode activation disappeared"))?;
        let mut state = BytecodeState::new();
        state.reset();
        let outcome = self.run_bytecode_state(block, &mut state)?;
        if outcome.is_suspended() {
            self.park_bytecode_state_at(activation_index, state)?;
        }
        Ok(outcome.completion())
    }

    pub(in crate::runtime) fn resume_bytecode_activation(
        &mut self,
        function: FunctionId,
        function_body: &BytecodeBlock,
        mut resume: Option<Completion>,
    ) -> Result<Completion> {
        loop {
            let activation_index = self
                .activation_frames
                .len()
                .checked_sub(1)
                .ok_or_else(|| Error::runtime("suspended bytecode activation disappeared"))?;
            let (program_function, block, mut state) = {
                let continuation = self
                    .activation_frames
                    .last_mut()
                    .map(crate::runtime::activation::ActivationFrame::continuation_mut)
                    .and_then(Option::as_mut)
                    .ok_or_else(|| Error::runtime("suspended bytecode continuation disappeared"))?;
                if let Some(completion) = resume.take() {
                    continuation.resume_await(completion)?;
                }
                let program_function = continuation.function_id();
                let block = continuation.program_block();
                let state = continuation.checkout_state()?;
                (program_function, block, state)
            };
            let run_block = if let Some(block) = &block {
                block
            } else {
                if program_function != Some(function) {
                    return Err(Error::runtime("suspended function continuation mismatch"));
                }
                function_body
            };
            let outcome = self.run_bytecode_state(run_block, &mut state)?;
            if outcome.is_suspended() {
                self.park_bytecode_state_at(activation_index, state)?;
                return Ok(outcome.completion());
            }
            let completion = outcome.completion();
            if block.is_none() {
                return Ok(completion);
            }
            self.finish_resumed_bytecode_child(run_block.clone(), completion)?;
        }
    }

    fn finish_resumed_bytecode_child(
        &mut self,
        block: BytecodeBlock,
        completion: Completion,
    ) -> Result<()> {
        let frame = self
            .activation_frames
            .pop()
            .ok_or_else(|| Error::runtime("resumed bytecode child disappeared"))?;
        if !frame.is_bytecode()
            || frame
                .continuation()
                .is_some_and(|continuation| !continuation.is_settled())
        {
            self.activation_frames.push(frame);
            return Err(Error::runtime("resumed bytecode child owner mismatch"));
        }
        self.storage_ledger
            .release_count(crate::runtime::VmStorageKind::ExecutionFrame, 1)?;
        self.activation_frames
            .last_mut()
            .map(crate::runtime::activation::ActivationFrame::continuation_mut)
            .and_then(Option::as_mut)
            .ok_or_else(|| Error::runtime("resumed bytecode parent disappeared"))?
            .store_resumed_child(block, completion)
    }

    fn eval_bytecode_block_outcome_with_state(
        &mut self,
        block: &BytecodeBlock,
        state: &mut BytecodeState,
    ) -> Result<BytecodeOutcome> {
        state.reset();
        let frame = self.push_bytecode_continuation(block)?;
        let outcome = match self.run_bytecode_state(block, state) {
            Ok(outcome) => outcome,
            Err(error) => {
                if let Err(unwind) = self.pop_bytecode_continuation(frame) {
                    return Err(Error::runtime(format!(
                        "bytecode continuation cleanup failed after '{error}': {unwind}"
                    )));
                }
                return Err(error);
            }
        };
        if outcome.is_suspended() {
            let parked = std::mem::replace(state, BytecodeState::new());
            self.park_bytecode_continuation_state(frame, parked)?;
            return Ok(outcome);
        }
        self.pop_bytecode_continuation(frame)?;
        Ok(outcome)
    }

    fn run_bytecode_state(
        &mut self,
        block: &BytecodeBlock,
        state: &mut BytecodeState,
    ) -> Result<BytecodeOutcome> {
        state.begin_run();
        if let Some(completion) = state.take_resume_completion() {
            return Ok(BytecodeOutcome::Completed {
                completion,
                span: None,
            });
        }
        while let Some(step) = block.step(state.pc)? {
            let span = step.span();
            let _root_scope = if state.has_suspend_state() {
                self.transient_root_scope(VmRootKind::TransientOperand, state.root_values())?
            } else {
                self.transient_root_scope(
                    VmRootKind::TransientOperand,
                    state.synchronous_root_values(),
                )?
            };
            self.step().map_err(|error| error.with_runtime_span(span))?;
            let completion = match self.eval_bytecode_instruction(state, step.instruction()) {
                Ok(completion) => completion,
                Err(error) => self.bytecode_error_completion(error, span)?,
            };
            if let Some(completion) = completion {
                if let Completion::Throw(value) = &completion {
                    self.annotate_error_value_span(value, span)?;
                }
                return Ok(match completion {
                    Completion::Suspended(awaited) => {
                        if !state.is_suspended() {
                            state.mark_child_suspended();
                        }
                        BytecodeOutcome::Suspended {
                            awaited,
                            span: Some(span),
                        }
                    }
                    completion => BytecodeOutcome::Completed {
                        completion,
                        span: Some(span),
                    },
                });
            }
        }
        Ok(BytecodeOutcome::Completed {
            completion: Completion::Normal(state.last.clone()),
            span: None,
        })
    }

    fn run_synchronous_bytecode_state(
        &mut self,
        block: &BytecodeBlock,
        state: &mut BytecodeState,
    ) -> Result<Completion> {
        while let Some(step) = block.step(state.pc)? {
            let span = step.span();
            let _root_scope = self.transient_root_scope(
                VmRootKind::TransientOperand,
                state.synchronous_root_values(),
            )?;
            self.step().map_err(|error| error.with_runtime_span(span))?;
            let completion = match self.eval_bytecode_instruction(state, step.instruction()) {
                Ok(completion) => completion,
                Err(error) => self.bytecode_error_completion(error, span)?,
            };
            if let Some(completion) = completion {
                if let Completion::Throw(value) = &completion {
                    self.annotate_error_value_span(value, span)?;
                }
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(state.last.clone()))
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
