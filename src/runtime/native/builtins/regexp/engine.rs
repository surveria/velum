use std::ops::Range;

use crate::{
    error::{Error, Result},
    regexp_syntax::{CompiledRegExp, compile_regexp_utf16 as compile_regexp_syntax},
    runtime::Context,
    value::ErrorName,
};
use velum_regexp::{ExecutionControl, ExecutionError, InterruptReason};

pub(super) use crate::regexp_syntax::RegExpFlags;

pub(super) fn parse_regexp_flags(flags: &str) -> Result<RegExpFlags> {
    RegExpFlags::parse(flags).map_err(|error| regexp_syntax_error(&error))
}

pub(super) fn compile_regexp_pattern_utf16(
    pattern: &[u16],
    flags: RegExpFlags,
) -> Result<CompiledRegExp> {
    compile_regexp_syntax(pattern, flags).map_err(|error| regexp_syntax_error(&error))
}

pub(super) fn regexp_find_utf16<C: ExecutionControl>(
    compiled: &CompiledRegExp,
    flags: RegExpFlags,
    input: &[u16],
    start: usize,
    control: &mut C,
) -> std::result::Result<Option<RegExpMatch>, ExecutionError> {
    if start > input.len() {
        return Ok(None);
    }
    let Some(matched) = compiled.find_utf16(flags, input, start, control)? else {
        return Ok(None);
    };
    let span = regexp_span(matched.span);
    let named_captures = matched
        .named_captures
        .into_iter()
        .map(|(name, range)| (name, regexp_optional_span(range)))
        .collect();
    let captures = matched
        .captures
        .into_iter()
        .map(regexp_optional_span)
        .collect();
    Ok(Some(RegExpMatch {
        span,
        captures,
        named_captures,
    }))
}

pub(super) struct RuntimeRegExpControl<'a> {
    context: &'a mut Context,
    error: Option<Error>,
}

impl<'a> RuntimeRegExpControl<'a> {
    pub(super) const fn new(context: &'a mut Context) -> Self {
        Self {
            context,
            error: None,
        }
    }

    pub(super) fn complete<T>(self, result: std::result::Result<T, ExecutionError>) -> Result<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => self.error.map_or_else(
                || Err(regexp_execution_error(&error)),
                |host_error| Err(host_error),
            ),
        }
    }
}

impl ExecutionControl for RuntimeRegExpControl<'_> {
    fn charge_steps(&mut self, steps: usize) -> std::result::Result<(), InterruptReason> {
        if let Err(error) = self.context.charge_runtime_steps(steps) {
            self.error = Some(error);
            return Err(InterruptReason::HostStepLimit);
        }
        Ok(())
    }
}

fn regexp_syntax_error(error: &crate::regexp_syntax::RegExpSyntaxError) -> Error {
    Error::exception(ErrorName::SyntaxError, error.to_string())
}

fn regexp_execution_error(error: &ExecutionError) -> Error {
    Error::limit(format!("native RegExp execution failed: {error}"))
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
    let mut in_character_class = false;
    for unit in pattern {
        match *unit {
            0x005C => {
                escaped.push(*unit);
                escaped_by_backslash = !escaped_by_backslash;
            }
            0x002F => {
                if escaped_by_backslash || in_character_class {
                    escaped.push(*unit);
                } else {
                    escaped.extend("\\/".encode_utf16());
                }
                escaped_by_backslash = false;
            }
            0x000A => {
                if escaped_by_backslash {
                    escaped.pop();
                }
                escaped.extend("\\n".encode_utf16());
                escaped_by_backslash = false;
            }
            0x000D => {
                if escaped_by_backslash {
                    escaped.pop();
                }
                escaped.extend("\\r".encode_utf16());
                escaped_by_backslash = false;
            }
            0x2028 => {
                if escaped_by_backslash {
                    escaped.pop();
                }
                escaped.extend("\\u2028".encode_utf16());
                escaped_by_backslash = false;
            }
            0x2029 => {
                if escaped_by_backslash {
                    escaped.pop();
                }
                escaped.extend("\\u2029".encode_utf16());
                escaped_by_backslash = false;
            }
            unit => {
                escaped.push(unit);
                if !escaped_by_backslash {
                    if unit == 0x005B {
                        in_character_class = true;
                    } else if unit == 0x005D {
                        in_character_class = false;
                    }
                }
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
