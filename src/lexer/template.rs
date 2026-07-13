use crate::{
    error::{Error, Result},
    lexer::{TemplatePart, support::push_utf16_char},
};

/// Tracks one open `${` substitution while its expression is scanned.
#[derive(Clone, Debug)]
pub(super) struct TemplateSubstitutionState {
    pub(super) open_braces: usize,
    pub(super) substitution_offset: usize,
}

/// Identifies the leading or continuation portion of a template literal.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum TemplatePartPosition {
    Head,
    Continuation,
}

pub(super) fn template_part_value(
    source: &str,
    cooked: Vec<u16>,
    raw_start: usize,
    raw_end: usize,
) -> Result<TemplatePart> {
    let raw_source = source
        .get(raw_start..raw_end)
        .ok_or_else(|| Error::lex("template literal raw span is invalid", raw_start))?;
    let mut raw = Vec::new();
    let mut chars = raw_source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            push_utf16_char(&mut raw, '\n');
        } else {
            push_utf16_char(&mut raw, ch);
        }
    }
    Ok(TemplatePart {
        cooked: Rc::from(cooked.into_boxed_slice()),
        raw: Rc::from(raw.into_boxed_slice()),
    })
}
use std::rc::Rc;
