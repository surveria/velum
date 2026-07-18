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
        &mut self,
        state: &mut AttemptState,
        id: usize,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let Some(class) = self.program.classes.get(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        if class.strings.is_empty() {
            return self.consume_codepoint_class(id, instruction, position);
        }
        let positions = self.class_positions_forward(id, position)?;
        let Some(selected) = positions.first().copied() else {
            return Ok(StepOutcome::Failed);
        };
        for alternative in positions.iter().skip(1).rev() {
            self.push_backtrack(state, instruction, *alternative)?;
        }
        Ok(next_step(instruction, selected))
    }

    pub(super) fn consume_class_reverse(
        &mut self,
        state: &mut AttemptState,
        id: usize,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let Some(class) = self.program.classes.get(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        if class.strings.is_empty() {
            return self.consume_codepoint_class_reverse(id, instruction, position);
        }
        let positions = self.class_positions_reverse(id, position)?;
        let Some(selected) = positions.first().copied() else {
            return Ok(StepOutcome::Failed);
        };
        for alternative in positions.iter().skip(1).rev() {
            self.push_backtrack(state, instruction, *alternative)?;
        }
        Ok(next_step(instruction, selected))
    }

    fn consume_codepoint_class(
        &mut self,
        id: usize,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let decoded = decode_forward(self.input, position, self.program.flags.has_unicode_mode())?;
        let Some((actual, next)) = decoded else {
            return Ok(StepOutcome::Failed);
        };
        self.charge_class_codepoint_work(id)?;
        let Some(class) = self.program.classes.get(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        Ok(if class.matches(actual, self.program.flags) {
            next_step(instruction, next)
        } else {
            StepOutcome::Failed
        })
    }

    fn consume_codepoint_class_reverse(
        &mut self,
        id: usize,
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
        self.charge_class_codepoint_work(id)?;
        let Some(class) = self.program.classes.get(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        Ok(if class.matches(actual, self.program.flags) {
            next_step(instruction, previous)
        } else {
            StepOutcome::Failed
        })
    }

    fn class_positions_forward(
        &mut self,
        id: usize,
        position: usize,
    ) -> Result<Vec<usize>, ExecutionError> {
        let decoded = decode_forward(self.input, position, self.program.flags.has_unicode_mode())?;
        let actual = decoded.map(|(value, _)| value);
        let (start, end, work) = self.class_string_candidates(id, actual)?;
        self.charge_steps(work)?;
        let mut positions = Vec::new();
        if let Some((actual, next)) = decoded {
            self.charge_class_codepoint_work(id)?;
            let Some(class) = self.program.classes.get(id) else {
                return Err(ExecutionError::InvalidProgram);
            };
            if class.matches(actual, self.program.flags) {
                positions.push(next);
            }
        }
        let Some(class) = self.program.classes.get(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        let Some(strings) = class.strings.get(start..end) else {
            return Err(ExecutionError::InvalidProgram);
        };
        for string in strings {
            if let Some(end_position) = self.string_end_forward(string, position)? {
                positions.push(end_position);
            }
        }
        positions.sort_unstable_by(|left, right| right.cmp(left));
        positions.dedup();
        Ok(positions)
    }

    fn class_positions_reverse(
        &mut self,
        id: usize,
        position: usize,
    ) -> Result<Vec<usize>, ExecutionError> {
        let decoded = decode_backward_with_position(
            self.input,
            position,
            self.program.flags.has_unicode_mode(),
        )?;
        let work = {
            let Some(class) = self.program.classes.get(id) else {
                return Err(ExecutionError::InvalidProgram);
            };
            class
                .strings
                .iter()
                .try_fold(class.strings.len(), |total, string| {
                    total
                        .checked_add(string.len())
                        .ok_or(ExecutionError::SizeOverflow)
                })?
        };
        self.charge_steps(work)?;
        let mut positions = Vec::new();
        if let Some((actual, previous)) = decoded {
            self.charge_class_codepoint_work(id)?;
            let Some(class) = self.program.classes.get(id) else {
                return Err(ExecutionError::InvalidProgram);
            };
            if class.matches(actual, self.program.flags) {
                positions.push(previous);
            }
        }
        let Some(class) = self.program.classes.get(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        for string in &class.strings {
            if let Some(start_position) = self.string_end_reverse(string, position)? {
                positions.push(start_position);
            }
        }
        positions.sort_unstable();
        positions.dedup();
        Ok(positions)
    }

    fn class_string_candidates(
        &self,
        id: usize,
        actual: Option<u32>,
    ) -> Result<(usize, usize, usize), ExecutionError> {
        let Some(class) = self.program.classes.get(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        if self.program.flags.ignore_case()
            || actual.is_none()
            || class
                .strings
                .first()
                .is_some_and(|string| string.is_empty())
        {
            let work = class
                .strings
                .iter()
                .try_fold(class.strings.len(), |total, string| {
                    total
                        .checked_add(string.len())
                        .ok_or(ExecutionError::SizeOverflow)
                })?;
            return Ok((0, class.strings.len(), work));
        }
        let actual = actual.ok_or(ExecutionError::InvalidProgram)?;
        let start = class
            .strings
            .partition_point(|string| string.first().is_some_and(|first| *first < actual));
        let remaining = class
            .strings
            .get(start..)
            .ok_or(ExecutionError::InvalidProgram)?;
        let count = remaining
            .iter()
            .take_while(|string| string.first().is_some_and(|first| *first == actual))
            .count();
        let end = start
            .checked_add(count)
            .ok_or(ExecutionError::SizeOverflow)?;
        let search_work = usize::try_from(usize::BITS - class.strings.len().leading_zeros())
            .map_err(|_| ExecutionError::SizeOverflow)?;
        let work = class
            .strings
            .get(start..end)
            .ok_or(ExecutionError::InvalidProgram)?
            .iter()
            .try_fold(search_work, |total, string| {
                total
                    .checked_add(string.len())
                    .ok_or(ExecutionError::SizeOverflow)
            })?;
        Ok((start, end, work))
    }

    fn charge_class_codepoint_work(&mut self, id: usize) -> Result<(), ExecutionError> {
        let Some(work) = self
            .program
            .classes
            .get(id)
            .map(|class| class.codepoint_work)
        else {
            return Err(ExecutionError::InvalidProgram);
        };
        if work == 0 {
            return Ok(());
        }
        self.charge_steps(work)
    }

    fn string_end_forward(
        &self,
        string: &[u32],
        position: usize,
    ) -> Result<Option<usize>, ExecutionError> {
        let mut current = position;
        for expected in string {
            let Some((actual, next)) =
                decode_forward(self.input, current, self.program.flags.has_unicode_mode())?
            else {
                return Ok(None);
            };
            if !self.characters_equal(actual, *expected) {
                return Ok(None);
            }
            current = next;
        }
        Ok(Some(current))
    }

    fn string_end_reverse(
        &self,
        string: &[u32],
        position: usize,
    ) -> Result<Option<usize>, ExecutionError> {
        let mut current = position;
        for expected in string.iter().rev() {
            let Some((actual, previous)) = decode_backward_with_position(
                self.input,
                current,
                self.program.flags.has_unicode_mode(),
            )?
            else {
                return Ok(None);
            };
            if !self.characters_equal(actual, *expected) {
                return Ok(None);
            }
            current = previous;
        }
        Ok(Some(current))
    }
}
