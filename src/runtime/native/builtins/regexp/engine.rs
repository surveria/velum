use std::ops::Range;

use crate::{
    error::{Error, Result},
    regexp_syntax::compile_regexp_utf16 as compile_regexp_syntax,
    value::ErrorName,
};

pub(super) use crate::regexp_syntax::RegExpFlags;

pub(super) fn parse_regexp_flags(flags: &str) -> Result<RegExpFlags> {
    RegExpFlags::parse(flags).map_err(|error| regexp_syntax_error(&error))
}

pub(super) fn validate_regexp_pattern_utf16(pattern: &[u16], flags: RegExpFlags) -> Result<()> {
    compile_regexp(pattern, flags).map(drop)
}

pub(super) fn regexp_find_utf16(
    pattern: &[u16],
    flags: RegExpFlags,
    input: &[u16],
    start: usize,
) -> Result<Option<RegExpMatch>> {
    if start > input.len() {
        return Ok(None);
    }
    let compiled = compile_regexp(pattern, flags)?;
    let Some(matched) = compiled.find_from_utf16(input, start).next() else {
        return Ok(None);
    };
    if flags.sticky() && matched.start() != start {
        return Ok(None);
    }
    let span = regexp_span(matched.range());
    let named_captures = matched
        .named_groups()
        .map(|(name, range)| (name.to_owned(), regexp_optional_span(range)))
        .collect::<Vec<_>>();
    let captures = matched
        .captures
        .into_iter()
        .map(regexp_optional_span)
        .collect::<Vec<_>>();
    Ok(Some(RegExpMatch {
        span,
        captures,
        named_captures,
    }))
}

pub(super) fn regexp_test_utf16(
    pattern: &[u16],
    flags: RegExpFlags,
    input: &[u16],
    start: usize,
) -> Result<Option<Range<usize>>> {
    if start > input.len() {
        return Ok(None);
    }
    let compiled = compile_regexp(pattern, flags)?;
    let Some(matched) = compiled.find_from_utf16(input, start).next() else {
        return Ok(None);
    };
    if flags.sticky() && matched.start() != start {
        return Ok(None);
    }
    Ok(Some(matched.range()))
}

fn compile_regexp(pattern: &[u16], flags: RegExpFlags) -> Result<regress::Regex> {
    compile_regexp_syntax(pattern, flags).map_err(|error| regexp_syntax_error(&error))
}

fn regexp_syntax_error(error: &crate::regexp_syntax::RegExpSyntaxError) -> Error {
    Error::exception(ErrorName::SyntaxError, error.to_string())
}

fn regexp_optional_span(range: Option<Range<usize>>) -> Option<RegExpSpan> {
    range.map(regexp_span)
}

const fn regexp_span(code_units: Range<usize>) -> RegExpSpan {
    RegExpSpan { code_units }
}

pub(super) fn escaped_regexp_source_utf16(pattern: &[u16]) -> Vec<u16> {
    if pattern.is_empty() {
        return "(?:)".encode_utf16().collect();
    }
    let mut escaped = Vec::new();
    let mut escaped_by_backslash = false;
    for unit in pattern {
        match *unit {
            0x005C => {
                escaped.push(*unit);
                escaped_by_backslash = !escaped_by_backslash;
            }
            0x002F => {
                if escaped_by_backslash {
                    escaped.push(*unit);
                } else {
                    escaped.extend("\\/".encode_utf16());
                }
                escaped_by_backslash = false;
            }
            0x000A => {
                escaped.extend("\\n".encode_utf16());
                escaped_by_backslash = false;
            }
            0x000D => {
                escaped.extend("\\r".encode_utf16());
                escaped_by_backslash = false;
            }
            0x2028 => {
                escaped.extend("\\u2028".encode_utf16());
                escaped_by_backslash = false;
            }
            0x2029 => {
                escaped.extend("\\u2029".encode_utf16());
                escaped_by_backslash = false;
            }
            unit => {
                escaped.push(unit);
                escaped_by_backslash = false;
            }
        }
    }
    escaped
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct RegExpMatch {
    pub(super) span: RegExpSpan,
    pub(super) captures: Vec<Option<RegExpSpan>>,
    pub(super) named_captures: Vec<(String, Option<RegExpSpan>)>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct RegExpSpan {
    pub(super) code_units: Range<usize>,
}

pub(super) fn regexp_index_usize_to_number(index: usize) -> Result<f64> {
    let value =
        u32::try_from(index).map_err(|_| Error::limit("RegExp index exceeded supported range"))?;
    Ok(f64::from(value))
}
