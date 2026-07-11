use std::ops::Range;

use crate::{
    error::{Error, Result},
    regexp_syntax::compile_regexp as compile_regexp_syntax,
    value::ErrorName,
};

pub(super) use crate::regexp_syntax::RegExpFlags;

pub(super) fn parse_regexp_flags(flags: &str) -> Result<RegExpFlags> {
    RegExpFlags::parse(flags).map_err(|error| regexp_syntax_error(&error))
}

pub(super) fn validate_regexp_pattern(pattern: &str, flags: RegExpFlags) -> Result<()> {
    compile_regexp(pattern, flags).map(drop)
}

pub(super) fn regexp_find(
    pattern: &str,
    flags: RegExpFlags,
    input: &str,
    start: usize,
) -> Result<Option<RegExpMatch>> {
    let input_units = input.encode_utf16().collect::<Vec<_>>();
    if start > input_units.len() {
        return Ok(None);
    }
    let compiled = compile_regexp(pattern, flags)?;
    let Some(matched) = compiled.find_from_utf16(&input_units, start).next() else {
        return Ok(None);
    };
    if flags.sticky() && matched.start() != start {
        return Ok(None);
    }
    let span = regexp_span(input, matched.range())?;
    let named_captures = matched
        .named_groups()
        .map(|(name, range)| regexp_optional_span(input, range).map(|span| (name.to_owned(), span)))
        .collect::<Result<Vec<_>>>()?;
    let captures = matched
        .captures
        .into_iter()
        .map(|range| regexp_optional_span(input, range))
        .collect::<Result<Vec<_>>>()?;
    Ok(Some(RegExpMatch {
        span,
        captures,
        named_captures,
    }))
}

pub(super) fn regexp_test_utf16(
    pattern: &str,
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

fn compile_regexp(pattern: &str, flags: RegExpFlags) -> Result<regress::Regex> {
    compile_regexp_syntax(pattern, flags).map_err(|error| regexp_syntax_error(&error))
}

fn regexp_syntax_error(error: &crate::regexp_syntax::RegExpSyntaxError) -> Error {
    Error::exception(ErrorName::SyntaxError, error.to_string())
}

fn regexp_optional_span(input: &str, range: Option<Range<usize>>) -> Result<Option<RegExpSpan>> {
    range.map(|range| regexp_span(input, range)).transpose()
}

fn regexp_span(input: &str, code_units: Range<usize>) -> Result<RegExpSpan> {
    let start = utf16_index_to_byte_boundary(input, code_units.start)
        .ok_or_else(|| Error::runtime("RegExp match starts inside a UTF-16 surrogate pair"))?;
    let end = utf16_index_to_byte_boundary(input, code_units.end)
        .ok_or_else(|| Error::runtime("RegExp match ends inside a UTF-16 surrogate pair"))?;
    Ok(RegExpSpan {
        code_units,
        bytes: start..end,
    })
}

pub(super) fn utf16_index_to_byte_boundary(input: &str, index: usize) -> Option<usize> {
    let mut code_units = 0usize;
    for (byte_index, ch) in input.char_indices() {
        if code_units == index {
            return Some(byte_index);
        }
        code_units = code_units.checked_add(ch.len_utf16())?;
        if code_units == index {
            return byte_index.checked_add(ch.len_utf8());
        }
        if code_units > index {
            return None;
        }
    }
    (code_units == index).then_some(input.len())
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
    pub(super) span: RegExpSpan,
    pub(super) captures: Vec<Option<RegExpSpan>>,
    pub(super) named_captures: Vec<(String, Option<RegExpSpan>)>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct RegExpSpan {
    pub(super) code_units: Range<usize>,
    pub(super) bytes: Range<usize>,
}

pub(super) fn regexp_index_usize_to_number(index: usize) -> Result<f64> {
    let value =
        u32::try_from(index).map_err(|_| Error::limit("RegExp index exceeded supported range"))?;
    Ok(f64::from(value))
}
