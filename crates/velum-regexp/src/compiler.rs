use crate::{
    CompileError, CompileErrorKind, CompileLimits, Flags,
    ast::{Node, ParsedPattern},
    program::{Instruction, InstructionIndex, Program},
};

pub struct Compiler {
    instructions: Vec<Instruction>,
    classes: Vec<crate::character_class::CharacterClass>,
    limits: CompileLimits,
    progress_count: usize,
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
            limits,
            progress_count: 0,
        };
        compiler.compile_node(&parsed.root)?;
        compiler.emit(Instruction::Accept)?;
        Ok(Program {
            instructions: compiler.instructions,
            classes: compiler.classes,
            flags,
            capture_count: parsed.capture_count,
            capture_names: parsed.capture_names.clone(),
            progress_count: compiler.progress_count,
        })
    }

    fn compile_node(&mut self, node: &Node) -> Result<(), CompileError> {
        match node {
            Node::Empty => Ok(()),
            Node::Literal(value) => self.emit(Instruction::Char(*value)).map(drop),
            Node::Backreference { id, .. } => self.emit(Instruction::Backreference(*id)).map(drop),
            Node::NamedBackreference { .. } => {
                Err(CompileError::new(CompileErrorKind::UnknownCaptureName, 0))
            }
            Node::Class(class) => {
                let id = self.classes.len();
                self.classes.push(class.clone());
                self.emit(Instruction::Class(id)).map(drop)
            }
            Node::Any => self.emit(Instruction::Any).map(drop),
            Node::WordBoundary(inverted) => {
                self.emit(Instruction::WordBoundary(*inverted)).map(drop)
            }
            Node::Lookahead { body, positive } => self.compile_lookahead(body, *positive),
            Node::AssertStart => self.emit(Instruction::AssertStart).map(drop),
            Node::AssertEnd => self.emit(Instruction::AssertEnd).map(drop),
            Node::Concat(nodes) => {
                for child in nodes {
                    self.compile_node(child)?;
                }
                Ok(())
            }
            Node::Alternation(nodes) => self.compile_alternatives(nodes),
            Node::Capture { id, body } => {
                self.emit(Instruction::SaveStart(*id))?;
                self.compile_node(body)?;
                self.emit(Instruction::SaveEnd(*id)).map(drop)
            }
            Node::Repeat {
                body,
                min,
                max,
                greedy,
            } => self.compile_repeat(body, *min, *max, *greedy),
        }
    }

    fn compile_alternatives(&mut self, nodes: &[Node]) -> Result<(), CompileError> {
        let Some((first, rest)) = nodes.split_first() else {
            return Ok(());
        };
        if rest.is_empty() {
            return self.compile_node(first);
        }
        let split = self.emit(Instruction::Split {
            first: 0,
            second: 0,
        })?;
        let first_target = self.next_index();
        self.compile_node(first)?;
        let jump = self.emit(Instruction::Jump(0))?;
        let second_target = self.next_index();
        self.compile_alternatives(rest)?;
        let end = self.next_index();
        self.patch_split(split, first_target, second_target)?;
        self.patch_jump(jump, end)
    }

    fn compile_repeat(
        &mut self,
        body: &Node,
        min: u32,
        max: Option<u32>,
        greedy: bool,
    ) -> Result<(), CompileError> {
        for _ in 0..min {
            self.emit_capture_clears(body)?;
            self.compile_node(body)?;
        }
        match max {
            Some(maximum) => {
                let optional = maximum
                    .checked_sub(min)
                    .ok_or_else(|| CompileError::new(CompileErrorKind::InvalidQuantifier, 0))?;
                for _ in 0..optional {
                    self.compile_optional(body, greedy)?;
                }
                Ok(())
            }
            None => self.compile_unbounded(body, greedy),
        }
    }

    fn compile_optional(&mut self, body: &Node, greedy: bool) -> Result<(), CompileError> {
        let split = self.emit(Instruction::Split {
            first: 0,
            second: 0,
        })?;
        let body_target = self.next_index();
        self.emit_capture_clears(body)?;
        self.compile_node(body)?;
        let end = self.next_index();
        if greedy {
            self.patch_split(split, body_target, end)
        } else {
            self.patch_split(split, end, body_target)
        }
    }

    fn compile_unbounded(&mut self, body: &Node, greedy: bool) -> Result<(), CompileError> {
        let progress_id = self.allocate_progress()?;
        self.emit(Instruction::ResetProgress(progress_id))?;
        let loop_start = self.next_index();
        let split = self.emit(Instruction::Split {
            first: 0,
            second: 0,
        })?;
        let body_target = self.next_index();
        self.emit_capture_clears(body)?;
        self.compile_node(body)?;
        let check = self.emit(Instruction::CheckProgress {
            id: progress_id,
            no_progress: 0,
        })?;
        self.emit(Instruction::Jump(loop_start))?;
        let end = self.next_index();
        if greedy {
            self.patch_split(split, body_target, end)?;
        } else {
            self.patch_split(split, end, body_target)?;
        }
        self.patch_progress(check, end)
    }

    fn emit_capture_clears(&mut self, node: &Node) -> Result<(), CompileError> {
        match node {
            Node::Capture { id, body } => {
                self.emit(Instruction::ClearCapture(*id))?;
                self.emit_capture_clears(body)
            }
            Node::Concat(nodes) | Node::Alternation(nodes) => {
                for child in nodes {
                    self.emit_capture_clears(child)?;
                }
                Ok(())
            }
            Node::Repeat { body, .. } | Node::Lookahead { body, .. } => {
                self.emit_capture_clears(body)
            }
            Node::Empty
            | Node::Literal(_)
            | Node::Backreference { .. }
            | Node::NamedBackreference { .. }
            | Node::Class(_)
            | Node::Any
            | Node::WordBoundary(_)
            | Node::AssertStart
            | Node::AssertEnd => Ok(()),
        }
    }

    fn compile_lookahead(&mut self, body: &Node, positive: bool) -> Result<(), CompileError> {
        if positive {
            let start = self.emit(Instruction::PositiveLookaheadStart { failure: 0 })?;
            self.compile_node(body)?;
            let matched = self.emit(Instruction::PositiveLookaheadMatched { success: 0 })?;
            let failure = self.next_index();
            self.emit(Instruction::Fail)?;
            let success = self.next_index();
            return self.patch_positive_lookahead(start, matched, failure, success);
        }
        let start = self.emit(Instruction::NegativeLookaheadStart { success: 0 })?;
        self.compile_node(body)?;
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
