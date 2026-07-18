use crate::{
    CompileError, CompileErrorKind, CompileLimits, Flags,
    ast::{Node, ParsedPattern},
    program::{Instruction, InstructionIndex, Program},
};

pub struct Compiler {
    instructions: Vec<Instruction>,
    classes: Vec<crate::character_class::CharacterClass>,
    backreference_sets: Vec<Box<[usize]>>,
    limits: CompileLimits,
    progress_count: usize,
}

#[derive(Clone, Copy)]
enum Direction {
    Forward,
    Reverse,
}

impl Compiler {
    pub(super) fn compile(
        parsed: &ParsedPattern,
        flags: Flags,
        limits: CompileLimits,
    ) -> Result<Program, CompileError> {
        let mut compiler = Self {
            instructions: Vec::new(),
            classes: Vec::new(),
            backreference_sets: Vec::new(),
            limits,
            progress_count: 0,
        };
        compiler.compile_node(&parsed.root, flags)?;
        compiler.emit(Instruction::Accept)?;
        Ok(Program {
            instructions: compiler.instructions,
            classes: compiler.classes,
            backreference_sets: compiler.backreference_sets,
            flags,
            capture_count: parsed.capture_count,
            capture_names: parsed.capture_names.clone(),
            progress_count: compiler.progress_count,
        })
    }

    fn compile_node(&mut self, node: &Node, flags: Flags) -> Result<(), CompileError> {
        self.compile_node_in(node, Direction::Forward, flags)
    }

    fn compile_node_in(
        &mut self,
        node: &Node,
        direction: Direction,
        flags: Flags,
    ) -> Result<(), CompileError> {
        match node {
            Node::Empty => Ok(()),
            Node::Literal(value) => self.emit_character(*value, direction, flags),
            Node::Backreference { id, .. } => self.emit_backreference(*id, direction, flags),
            Node::BackreferenceSet { ids, .. } => {
                self.emit_backreference_set(ids, direction, flags)
            }
            Node::NamedBackreference { .. } => {
                Err(CompileError::new(CompileErrorKind::UnknownCaptureName, 0))
            }
            Node::Class(class) => {
                let id = self.classes.len();
                self.classes.push(class.clone());
                let instruction = match direction {
                    Direction::Forward => Instruction::Class { id, flags },
                    Direction::Reverse => Instruction::ClassReverse { id, flags },
                };
                self.emit(instruction).map(drop)
            }
            Node::Any => {
                let instruction = match direction {
                    Direction::Forward => Instruction::Any { flags },
                    Direction::Reverse => Instruction::AnyReverse { flags },
                };
                self.emit(instruction).map(drop)
            }
            Node::WordBoundary(inverted) => self
                .emit(Instruction::WordBoundary {
                    inverted: *inverted,
                    flags,
                })
                .map(drop),
            Node::Lookahead { body, positive } => {
                self.compile_assertion(body, *positive, Direction::Forward, flags)
            }
            Node::Lookbehind { body, positive } => {
                self.compile_assertion(body, *positive, Direction::Reverse, flags)
            }
            Node::Modifier { body, set, unset } => {
                self.compile_node_in(body, direction, flags.apply_modifiers(*set, *unset))
            }
            Node::AssertStart => self.emit(Instruction::AssertStart { flags }).map(drop),
            Node::AssertEnd => self.emit(Instruction::AssertEnd { flags }).map(drop),
            Node::Concat(nodes) | Node::LegacySequence(nodes) => {
                match direction {
                    Direction::Forward => {
                        for child in nodes {
                            self.compile_node_in(child, direction, flags)?;
                        }
                    }
                    Direction::Reverse => {
                        for child in nodes.iter().rev() {
                            self.compile_node_in(child, direction, flags)?;
                        }
                    }
                }
                Ok(())
            }
            Node::Alternation(nodes) => self.compile_alternatives(nodes, direction, flags),
            Node::Capture { id, body } => {
                match direction {
                    Direction::Forward => self.emit(Instruction::SaveStart(*id))?,
                    Direction::Reverse => self.emit(Instruction::SaveEndReverse(*id))?,
                };
                self.compile_node_in(body, direction, flags)?;
                let instruction = match direction {
                    Direction::Forward => Instruction::SaveEnd(*id),
                    Direction::Reverse => Instruction::SaveStartReverse(*id),
                };
                self.emit(instruction).map(drop)
            }
            Node::Repeat {
                body,
                min,
                max,
                greedy,
            } => self.compile_repeat(body, *min, *max, *greedy, direction, flags),
        }
    }

