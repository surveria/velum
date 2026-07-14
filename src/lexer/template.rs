use std::rc::Rc;

use crate::{
    error::Result,
    lexer::{SourceText, TemplatePart},
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
    source: &SourceText,
    cooked: Option<Vec<u16>>,
    raw_start: usize,
    raw_end: usize,
) -> Result<TemplatePart> {
    let raw_source = source.utf16_range(raw_start, raw_end)?;
    let mut raw = Vec::new();
    let mut units = raw_source.into_iter().peekable();
    while let Some(unit) = units.next() {
        if unit == u16::from(b'\r') {
            if units.peek() == Some(&u16::from(b'\n')) {
                units.next();
            }
            raw.push(u16::from(b'\n'));
        } else {
            raw.push(unit);
        }
    }
    Ok(TemplatePart {
        cooked: cooked.map(|value| Rc::from(value.into_boxed_slice())),
        raw: Rc::from(raw.into_boxed_slice()),
    })
}
