#[cfg(not(feature = "std"))]
use crate::prelude::*;

use core::ops::Range;

use velum_regexp::{
    CompileLimits, ExecutionControl, ExecutionError, ExecutionLimits, Flags, Regex,
};

type NamedCaptureResults = Vec<(String, Option<Range<usize>>)>;

#[derive(Debug)]
pub struct CompiledRegExp {
    backend: Regex,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CompiledRegExpMatch {
    pub span: Range<usize>,
    pub captures: Vec<Option<Range<usize>>>,
    pub named_captures: NamedCaptureResults,
}

impl CompiledRegExp {
    pub fn retained_payload_bytes(&self) -> Option<usize> {
        self.backend.retained_payload_bytes().ok()
    }

    pub fn find_utf16<C: ExecutionControl>(
        &self,
        flags: RegExpFlags,
        input: &[u16],
        start: usize,
        control: &mut C,
    ) -> Result<Option<CompiledRegExpMatch>, ExecutionError> {
        let limits = ExecutionLimits {
            max_steps: ExecutionLimits::MAXIMUM.max_steps,
            max_candidate_starts: ExecutionLimits::MAXIMUM.max_candidate_starts,
            ..ExecutionLimits::default()
        };
        let matched = self
            .backend
            .find_with_control(input, start, flags.sticky(), limits, control)?
            .matched;
        let Some(matched) = matched else {
            return Ok(None);
        };
        let captures = matched
            .captures
            .into_iter()
            .map(|capture| capture.span)
            .collect::<Vec<_>>();
        let named_captures = self.named_capture_results(&captures, control)?;
        Ok(Some(CompiledRegExpMatch {
            span: matched.span,
            captures,
            named_captures,
        }))
    }

    fn named_capture_results<C: ExecutionControl>(
        &self,
        captures: &[Option<Range<usize>>],
        control: &mut C,
    ) -> Result<NamedCaptureResults, ExecutionError> {
        let mut named = NamedCaptureResults::new();
        for index in 0..self.backend.capture_count() {
            let Some(name) = self.backend.capture_name(index) else {
                continue;
            };
            control
                .charge_steps(1)
                .map_err(ExecutionError::Interrupted)?;
            let span = captures.get(index).cloned().flatten();
            let mut existing = None;
            for (position, (candidate, _)) in named.iter().enumerate() {
                control
                    .charge_steps(1)
                    .map_err(ExecutionError::Interrupted)?;
                if candidate == name {
                    existing = Some(position);
                    break;
                }
            }
            if let Some(position) = existing {
                if span.is_some() {
                    let Some((_, current)) = named.get_mut(position) else {
                        return Err(ExecutionError::InvalidProgram);
                    };
                    *current = span;
                }
            } else {
                named.push((name.to_owned(), span));
            }
        }
        Ok(named)
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct RegExpFlags {
    bits: u16,
}

impl RegExpFlags {
    pub(super) fn parse(flags: &str) -> Result<Self, RegExpSyntaxError> {
        let mut seen = Self::default();
        for flag in flags.chars() {
            seen.record(flag)?;
        }
        Ok(seen)
    }

    const fn record(&mut self, flag: char) -> Result<(), RegExpSyntaxError> {
        let bit = match flag {
            'g' => REGEXP_FLAG_GLOBAL,
            'i' => REGEXP_FLAG_IGNORE_CASE,
            'm' => REGEXP_FLAG_MULTILINE,
            's' => REGEXP_FLAG_DOT_ALL,
            'u' => REGEXP_FLAG_UNICODE,
            'y' => REGEXP_FLAG_STICKY,
            'd' => REGEXP_FLAG_HAS_INDICES,
            'v' => REGEXP_FLAG_UNICODE_SETS,
            _ => return Err(RegExpSyntaxError::UnsupportedFlag(flag)),
        };
        if self.bits & bit != 0 {
            return Err(RegExpSyntaxError::DuplicateFlag(flag));
        }
        if bit & (REGEXP_FLAG_UNICODE | REGEXP_FLAG_UNICODE_SETS) != 0
            && self.bits & (REGEXP_FLAG_UNICODE | REGEXP_FLAG_UNICODE_SETS) != 0
        {
            return Err(RegExpSyntaxError::IncompatibleUnicodeModes);
        }
        self.bits |= bit;
        Ok(())
    }

    pub(super) fn native_flags(self) -> Flags {
        Flags::default()
            .with_ignore_case(self.ignore_case())
            .with_multiline(self.multiline())
            .with_dot_all(self.dot_all())
            .with_unicode(self.unicode())
            .with_unicode_sets(self.unicode_sets())
    }

    pub(super) const fn ignore_case(self) -> bool {
        self.bits & REGEXP_FLAG_IGNORE_CASE != 0
    }

    pub(super) const fn multiline(self) -> bool {
        self.bits & REGEXP_FLAG_MULTILINE != 0
    }

    pub(super) const fn dot_all(self) -> bool {
        self.bits & REGEXP_FLAG_DOT_ALL != 0
    }

    pub(super) const fn global(self) -> bool {
        self.bits & REGEXP_FLAG_GLOBAL != 0
    }

    pub(super) const fn sticky(self) -> bool {
        self.bits & REGEXP_FLAG_STICKY != 0
    }

    pub(super) const fn has_indices(self) -> bool {
        self.bits & REGEXP_FLAG_HAS_INDICES != 0
    }

    pub(super) const fn unicode(self) -> bool {
        self.bits & REGEXP_FLAG_UNICODE != 0
    }

    pub(super) const fn unicode_sets(self) -> bool {
        self.bits & REGEXP_FLAG_UNICODE_SETS != 0
    }
}

pub fn compile_regexp_utf16(
    pattern: &[u16],
    flags: RegExpFlags,
) -> Result<CompiledRegExp, RegExpSyntaxError> {
    let limits = CompileLimits {
        max_pattern_units: CompileLimits::MAXIMUM.max_pattern_units,
        max_repeat_count: CompileLimits::MAXIMUM.max_repeat_count,
        ..CompileLimits::default()
    };
    let backend = Regex::compile(pattern, flags.native_flags(), limits)
        .map_err(|error| RegExpSyntaxError::InvalidPattern(error.to_string()))?;
    Ok(CompiledRegExp { backend })
}

pub fn validate_regexp_literal_utf16(
    pattern: &[u16],
    flags: &str,
) -> Result<(), RegExpSyntaxError> {
    let flags = RegExpFlags::parse(flags)?;
    compile_regexp_utf16(pattern, flags).map(drop)
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum RegExpSyntaxError {
    #[error("unsupported regular expression flag: {0}")]
    UnsupportedFlag(char),
    #[error("duplicate regular expression flag: {0}")]
    DuplicateFlag(char),
    #[error("regular expression flags 'u' and 'v' are mutually exclusive")]
    IncompatibleUnicodeModes,
    #[error("invalid regular expression pattern: {0}")]
    InvalidPattern(String),
}

const REGEXP_FLAG_GLOBAL: u16 = 1 << 0;
const REGEXP_FLAG_IGNORE_CASE: u16 = 1 << 1;
const REGEXP_FLAG_MULTILINE: u16 = 1 << 2;
const REGEXP_FLAG_DOT_ALL: u16 = 1 << 3;
const REGEXP_FLAG_UNICODE: u16 = 1 << 4;
const REGEXP_FLAG_STICKY: u16 = 1 << 5;
const REGEXP_FLAG_HAS_INDICES: u16 = 1 << 6;
const REGEXP_FLAG_UNICODE_SETS: u16 = 1 << 7;