    fn emit_character(
        &mut self,
        value: u32,
        direction: Direction,
        flags: Flags,
    ) -> Result<(), CompileError> {
        let instruction = match direction {
            Direction::Forward => Instruction::Char {
                expected: value,
                flags,
            },
            Direction::Reverse => Instruction::CharReverse {
                expected: value,
                flags,
            },
        };
        self.emit(instruction).map(drop)
    }

    fn emit_backreference(
        &mut self,
        id: usize,
        direction: Direction,
        flags: Flags,
    ) -> Result<(), CompileError> {
        let instruction = match direction {
            Direction::Forward => Instruction::Backreference { id, flags },
            Direction::Reverse => Instruction::BackreferenceReverse { id, flags },
        };
        self.emit(instruction).map(drop)
    }

    fn emit_backreference_set(
        &mut self,
        ids: &[usize],
        direction: Direction,
        flags: Flags,
    ) -> Result<(), CompileError> {
        if self.backreference_sets.len() >= self.limits.max_instructions {
            return Err(CompileError::new(
                CompileErrorKind::InstructionLimit {
                    limit: self.limits.max_instructions,
                },
                0,
            ));
        }
        let id = self.backreference_sets.len();
        let instruction = match direction {
            Direction::Forward => Instruction::BackreferenceSet { id, flags },
            Direction::Reverse => Instruction::BackreferenceSetReverse { id, flags },
        };
        self.emit(instruction)?;
        self.backreference_sets.push(Box::from(ids));
        Ok(())
    }

    fn compile_alternatives(
        &mut self,
        nodes: &[Node],
        direction: Direction,
        flags: Flags,
    ) -> Result<(), CompileError> {
        let Some((first, rest)) = nodes.split_first() else {
            return Ok(());
        };
        if rest.is_empty() {
            return self.compile_node_in(first, direction, flags);
        }
        let mut jumps = Vec::new();
        let mut current = first;
        for next in rest {
            let split = self.emit(Instruction::Split {
                first: 0,
                second: 0,
            })?;
            let first_target = self.next_index();
            self.compile_node_in(current, direction, flags)?;
            jumps.push(self.emit(Instruction::Jump(0))?);
            let second_target = self.next_index();
            self.patch_split(split, first_target, second_target)?;
            current = next;
        }
        self.compile_node_in(current, direction, flags)?;
        let end = self.next_index();
        for jump in jumps {
            self.patch_jump(jump, end)?;
        }
        Ok(())
    }

    fn compile_repeat(
        &mut self,
        body: &Node,
        min: u64,
        max: Option<u64>,
        greedy: bool,
        direction: Direction,
        flags: Flags,
    ) -> Result<(), CompileError> {
        let execution_limit = u64::try_from(self.limits.max_instructions)
            .map_err(|_| CompileError::new(CompileErrorKind::SizeOverflow, 0))?;
        if min > execution_limit && self.permits_oversized_repeats() {
            let body_units = minimum_input_units(body);
            let minimum_input_units = (body_units > 0).then(|| body_units.saturating_mul(min));
            self.emit(Instruction::OversizedRepeat {
                minimum_input_units,
                execution_limit,
                reverse: matches!(direction, Direction::Reverse),
            })?;
            return Ok(());
        }
        for _ in 0..min {
            self.emit_capture_clears(body)?;
            self.compile_node_in(body, direction, flags)?;
        }
        match max {
            Some(maximum) => {
                let optional = maximum
                    .checked_sub(min)
                    .ok_or_else(|| CompileError::new(CompileErrorKind::InvalidQuantifier, 0))?;
                for _ in 0..optional {
                    self.compile_optional(body, greedy, direction, flags)?;
                }
                Ok(())
            }
            None => self.compile_unbounded(body, greedy, direction, flags),
        }
    }

