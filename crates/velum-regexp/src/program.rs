use core::mem::size_of;

use crate::{Flags, SizeOverflow};

pub type InstructionIndex = usize;

#[derive(Debug, Clone, Copy)]
pub enum Instruction {
    Accept,
    Char(u32),
    Any,
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
    pub flags: Flags,
    pub capture_count: usize,
    pub progress_count: usize,
}

impl Program {
    pub fn retained_payload_bytes(&self) -> Result<usize, SizeOverflow> {
        self.instructions
            .len()
            .checked_mul(size_of::<Instruction>())
            .ok_or(SizeOverflow)
    }
}
