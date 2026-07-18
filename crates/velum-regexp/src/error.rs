use core::fmt;

use crate::InterruptReason;

/// Stable category for a pattern parse or compile failure.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CompileErrorKind {
    PatternTooLong { limit: usize },
    NestingLimit { limit: usize },
    NodeLimit { limit: usize },
    InstructionLimit { limit: usize },
    CaptureLimit { limit: usize },
    RepeatLimit { limit: u32 },
    UnexpectedToken,
    UnterminatedGroup,
    UnterminatedCharacterClass,
    InvalidCharacterClass,
    InvalidUnicodeProperty,
    InvalidEscape,
    InvalidQuantifier,
    IncompatibleUnicodeFlags,
    UnsupportedSyntax,
    SizeOverflow,
}

/// A compile failure with an exact UTF-16 pattern offset.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompileError {
    pub kind: CompileErrorKind,
    pub pattern_offset: usize,
}

impl CompileError {
    pub(super) const fn new(kind: CompileErrorKind, pattern_offset: usize) -> Self {
        Self {
            kind,
            pattern_offset,
        }
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "RegExp compile error {:?} at UTF-16 offset {}",
            self.kind, self.pattern_offset
        )
    }
}

impl std::error::Error for CompileError {}

/// A bounded execution failure distinct from an ordinary no-match result.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ExecutionError {
    StartOutOfBounds,
    StepLimit { limit: usize },
    CandidateStartLimit { limit: usize },
    BacktrackLimit { limit: usize },
    UndoLimit { limit: usize },
    CaptureLimit { limit: usize },
    Interrupted(InterruptReason),
    InvalidProgram,
    SizeOverflow,
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "RegExp execution error: {self:?}")
    }
}

impl std::error::Error for ExecutionError {}

/// Logical retained-size arithmetic exceeded the platform size type.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct SizeOverflow;

impl fmt::Display for SizeOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("RegExp retained payload size overflowed")
    }
}

impl std::error::Error for SizeOverflow {}
