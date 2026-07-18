use core::mem::size_of;

use crate::{Flags, SizeOverflow, character_class::CharacterClass};

pub type InstructionIndex = usize;

#[derive(Debug, Clone, Copy)]
pub enum Instruction {
    Accept,
    Char(u32),
    Backreference(usize),
    Class(usize),
    Any,
    WordBoundary(bool),
    PositiveLookaheadStart {
        failure: InstructionIndex,
    },
    PositiveLookaheadMatched {
        success: InstructionIndex,
    },
    NegativeLookaheadStart {
        success: InstructionIndex,
    },
    NegativeLookaheadMatched,
    Fail,
    AssertStart,
    AssertEnd,
    SaveStart(usize),
    SaveEnd(usize),
    ClearCapture(usize),
    Split {
        first: InstructionIndex,
        second: InstructionIndex,
    },
    Jump(InstructionIndex),
    ResetProgress(usize),
    CheckProgress {
        id: usize,
        no_progress: InstructionIndex,
    },
}

#[derive(Debug, Clone)]
pub struct Program {
    pub instructions: Vec<Instruction>,
    pub classes: Vec<CharacterClass>,
    pub flags: Flags,
    pub capture_count: usize,
    pub capture_names: Vec<Option<String>>,
    pub progress_count: usize,
}

impl Program {
    pub fn retained_payload_bytes(&self) -> Result<usize, SizeOverflow> {
        let instruction_bytes = self
            .instructions
            .len()
            .checked_mul(size_of::<Instruction>())
            .ok_or(SizeOverflow)?;
        let class_headers = self
            .classes
            .len()
            .checked_mul(size_of::<CharacterClass>())
            .ok_or(SizeOverflow)?;
        let class_bytes = self.classes.iter().try_fold(
            instruction_bytes
                .checked_add(class_headers)
                .ok_or(SizeOverflow)?,
            |total, class| {
                total
                    .checked_add(class.retained_payload_bytes()?)
                    .ok_or(SizeOverflow)
            },
        )?;
        let name_headers = self
            .capture_names
            .len()
            .checked_mul(size_of::<Option<String>>())
            .ok_or(SizeOverflow)?;
        self.capture_names.iter().try_fold(
            class_bytes.checked_add(name_headers).ok_or(SizeOverflow)?,
            |total, name| {
                total
                    .checked_add(name.as_ref().map_or(0, String::len))
                    .ok_or(SizeOverflow)
            },
        )
    }
}
