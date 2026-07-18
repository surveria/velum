use crate::{
    Capture, ExecutionControl, ExecutionError, ExecutionLimits, ExecutionStats, Match,
    SearchOutcome,
    character_class::is_word_character,
    input::{advance_candidate, decode_backward, decode_forward, is_line_terminator},
    program::{Instruction, InstructionIndex, Program},
};

mod consume;

#[derive(Debug, Clone, Copy, Default)]
struct CaptureSlot {
    start: Option<usize>,
    end: Option<usize>,
}

impl CaptureSlot {
    fn span(self) -> Option<core::ops::Range<usize>> {
        self.start.zip(self.end).map(|(start, end)| start..end)
    }
}

#[derive(Debug)]
enum UndoRecord {
    Capture { id: usize, previous: CaptureSlot },
    Progress { id: usize, previous: Option<usize> },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum BacktrackFrameKind {
    Branch,
    PositiveLookahead,
    NegativeLookahead,
}

#[derive(Debug, Clone, Copy)]
struct BacktrackFrame {
    instruction: InstructionIndex,
    position: usize,
    undo_depth: usize,
    kind: BacktrackFrameKind,
}

enum StepOutcome {
    Accept,
    Next {
        instruction: InstructionIndex,
        position: usize,
    },
    Failed,
}

#[derive(Debug)]
struct AttemptState {
    captures: Vec<CaptureSlot>,
    progress: Vec<Option<usize>>,
    undo: Vec<UndoRecord>,
    backtrack: Vec<BacktrackFrame>,
}

impl AttemptState {
    fn new(program: &Program) -> Self {
        Self {
            captures: vec![CaptureSlot::default(); program.capture_count],
            progress: vec![None; program.progress_count],
            undo: Vec::new(),
            backtrack: Vec::new(),
        }
    }
}

pub struct Executor<'a, C> {
    program: &'a Program,
    input: &'a [u16],
    limits: ExecutionLimits,
    control: &'a mut C,
    stats: ExecutionStats,
}

impl<'a, C: ExecutionControl> Executor<'a, C> {
    pub(super) const fn new(
        program: &'a Program,
        input: &'a [u16],
        limits: ExecutionLimits,
        control: &'a mut C,
    ) -> Self {
        Self {
            program,
            input,
            limits,
            control,
            stats: ExecutionStats {
                steps: 0,
                candidate_starts: 0,
                max_backtrack_depth: 0,
                max_undo_depth: 0,
            },
        }
    }

    pub(super) fn search(
        mut self,
        start: usize,
        anchored: bool,
    ) -> Result<SearchOutcome, ExecutionError> {
        if start > self.input.len() {
            return Err(ExecutionError::StartOutOfBounds);
        }
        if self.program.capture_count > self.limits.max_capture_slots {
            return Err(ExecutionError::CaptureLimit {
                limit: self.limits.max_capture_slots,
            });
        }
        let mut candidate = start;
        loop {
            self.charge_candidate()?;
            if let Some(matched) = self.run_attempt(candidate)? {
                return Ok(SearchOutcome {
                    matched: Some(matched),
                    stats: self.stats,
                });
            }
            if anchored || candidate == self.input.len() {
                return Ok(SearchOutcome {
                    matched: None,
                    stats: self.stats,
                });
            }
            candidate =
                advance_candidate(self.input, candidate, self.program.flags.has_unicode_mode())?;
        }
    }

    fn run_attempt(&mut self, start: usize) -> Result<Option<Match>, ExecutionError> {
        let mut state = AttemptState::new(self.program);
        let mut instruction = 0_usize;
        let mut position = start;
        loop {
            self.charge_step()?;
            let Some(current) = self.program.instructions.get(instruction).copied() else {
                return Err(ExecutionError::InvalidProgram);
            };
            match self.execute_instruction(current, &mut state, instruction, position)? {
                StepOutcome::Accept => return Ok(Some(build_match(start, position, &state))),
                StepOutcome::Next {
                    instruction: next,
                    position: next_position,
                } => {
                    instruction = next;
                    position = next_position;
                }
                StepOutcome::Failed => {
                    let Some(restored) = Self::backtrack(&mut state)? else {
                        return Ok(None);
                    };
                    (instruction, position) = restored;
                }
            }
        }
    }

