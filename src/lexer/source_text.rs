use alloc::{collections::BTreeMap, rc::Rc};

use crate::{Error, Result};

const REPLACEMENT_CHARACTER: char = '\u{FFFD}';

#[derive(Clone)]
pub struct SourceText {
    rendered: Rc<str>,
    source_len: usize,
    surrogate_units: Rc<BTreeMap<usize, u16>>,
}

impl SourceText {
    pub fn from_utf8(source: &str) -> Self {
        Self {
            rendered: Rc::from(source),
            source_len: source.len(),
            surrogate_units: Rc::new(BTreeMap::new()),
        }
    }

    pub fn from_utf16(source: &[u16]) -> Self {
        let mut rendered = String::new();
        let mut surrogate_units = BTreeMap::new();
        for decoded in char::decode_utf16(source.iter().copied()) {
            match decoded {
                Ok(ch) => rendered.push(ch),
                Err(error) => {
                    surrogate_units.insert(rendered.len(), error.unpaired_surrogate());
                    rendered.push(REPLACEMENT_CHARACTER);
                }
            }
        }
        Self {
            rendered: Rc::from(rendered.into_boxed_str()),
            source_len: source.len(),
            surrogate_units: Rc::new(surrogate_units),
        }
    }

    pub fn rendered(&self) -> &str {
        self.rendered.as_ref()
    }

    pub const fn source_len(&self) -> usize {
        self.source_len
    }

    pub fn rendered_len(&self) -> usize {
        self.rendered.len()
    }

    pub fn surrogate_at(&self, offset: usize) -> Option<u16> {
        self.surrogate_units.get(&offset).copied()
    }

    pub fn append_utf16_character(&self, output: &mut Vec<u16>, offset: usize, ch: char) {
        if let Some(unit) = self.surrogate_at(offset) {
            output.push(unit);
        } else {
            let mut buffer = [0_u16; 2];
            output.extend_from_slice(ch.encode_utf16(&mut buffer));
        }
    }

    pub fn utf16_range(&self, start: usize, end: usize) -> Result<Vec<u16>> {
        let source = self
            .rendered
            .get(start..end)
            .ok_or_else(|| Error::lex("source UTF-16 span is invalid", start))?;
        let mut output = Vec::new();
        for (relative, ch) in source.char_indices() {
            let offset = start
                .checked_add(relative)
                .ok_or_else(|| Error::lex("source UTF-16 span overflowed", start))?;
            self.append_utf16_character(&mut output, offset, ch);
        }
        Ok(output)
    }
}
