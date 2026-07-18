use crate::{
    ExecutionControl, ExecutionError,
    input::{decode_backward_with_position, decode_forward, is_line_terminator},
    program::InstructionIndex,
};

use super::{AttemptState, Executor, StepOutcome, next_step};

impl<C: ExecutionControl> Executor<'_, C> {
    pub(super) fn consume_char(
        &self,
        expected: u32,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let decoded = decode_forward(self.input, position, self.program.flags.has_unicode_mode())?;
        Ok(match decoded {
            Some((actual, next)) if self.characters_equal(actual, expected) => {
                next_step(instruction, next)
            }
            Some(_) | None => StepOutcome::Failed,
        })
    }

    pub(super) fn consume_char_reverse(
        &self,
        expected: u32,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let decoded = decode_backward_with_position(
            self.input,
            position,
            self.program.flags.has_unicode_mode(),
        )?;
        Ok(match decoded {
            Some((actual, previous)) if self.characters_equal(actual, expected) => {
                next_step(instruction, previous)
            }
            Some(_) | None => StepOutcome::Failed,
        })
    }

    pub(super) fn consume_any(
        &self,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let decoded = decode_forward(self.input, position, self.program.flags.has_unicode_mode())?;
        let Some((actual, next)) = decoded else {
            return Ok(StepOutcome::Failed);
        };
        let line_terminator = u16::try_from(actual).is_ok_and(is_line_terminator);
        if line_terminator && !self.program.flags.dot_all() {
            Ok(StepOutcome::Failed)
        } else {
            Ok(next_step(instruction, next))
        }
    }

    pub(super) fn consume_any_reverse(
        &self,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let decoded = decode_backward_with_position(
            self.input,
            position,
            self.program.flags.has_unicode_mode(),
        )?;
        let Some((actual, previous)) = decoded else {
            return Ok(StepOutcome::Failed);
        };
        let line_terminator = u16::try_from(actual).is_ok_and(is_line_terminator);
        if line_terminator && !self.program.flags.dot_all() {
            Ok(StepOutcome::Failed)
        } else {
            Ok(next_step(instruction, previous))
        }
    }

    pub(super) fn consume_backreference(
        &mut self,
        state: &AttemptState,
        id: usize,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let Some(slot) = state.captures.get(id).copied() else {
            return Err(ExecutionError::InvalidProgram);
        };
        let Some(span) = slot.span() else {
            return Ok(next_step(instruction, position));
        };
        let mut capture_position = span.start;
        let mut input_position = position;
        while capture_position < span.end {
            self.charge_step()?;
            let Some((expected, next_capture)) = decode_forward(
                self.input,
                capture_position,
                self.program.flags.has_unicode_mode(),
            )?
            else {
                return Err(ExecutionError::InvalidProgram);
            };
            if next_capture > span.end {
                return Err(ExecutionError::InvalidProgram);
            }
            let Some((actual, next_input)) = decode_forward(
                self.input,
                input_position,
                self.program.flags.has_unicode_mode(),
            )?
            else {
                return Ok(StepOutcome::Failed);
            };
            if !self.characters_equal(actual, expected) {
                return Ok(StepOutcome::Failed);
            }
            capture_position = next_capture;
            input_position = next_input;
        }
        Ok(next_step(instruction, input_position))
    }

    pub(super) fn consume_backreference_reverse(
        &mut self,
        state: &AttemptState,
        id: usize,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let Some(slot) = state.captures.get(id).copied() else {
            return Err(ExecutionError::InvalidProgram);
        };
        let Some(span) = slot.span() else {
            return Ok(next_step(instruction, position));
        };
        let mut capture_position = span.end;
        let mut input_position = position;
        while capture_position > span.start {
            self.charge_step()?;
            let Some((expected, previous_capture)) = decode_backward_with_position(
                self.input,
                capture_position,
                self.program.flags.has_unicode_mode(),
            )?
            else {
                return Err(ExecutionError::InvalidProgram);
            };
            if previous_capture < span.start {
                return Err(ExecutionError::InvalidProgram);
            }
            let Some((actual, previous_input)) = decode_backward_with_position(
                self.input,
                input_position,
                self.program.flags.has_unicode_mode(),
            )?
            else {
                return Ok(StepOutcome::Failed);
            };
            if !self.characters_equal(actual, expected) {
                return Ok(StepOutcome::Failed);
            }
            capture_position = previous_capture;
            input_position = previous_input;
        }
        Ok(next_step(instruction, input_position))
    }

    pub(super) fn consume_class(
        &self,
        id: usize,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let Some(class) = self.program.classes.get(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        let decoded = decode_forward(self.input, position, self.program.flags.has_unicode_mode())?;
        Ok(match decoded {
            Some((actual, next)) if class.matches(actual, self.program.flags) => {
                next_step(instruction, next)
            }
            Some(_) | None => StepOutcome::Failed,
        })
    }

    pub(super) fn consume_class_reverse(
        &self,
        id: usize,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let Some(class) = self.program.classes.get(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        let decoded = decode_backward_with_position(
            self.input,
            position,
            self.program.flags.has_unicode_mode(),
        )?;
        Ok(match decoded {
            Some((actual, previous)) if class.matches(actual, self.program.flags) => {
                next_step(instruction, previous)
            }
            Some(_) | None => StepOutcome::Failed,
        })
    }
}
