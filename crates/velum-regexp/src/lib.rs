#![forbid(unsafe_code)]

mod ast;
mod character_class;
mod compiler;
mod control;
mod error;
mod executor;
mod flags;
mod input;
mod limits;
mod match_result;
mod parser;
mod program;
mod unicode;

pub use control::{ExecutionControl, InterruptReason, NoopExecutionControl};
pub use error::{CompileError, CompileErrorKind, ExecutionError, SizeOverflow};
pub use flags::Flags;
pub use limits::{CompileLimits, ExecutionLimits};
pub use match_result::{Capture, ExecutionStats, Match, SearchOutcome};
pub use unicode::{
    UnicodeStringProperty, binary_property_contains, binary_property_ranges, is_id_continue,
    is_id_start, unicode_property_ranges, unicode_string_property, unicode_version,
};

use compiler::Compiler;
use executor::Executor;
use parser::Parser;
use program::Program;

/// An immutable compiled ECMAScript regular expression program.
#[derive(Debug, Clone)]
pub struct Regex {
    program: Program,
}

impl Regex {
    /// Parses and compiles an exact UTF-16 pattern.
    ///
    /// # Errors
    ///
    /// Returns a structured syntax or compile-limit failure with a UTF-16
    /// pattern offset.
    pub fn compile(
        pattern: &[u16],
        flags: Flags,
        limits: CompileLimits,
    ) -> Result<Self, CompileError> {
        let parsed = Parser::parse(pattern, flags, limits)?;
        let program = Compiler::compile(&parsed, flags, limits)?;
        Ok(Self { program })
    }

    /// Searches with a standalone no-op host control and crate-local limits.
    ///
    /// # Errors
    ///
    /// Returns a structured execution-limit or invalid-start failure.
    pub fn find(
        &self,
        input: &[u16],
        start: usize,
        limits: ExecutionLimits,
    ) -> Result<SearchOutcome, ExecutionError> {
        self.find_with_control(input, start, false, limits, &mut NoopExecutionControl)
    }

    /// Searches with an embedding-provided execution control.
    ///
    /// # Errors
    ///
    /// Returns a structured execution-limit, interruption, or invalid-start
    /// failure.
    pub fn find_with_control<C: ExecutionControl>(
        &self,
        input: &[u16],
        start: usize,
        anchored: bool,
        limits: ExecutionLimits,
        control: &mut C,
    ) -> Result<SearchOutcome, ExecutionError> {
        Executor::new(&self.program, input, limits, control).search(start, anchored)
    }

    /// Returns the logical bytes retained by the compiled program.
    ///
    /// # Errors
    ///
    /// Returns [`SizeOverflow`] when the logical byte total cannot fit in
    /// `usize`.
    pub fn retained_payload_bytes(&self) -> Result<usize, SizeOverflow> {
        self.program.retained_payload_bytes()
    }

    /// Returns the number of explicit capturing groups.
    #[must_use]
    pub const fn capture_count(&self) -> usize {
        self.program.capture_count
    }

    /// Returns the optional name assigned to one zero-based capture index.
    #[must_use]
    pub fn capture_name(&self, index: usize) -> Option<&str> {
        self.program
            .capture_names
            .get(index)
            .and_then(|name| name.as_deref())
    }

    /// Returns the zero-based capture index assigned to an exact group name.
    #[must_use]
    pub fn capture_index(&self, name: &str) -> Option<usize> {
        self.program
            .capture_names
            .iter()
            .position(|candidate| candidate.as_deref() == Some(name))
    }
}