    const fn permits_oversized_repeats(&self) -> bool {
        self.limits.max_instructions == CompileLimits::MAXIMUM.max_instructions
    }

    fn compile_optional(
        &mut self,
        body: &Node,
        greedy: bool,
        direction: Direction,
        flags: Flags,
    ) -> Result<(), CompileError> {
        let split = self.emit(Instruction::Split {
            first: 0,
            second: 0,
        })?;
        let body_target = self.next_index();
        self.emit_capture_clears(body)?;
        self.compile_node_in(body, direction, flags)?;
        let end = self.next_index();
        if greedy {
            self.patch_split(split, body_target, end)
        } else {
            self.patch_split(split, end, body_target)
        }
    }

    fn compile_unbounded(
        &mut self,
        body: &Node,
        greedy: bool,
        direction: Direction,
        flags: Flags,
    ) -> Result<(), CompileError> {
        let progress_id = self.allocate_progress()?;
        self.emit(Instruction::ResetProgress(progress_id))?;
        let loop_start = self.next_index();
        let split = self.emit(Instruction::Split {
            first: 0,
            second: 0,
        })?;
        let body_target = self.next_index();
        self.emit_capture_clears(body)?;
        self.compile_node_in(body, direction, flags)?;
        let check = self.emit(Instruction::CheckProgress {
            id: progress_id,
            no_progress: 0,
        })?;
        self.emit(Instruction::Jump(loop_start))?;
        let no_progress = self.next_index();
        self.emit(Instruction::Fail)?;
        let end = self.next_index();
        if greedy {
            self.patch_split(split, body_target, end)?;
        } else {
            self.patch_split(split, end, body_target)?;
        }
        self.patch_progress(check, no_progress)
    }

    fn emit_capture_clears(&mut self, node: &Node) -> Result<(), CompileError> {
        match node {
            Node::Capture { id, body } => {
                self.emit(Instruction::ClearCapture(*id))?;
                self.emit_capture_clears(body)
            }
            Node::Concat(nodes) | Node::LegacySequence(nodes) | Node::Alternation(nodes) => {
                for child in nodes {
                    self.emit_capture_clears(child)?;
                }
                Ok(())
            }
            Node::Repeat { body, .. }
            | Node::Lookahead { body, .. }
            | Node::Lookbehind { body, .. }
            | Node::Modifier { body, .. } => self.emit_capture_clears(body),
            Node::Empty
            | Node::Literal(_)
            | Node::Backreference { .. }
            | Node::BackreferenceSet { .. }
            | Node::NamedBackreference { .. }
            | Node::Class(_)
            | Node::Any
            | Node::WordBoundary(_)
            | Node::AssertStart
            | Node::AssertEnd => Ok(()),
        }
    }

    fn compile_assertion(
        &mut self,
        body: &Node,
        positive: bool,
        direction: Direction,
        flags: Flags,
    ) -> Result<(), CompileError> {
        if positive {
            let start = self.emit(Instruction::PositiveLookaheadStart { failure: 0 })?;
            self.compile_node_in(body, direction, flags)?;
            let matched = self.emit(Instruction::PositiveLookaheadMatched { success: 0 })?;
            let failure = self.next_index();
            self.emit(Instruction::Fail)?;
            let success = self.next_index();
            return self.patch_positive_lookahead(start, matched, failure, success);
        }
        let start = self.emit(Instruction::NegativeLookaheadStart { success: 0 })?;
        self.compile_node_in(body, direction, flags)?;
        self.emit(Instruction::NegativeLookaheadMatched)?;
        let success = self.next_index();
        self.patch_negative_lookahead(start, success)
    }

