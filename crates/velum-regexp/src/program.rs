use core::mem::size_of;

use crate::{Flags, SizeOverflow, character_class::CharacterClass};

pub type InstructionIndex = usize;

#[derive(Debug, Clone, Copy)]
pub enum SimpleAtom {
    Char { expected: u32, flags: Flags },
    Class { id: usize, flags: Flags },
    Any { flags: Flags },
}

#[derive(Debug, Clone, Copy)]
pub struct SimplePattern {
    pub prefix: Option<SimpleAtom>,
    pub atom: SimpleAtom,
    pub min: u64,
    pub max: Option<u64>,
    pub greedy: bool,
    pub tail_capture: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub enum Instruction {
    Accept,
    Char {
        expected: u32,
        flags: Flags,
    },
    CharReverse {
        expected: u32,
        flags: Flags,
    },
    Backreference {
        id: usize,
        flags: Flags,
    },
    BackreferenceReverse {
        id: usize,
        flags: Flags,
    },
    BackreferenceSet {
        id: usize,
        flags: Flags,
    },
    BackreferenceSetReverse {
        id: usize,
        flags: Flags,
    },
    Class {
        id: usize,
        flags: Flags,
    },
    ClassReverse {
        id: usize,
        flags: Flags,
    },
    Any {
        flags: Flags,
    },
    AnyReverse {
        flags: Flags,
    },
    WordBoundary {
        inverted: bool,
        flags: Flags,
    },
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
    AssertStart {
        flags: Flags,
    },
    AssertEnd {
        flags: Flags,
    },
    SaveStart(usize),
    SaveEnd(usize),
    SaveStartReverse(usize),
    SaveEndReverse(usize),
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
    OversizedRepeat {
        minimum_input_units: Option<u64>,
        execution_limit: u64,
        reverse: bool,
    },
}

#[derive(Debug, Clone)]
pub struct Program {
    pub instructions: Vec<Instruction>,
    pub classes: Vec<CharacterClass>,
    pub backreference_sets: Vec<Box<[usize]>>,
    pub flags: Flags,
    pub capture_count: usize,
    pub capture_names: Vec<Option<String>>,
    pub progress_count: usize,
    pub simple_pattern: Option<SimplePattern>,
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
        let backreference_set_headers = self
            .backreference_sets
            .len()
            .checked_mul(size_of::<Box<[usize]>>())
            .ok_or(SizeOverflow)?;
        let backreference_set_bytes = self.backreference_sets.iter().try_fold(
            class_bytes
                .checked_add(backreference_set_headers)
                .ok_or(SizeOverflow)?,
            |total, set| {
                total
                    .checked_add(
                        set.len()
                            .checked_mul(size_of::<usize>())
                            .ok_or(SizeOverflow)?,
                    )
                    .ok_or(SizeOverflow)
            },
        )?;
        let name_headers = self
            .capture_names
            .len()
            .checked_mul(size_of::<Option<String>>())
            .ok_or(SizeOverflow)?;
        self.capture_names.iter().try_fold(
            backreference_set_bytes
                .checked_add(name_headers)
                .ok_or(SizeOverflow)?,
            |total, name| {
                total
                    .checked_add(name.as_ref().map_or(0, String::len))
                    .ok_or(SizeOverflow)
            },
        )
    }
}
