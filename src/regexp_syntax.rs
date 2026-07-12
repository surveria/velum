use regress::{Flags, Regex};

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
        self.bits |= bit;
        Ok(())
    }

    pub(super) const fn regress_flags(self) -> Flags {
        Flags {
            icase: self.ignore_case(),
            multiline: self.multiline(),
            dot_all: self.dot_all(),
            no_opt: false,
            unicode: self.unicode(),
            unicode_sets: self.unicode_sets(),
        }
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

pub fn compile_regexp(pattern: &str, flags: RegExpFlags) -> Result<Regex, RegExpSyntaxError> {
    let pattern = pattern.encode_utf16().collect::<Vec<_>>();
    compile_regexp_utf16(&pattern, flags)
}

pub fn compile_regexp_utf16(
    pattern: &[u16],
    flags: RegExpFlags,
) -> Result<Regex, RegExpSyntaxError> {
    let code_points = char::decode_utf16(pattern.iter().copied())
        .map(|value| value.map_or_else(|error| u32::from(error.unpaired_surrogate()), u32::from));
    Regex::from_unicode(code_points, flags.regress_flags())
        .map_err(|error| RegExpSyntaxError::InvalidPattern(error.to_string()))
}

pub fn validate_regexp_literal(pattern: &str, flags: &str) -> Result<(), RegExpSyntaxError> {
    let flags = RegExpFlags::parse(flags)?;
    compile_regexp(pattern, flags).map(drop)
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum RegExpSyntaxError {
    #[error("unsupported regular expression flag: {0}")]
    UnsupportedFlag(char),
    #[error("duplicate regular expression flag: {0}")]
    DuplicateFlag(char),
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