    fn execute_instruction(
        &mut self,
        current: Instruction,
        state: &mut AttemptState,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        let sequential = next_instruction(instruction)?;
        match current {
            Instruction::Accept => Ok(StepOutcome::Accept),
            Instruction::Char(expected) => self.consume_char(expected, sequential, position),
            Instruction::CharReverse(expected) => {
                self.consume_char_reverse(expected, sequential, position)
            }
            Instruction::Backreference(id) => {
                self.consume_backreference(state, id, sequential, position)
            }
            Instruction::BackreferenceReverse(id) => {
                self.consume_backreference_reverse(state, id, sequential, position)
            }
            Instruction::Class(id) => self.consume_class(id, sequential, position),
            Instruction::ClassReverse(id) => self.consume_class_reverse(id, sequential, position),
            Instruction::Any => self.consume_any(sequential, position),
            Instruction::AnyReverse => self.consume_any_reverse(sequential, position),
            _ => self.execute_assertion_instruction(current, state, sequential, position),
        }
    }

    fn execute_assertion_instruction(
        &mut self,
        current: Instruction,
        state: &mut AttemptState,
        sequential: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        match current {
            Instruction::WordBoundary(inverted) => {
                self.assert_word_boundary(inverted, sequential, position)
            }
            Instruction::PositiveLookaheadStart { failure } => {
                self.push_backtrack_frame(
                    state,
                    failure,
                    position,
                    BacktrackFrameKind::PositiveLookahead,
                )?;
                Ok(next_step(sequential, position))
            }
            Instruction::PositiveLookaheadMatched { success } => {
                let saved = Self::complete_positive_lookahead(state)?;
                Ok(next_step(success, saved))
            }
            Instruction::NegativeLookaheadStart { success } => {
                self.push_backtrack_frame(
                    state,
                    success,
                    position,
                    BacktrackFrameKind::NegativeLookahead,
                )?;
                Ok(next_step(sequential, position))
            }
            Instruction::NegativeLookaheadMatched => {
                Self::fail_negative_lookahead(state)?;
                Ok(StepOutcome::Failed)
            }
            Instruction::Fail => Ok(StepOutcome::Failed),
            Instruction::AssertStart => Ok(if self.at_start(position) {
                next_step(sequential, position)
            } else {
                StepOutcome::Failed
            }),
            Instruction::AssertEnd => Ok(if self.at_end(position) {
                next_step(sequential, position)
            } else {
                StepOutcome::Failed
            }),
            _ => self.execute_vm_instruction(current, state, sequential, position),
        }
    }

    fn execute_vm_instruction(
        &mut self,
        current: Instruction,
        state: &mut AttemptState,
        sequential: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        match current {
            Instruction::SaveStart(id) => {
                self.set_capture(
                    state,
                    id,
                    CaptureSlot {
                        start: Some(position),
                        end: None,
                    },
                )?;
                Ok(next_step(sequential, position))
            }
            Instruction::SaveEnd(id) => {
                let Some(previous) = state.captures.get(id).copied() else {
                    return Err(ExecutionError::InvalidProgram);
                };
                self.set_capture(
                    state,
                    id,
                    CaptureSlot {
                        start: previous.start,
                        end: Some(position),
                    },
                )?;
                Ok(next_step(sequential, position))
            }
            Instruction::SaveEndReverse(id) => {
                self.set_capture(
                    state,
                    id,
                    CaptureSlot {
                        start: None,
                        end: Some(position),
                    },
                )?;
                Ok(next_step(sequential, position))
            }
            Instruction::SaveStartReverse(id) => {
                let Some(previous) = state.captures.get(id).copied() else {
                    return Err(ExecutionError::InvalidProgram);
                };
                self.set_capture(
                    state,
                    id,
                    CaptureSlot {
                        start: Some(position),
                        end: previous.end,
                    },
                )?;
                Ok(next_step(sequential, position))
            }
            Instruction::ClearCapture(id) => {
                self.set_capture(state, id, CaptureSlot::default())?;
                Ok(next_step(sequential, position))
            }
            Instruction::Split { first, second } => {
                self.push_backtrack(state, second, position)?;
                Ok(next_step(first, position))
            }
            Instruction::Jump(target) => Ok(next_step(target, position)),
            Instruction::ResetProgress(id) => {
                self.set_progress(state, id, Some(position))?;
                Ok(next_step(sequential, position))
            }
            Instruction::CheckProgress { id, no_progress } => {
                let Some(previous) = state.progress.get(id).copied() else {
                    return Err(ExecutionError::InvalidProgram);
                };
                if previous == Some(position) {
                    Ok(next_step(no_progress, position))
                } else {
                    self.set_progress(state, id, Some(position))?;
                    Ok(next_step(sequential, position))
                }
            }
            _ => Err(ExecutionError::InvalidProgram),
        }
    }