    fn allocate_progress(&mut self) -> Result<usize, CompileError> {
        let id = self.progress_count;
        self.progress_count = self
            .progress_count
            .checked_add(1)
            .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, 0))?;
        Ok(id)
    }

    fn emit(&mut self, instruction: Instruction) -> Result<InstructionIndex, CompileError> {
        if self.instructions.len() >= self.limits.max_instructions {
            return Err(CompileError::new(
                CompileErrorKind::InstructionLimit {
                    limit: self.limits.max_instructions,
                },
                0,
            ));
        }
        let index = self.instructions.len();
        self.instructions.push(instruction);
        Ok(index)
    }

    const fn next_index(&self) -> InstructionIndex {
        self.instructions.len()
    }

    fn patch_split(
        &mut self,
        index: InstructionIndex,
        first: InstructionIndex,
        second: InstructionIndex,
    ) -> Result<(), CompileError> {
        let Some(instruction) = self.instructions.get_mut(index) else {
            return Err(CompileError::new(CompileErrorKind::SizeOverflow, 0));
        };
        *instruction = Instruction::Split { first, second };
        Ok(())
    }

    fn patch_jump(
        &mut self,
        index: InstructionIndex,
        target: InstructionIndex,
    ) -> Result<(), CompileError> {
        let Some(instruction) = self.instructions.get_mut(index) else {
            return Err(CompileError::new(CompileErrorKind::SizeOverflow, 0));
        };
        *instruction = Instruction::Jump(target);
        Ok(())
    }

    fn patch_progress(
        &mut self,
        index: InstructionIndex,
        target: InstructionIndex,
    ) -> Result<(), CompileError> {
        let Some(instruction) = self.instructions.get_mut(index) else {
            return Err(CompileError::new(CompileErrorKind::SizeOverflow, 0));
        };
        let Instruction::CheckProgress { id, .. } = instruction else {
            return Err(CompileError::new(CompileErrorKind::SizeOverflow, 0));
        };
        *instruction = Instruction::CheckProgress {
            id: *id,
            no_progress: target,
        };
        Ok(())
    }

    fn patch_negative_lookahead(
        &mut self,
        index: InstructionIndex,
        success: InstructionIndex,
    ) -> Result<(), CompileError> {
        let Some(instruction) = self.instructions.get_mut(index) else {
            return Err(CompileError::new(CompileErrorKind::SizeOverflow, 0));
        };
        *instruction = Instruction::NegativeLookaheadStart { success };
        Ok(())
    }

    fn patch_positive_lookahead(
        &mut self,
        start: InstructionIndex,
        matched: InstructionIndex,
        failure: InstructionIndex,
        success: InstructionIndex,
    ) -> Result<(), CompileError> {
        let Some(start_instruction) = self.instructions.get_mut(start) else {
            return Err(CompileError::new(CompileErrorKind::SizeOverflow, 0));
        };
        *start_instruction = Instruction::PositiveLookaheadStart { failure };
        let Some(matched_instruction) = self.instructions.get_mut(matched) else {
            return Err(CompileError::new(CompileErrorKind::SizeOverflow, 0));
        };
        *matched_instruction = Instruction::PositiveLookaheadMatched { success };
        Ok(())
    }
}

fn minimum_input_units(node: &Node) -> u64 {
    match node {
        Node::Literal(_) | Node::Any => 1,
        Node::Class(class) => u64::from(class.strings.iter().all(|string| !string.is_empty())),
        Node::Concat(nodes) | Node::LegacySequence(nodes) => {
            nodes.iter().fold(0_u64, |total, child| {
                total.saturating_add(minimum_input_units(child))
            })
        }
        Node::Alternation(nodes) => nodes.iter().map(minimum_input_units).min().unwrap_or(0),
        Node::Capture { body, .. } | Node::Modifier { body, .. } => minimum_input_units(body),
        Node::Repeat { body, min, .. } => minimum_input_units(body).saturating_mul(*min),
        Node::Empty
        | Node::Backreference { .. }
        | Node::NamedBackreference { .. }
        | Node::BackreferenceSet { .. }
        | Node::WordBoundary(_)
        | Node::Lookahead { .. }
        | Node::Lookbehind { .. }
        | Node::AssertStart
        | Node::AssertEnd => 0,
    }
}
