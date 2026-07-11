use std::ops::Range;

use regress::{Flags, Regex};

use crate::{
    error::{Error, Result},
    value::ErrorName,
};

const INVALID_REGEXP_PATTERN_ERROR: &str = "invalid regular expression pattern";
const UNSUPPORTED_REGEXP_FLAG_ERROR: &str = "unsupported regular expression flag";

pub(super) fn validate_regexp_pattern(pattern: &str, flags: &RegExpFlags) -> Result<()> {
    compile_regexp(pattern, flags).map(drop)
}

pub(super) fn regexp_find(
    pattern: &str,
    flags: &RegExpFlags,
    input: &str,
    start: usize,
) -> Result<Option<RegExpMatch>> {
    if start > input.len() {
        return Ok(None);
    }
    let Some(start) = next_string_boundary(input, start) else {
        return Ok(None);
    };
    let compiled = compile_regexp(pattern, flags)?;
    let Some(matched) = compiled.find_from(input, start).next() else {
        return Ok(None);
    };
    if flags.sticky() && matched.start() != start {
        return Ok(None);
    }
    let named_captures = matched
        .named_groups()
        .map(|(name, range)| (name.to_owned(), range))
        .collect();
    Ok(Some(RegExpMatch {
        start: matched.start(),
        end: matched.end(),
        captures: matched.captures,
        named_captures,
    }))
}

fn compile_regexp(pattern: &str, flags: &RegExpFlags) -> Result<Regex> {
    Regex::with_flags(pattern, flags.regress_flags()).map_err(|error| {
        Error::exception(
            ErrorName::SyntaxError,
            format!("{INVALID_REGEXP_PATTERN_ERROR}: {error}"),
        )
    })
}

fn next_string_boundary(input: &str, start: usize) -> Option<usize> {
    if input.is_char_boundary(start) {
        return Some(start);
    }
    input
        .char_indices()
        .map(|(index, _)| index)
        .find(|index| *index > start)
}

#[derive(Debug, Default)]
pub(super) struct RegExpFlags {
    bits: u16,
}

impl RegExpFlags {
    pub(super) fn parse(flags: &str) -> Result<Self> {
        let mut seen = Self::default();
        for flag in flags.chars() {
            seen.record(flag)?;
        }
        Ok(seen)
    }

    fn record(&mut self, flag: char) -> Result<()> {
        let bit = match flag {
            'g' => REGEXP_FLAG_GLOBAL,
            'i' => REGEXP_FLAG_IGNORE_CASE,
            'm' => REGEXP_FLAG_MULTILINE,
            's' => REGEXP_FLAG_DOT_ALL,
            'u' => REGEXP_FLAG_UNICODE,
            'y' => REGEXP_FLAG_STICKY,
            'd' => REGEXP_FLAG_HAS_INDICES,
            'v' => REGEXP_FLAG_UNICODE_SETS,
            _ => {
                return Err(Error::exception(
                    ErrorName::SyntaxError,
                    format!("{UNSUPPORTED_REGEXP_FLAG_ERROR}: {flag}"),
                ));
            }
        };
        if self.bits & bit != 0 {
            return Err(Error::exception(
                ErrorName::SyntaxError,
                format!("duplicate regular expression flag: {flag}"),
            ));
        }
        self.bits |= bit;
        Ok(())
    }

    const fn regress_flags(&self) -> Flags {
        Flags {
            icase: self.ignore_case(),
            multiline: self.multiline(),
            dot_all: self.dot_all(),
            no_opt: false,
            unicode: self.unicode(),
            unicode_sets: self.unicode_sets(),
        }
    }

    pub(super) const fn ignore_case(&self) -> bool {
        self.bits & REGEXP_FLAG_IGNORE_CASE != 0
    }

    pub(super) const fn multiline(&self) -> bool {
        self.bits & REGEXP_FLAG_MULTILINE != 0
    }

    pub(super) const fn dot_all(&self) -> bool {
        self.bits & REGEXP_FLAG_DOT_ALL != 0
    }

    pub(super) const fn global(&self) -> bool {
        self.bits & REGEXP_FLAG_GLOBAL != 0
    }

    pub(super) const fn sticky(&self) -> bool {
        self.bits & REGEXP_FLAG_STICKY != 0
    }

    pub(super) const fn has_indices(&self) -> bool {
        self.bits & REGEXP_FLAG_HAS_INDICES != 0
    }

    pub(super) const fn unicode(&self) -> bool {
        self.bits & REGEXP_FLAG_UNICODE != 0
    }

    pub(super) const fn unicode_sets(&self) -> bool {
        self.bits & REGEXP_FLAG_UNICODE_SETS != 0
    }

    pub(super) fn canonical_text(&self) -> String {
        let mut flags = String::new();
        for (enabled, flag) in [
            (self.has_indices(), 'd'),
            (self.global(), 'g'),
            (self.ignore_case(), 'i'),
            (self.multiline(), 'm'),
            (self.dot_all(), 's'),
            (self.unicode(), 'u'),
            (self.unicode_sets(), 'v'),
            (self.sticky(), 'y'),
        ] {
            if enabled {
                flags.push(flag);
            }
        }
        flags
    }
}

pub(super) fn escaped_regexp_source(pattern: &str) -> String {
    if pattern.is_empty() {
        return "(?:)".to_owned();
    }
    let mut escaped = String::new();
    for ch in pattern.chars() {
        match ch {
            '/' => escaped.push_str("\\/"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\u{2028}' => escaped.push_str("\\u2028"),
            '\u{2029}' => escaped.push_str("\\u2029"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct RegExpMatch {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) captures: Vec<Option<Range<usize>>>,
    pub(super) named_captures: Vec<(String, Option<Range<usize>>)>,
}

pub(super) fn regexp_index_usize_to_number(index: usize) -> Result<f64> {
    let value =
        u32::try_from(index).map_err(|_| Error::limit("RegExp index exceeded supported range"))?;
    Ok(f64::from(value))
}

const REGEXP_FLAG_GLOBAL: u16 = 1 << 0;
const REGEXP_FLAG_IGNORE_CASE: u16 = 1 << 1;
const REGEXP_FLAG_MULTILINE: u16 = 1 << 2;
const REGEXP_FLAG_DOT_ALL: u16 = 1 << 3;
const REGEXP_FLAG_UNICODE: u16 = 1 << 4;
const REGEXP_FLAG_STICKY: u16 = 1 << 5;
const REGEXP_FLAG_HAS_INDICES: u16 = 1 << 6;
const REGEXP_FLAG_UNICODE_SETS: u16 = 1 << 7;