    fn at_word_boundary(&self, position: usize) -> Result<bool, ExecutionError> {
        let unicode = self.program.flags.has_unicode_mode();
        let previous = decode_backward(self.input, position, unicode)?
            .is_some_and(|value| is_word_character(value, self.program.flags));
        let next = decode_forward(self.input, position, unicode)?
            .map(|(value, _)| value)
            .is_some_and(|value| is_word_character(value, self.program.flags));
        Ok(previous != next)
    }

    fn assert_word_boundary(
        &self,
        inverted: bool,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<StepOutcome, ExecutionError> {
        Ok(if self.at_word_boundary(position)? == inverted {
            StepOutcome::Failed
        } else {
            next_step(instruction, position)
        })
    }

    fn characters_equal(&self, actual: u32, expected: u32) -> bool {
        if self.program.flags.ignore_case() {
            let unicode = self.program.flags.has_unicode_mode();
            crate::unicode::canonicalize(actual, unicode)
                == crate::unicode::canonicalize(expected, unicode)
        } else {
            actual == expected
        }
    }

    fn at_start(&self, position: usize) -> bool {
        position == 0
            || (self.program.flags.multiline()
                && position
                    .checked_sub(1)
                    .and_then(|index| self.input.get(index))
                    .is_some_and(|unit| is_line_terminator(*unit)))
    }

    fn at_end(&self, position: usize) -> bool {
        position == self.input.len()
            || (self.program.flags.multiline()
                && self
                    .input
                    .get(position)
                    .is_some_and(|unit| is_line_terminator(*unit)))
    }

    fn set_capture(
        &mut self,
        state: &mut AttemptState,
        id: usize,
        value: CaptureSlot,
    ) -> Result<(), ExecutionError> {
        let Some(previous) = state.captures.get(id).copied() else {
            return Err(ExecutionError::InvalidProgram);
        };
        self.push_undo(state, UndoRecord::Capture { id, previous })?;
        let Some(slot) = state.captures.get_mut(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        *slot = value;
        Ok(())
    }

    fn set_progress(
        &mut self,
        state: &mut AttemptState,
        id: usize,
        value: Option<usize>,
    ) -> Result<(), ExecutionError> {
        let Some(previous) = state.progress.get(id).copied() else {
            return Err(ExecutionError::InvalidProgram);
        };
        self.push_undo(state, UndoRecord::Progress { id, previous })?;
        let Some(slot) = state.progress.get_mut(id) else {
            return Err(ExecutionError::InvalidProgram);
        };
        *slot = value;
        Ok(())
    }

    fn push_undo(
        &mut self,
        state: &mut AttemptState,
        record: UndoRecord,
    ) -> Result<(), ExecutionError> {
        if state.undo.len() >= self.limits.max_undo_records {
            return Err(ExecutionError::UndoLimit {
                limit: self.limits.max_undo_records,
            });
        }
        state.undo.push(record);
        self.stats.max_undo_depth = self.stats.max_undo_depth.max(state.undo.len());
        Ok(())
    }

    fn push_backtrack(
        &mut self,
        state: &mut AttemptState,
        instruction: InstructionIndex,
        position: usize,
    ) -> Result<(), ExecutionError> {
        self.push_backtrack_frame(state, instruction, position, BacktrackFrameKind::Branch)
    }

    fn push_backtrack_frame(
        &mut self,
        state: &mut AttemptState,
        instruction: InstructionIndex,
        position: usize,
        kind: BacktrackFrameKind,
    ) -> Result<(), ExecutionError> {
        if state.backtrack.len() >= self.limits.max_backtrack_frames {
            return Err(ExecutionError::BacktrackLimit {
                limit: self.limits.max_backtrack_frames,
            });
        }
        state.backtrack.push(BacktrackFrame {
            instruction,
            position,
            undo_depth: state.undo.len(),
            kind,
        });
        self.stats.max_backtrack_depth = self.stats.max_backtrack_depth.max(state.backtrack.len());
        Ok(())
    }

    fn backtrack(
        state: &mut AttemptState,
    ) -> Result<Option<(InstructionIndex, usize)>, ExecutionError> {
        let Some(frame) = state.backtrack.pop() else {
            return Ok(None);
        };
        Self::restore_undo(state, frame.undo_depth)?;
        Ok(Some((frame.instruction, frame.position)))
    }

    fn fail_negative_lookahead(state: &mut AttemptState) -> Result<(), ExecutionError> {
        let Some(index) = state
            .backtrack
            .iter()
            .rposition(|frame| frame.kind == BacktrackFrameKind::NegativeLookahead)
        else {
            return Err(ExecutionError::InvalidProgram);
        };
        let Some(sentinel) = state.backtrack.get(index).copied() else {
            return Err(ExecutionError::InvalidProgram);
        };
        state.backtrack.truncate(index);
        Self::restore_undo(state, sentinel.undo_depth)
    }

    fn complete_positive_lookahead(state: &mut AttemptState) -> Result<usize, ExecutionError> {
        let Some(index) = state
            .backtrack
            .iter()
            .rposition(|frame| frame.kind == BacktrackFrameKind::PositiveLookahead)
        else {
            return Err(ExecutionError::InvalidProgram);
        };
        let Some(sentinel) = state.backtrack.get(index).copied() else {
            return Err(ExecutionError::InvalidProgram);
        };
        state.backtrack.truncate(index);
        Ok(sentinel.position)
    }

    fn restore_undo(state: &mut AttemptState, undo_depth: usize) -> Result<(), ExecutionError> {
        while state.undo.len() > undo_depth {
            let Some(record) = state.undo.pop() else {
                return Err(ExecutionError::InvalidProgram);
            };
            match record {
                UndoRecord::Capture { id, previous } => {
                    let Some(slot) = state.captures.get_mut(id) else {
                        return Err(ExecutionError::InvalidProgram);
                    };
                    *slot = previous;
                }
                UndoRecord::Progress { id, previous } => {
                    let Some(slot) = state.progress.get_mut(id) else {
                        return Err(ExecutionError::InvalidProgram);
                    };
                    *slot = previous;
                }
            }
        }
        Ok(())
    }

    fn charge_step(&mut self) -> Result<(), ExecutionError> {
        self.stats.steps = self
            .stats
            .steps
            .checked_add(1)
            .ok_or(ExecutionError::SizeOverflow)?;
        if self.stats.steps > self.limits.max_steps {
            return Err(ExecutionError::StepLimit {
                limit: self.limits.max_steps,
            });
        }
        self.control
            .charge_steps(1)
            .map_err(ExecutionError::Interrupted)
    }

    fn charge_candidate(&mut self) -> Result<(), ExecutionError> {
        self.stats.candidate_starts = self
            .stats
            .candidate_starts
            .checked_add(1)
            .ok_or(ExecutionError::SizeOverflow)?;
        if self.stats.candidate_starts > self.limits.max_candidate_starts {
            return Err(ExecutionError::CandidateStartLimit {
                limit: self.limits.max_candidate_starts,
            });
        }
        Ok(())
    }
}

const fn next_step(instruction: InstructionIndex, position: usize) -> StepOutcome {
    StepOutcome::Next {
        instruction,
        position,
    }
}

fn build_match(start: usize, end: usize, state: &AttemptState) -> Match {
    let captures = state
        .captures
        .iter()
        .copied()
        .map(|slot| Capture { span: slot.span() })
        .collect();
    Match {
        span: start..end,
        captures,
    }
}

fn next_instruction(index: InstructionIndex) -> Result<InstructionIndex, ExecutionError> {
    index.checked_add(1).ok_or(ExecutionError::SizeOverflow)
}
