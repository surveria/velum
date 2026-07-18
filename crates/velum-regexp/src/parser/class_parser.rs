use crate::{
    CompileError, CompileErrorKind,
    ast::Node,
    character_class::{CharacterClass, CharacterClassTerm},
    unicode_property_ranges,
};

use super::{Parser, is_syntax_character, predefined_class_term};

enum ClassAtom {
    Single(u32),
    Term(CharacterClassTerm),
}

impl ClassAtom {
    const fn into_term(self) -> CharacterClassTerm {
        match self {
            Self::Single(value) => CharacterClassTerm::Range {
                start: value,
                end: value,
            },
            Self::Term(term) => term,
        }
    }
}

impl Parser<'_> {
    pub(super) fn parse_character_class(&mut self) -> Result<Node, CompileError> {
        self.advance_one()?;
        let inverted = if self.peek() == Some(u16::from(b'^')) {
            self.advance_one()?;
            true
        } else {
            false
        };
        let mut terms = Vec::new();
        loop {
            let Some(unit) = self.peek() else {
                return Err(self.error(CompileErrorKind::UnterminatedCharacterClass));
            };
            if unit == u16::from(b']') {
                self.advance_one()?;
                break;
            }
            let first = self.parse_class_atom()?;
            if matches!(&first, ClassAtom::Single(_)) && self.class_range_follows()? {
                self.advance_one()?;
                let second = self.parse_class_atom()?;
                let (ClassAtom::Single(start), ClassAtom::Single(end)) = (first, second) else {
                    return Err(self.error(CompileErrorKind::InvalidCharacterClass));
                };
                if start > end {
                    return Err(self.error(CompileErrorKind::InvalidCharacterClass));
                }
                self.push_class_term(&mut terms, CharacterClassTerm::Range { start, end })?;
            } else {
                self.push_class_term(&mut terms, first.into_term())?;
            }
        }
        self.node(Node::Class(CharacterClass { inverted, terms }))
    }

    fn parse_class_atom(&mut self) -> Result<ClassAtom, CompileError> {
        let Some(unit) = self.peek() else {
            return Err(self.error(CompileErrorKind::UnterminatedCharacterClass));
        };
        if unit != u16::from(b'\\') {
            return self.decode_pattern_value().map(ClassAtom::Single);
        }
        self.advance_one()?;
        let escape_offset = self.position;
        let Some(escaped) = self.peek() else {
            return Err(self.error(CompileErrorKind::InvalidEscape));
        };
        self.advance_one()?;
        match escaped {
            0x0062 => Ok(ClassAtom::Single(0x0008)),
            0x0064 | 0x0044 | 0x0073 | 0x0053 | 0x0077 | 0x0057 => {
                predefined_class_term(escaped).map(ClassAtom::Term)
            }
            0x0070 | 0x0050 if self.flags.has_unicode_mode() => self
                .parse_property_term(escaped == 0x0050, escape_offset)
                .map(ClassAtom::Term),
            0x006E => Ok(ClassAtom::Single(0x000A)),
            0x0072 => Ok(ClassAtom::Single(0x000D)),
            0x0074 => Ok(ClassAtom::Single(0x0009)),
            0x0076 => Ok(ClassAtom::Single(0x000B)),
            0x0066 => Ok(ClassAtom::Single(0x000C)),
            0x0078 => self.parse_fixed_hex(2).map(ClassAtom::Single),
            0x0075 if self.peek() == Some(u16::from(b'{')) => {
                if !self.flags.has_unicode_mode() {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidEscape,
                        escape_offset,
                    ));
                }
                self.parse_braced_hex().map(ClassAtom::Single)
            }
            0x0075 => self.parse_fixed_hex(4).map(ClassAtom::Single),
            0x0030 => Ok(ClassAtom::Single(0)),
            value if self.flags.has_unicode_mode() && !is_class_escape_character(value) => Err(
                CompileError::new(CompileErrorKind::InvalidEscape, escape_offset),
            ),
            value => Ok(ClassAtom::Single(u32::from(value))),
        }
    }

    pub(super) fn parse_property_term(
        &mut self,
        inverted: bool,
        escape_offset: usize,
    ) -> Result<CharacterClassTerm, CompileError> {
        if self.peek() != Some(u16::from(b'{')) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidUnicodeProperty,
                escape_offset,
            ));
        }
        self.advance_one()?;
        let mut name = String::new();
        loop {
            let Some(unit) = self.peek() else {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidUnicodeProperty,
                    escape_offset,
                ));
            };
            if unit == u16::from(b'}') {
                break;
            }
            if !u8::try_from(unit).is_ok_and(|byte| byte.is_ascii_alphanumeric())
                && unit != u16::from(b'_')
                && unit != u16::from(b'=')
            {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidUnicodeProperty,
                    escape_offset,
                ));
            }
            let byte = u8::try_from(unit).map_err(|_| {
                CompileError::new(CompileErrorKind::InvalidUnicodeProperty, escape_offset)
            })?;
            name.push(char::from(byte));
            self.advance_one()?;
        }
        if name.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidUnicodeProperty,
                escape_offset,
            ));
        }
        self.advance_one()?;
        let (property_name, property_value) = name
            .split_once('=')
            .map_or((None, name.as_str()), |(property, value)| {
                (Some(property), value)
            });
        if property_name.is_some_and(str::is_empty) || property_value.is_empty() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidUnicodeProperty,
                escape_offset,
            ));
        }
        let ranges = unicode_property_ranges(property_name, property_value).ok_or_else(|| {
            CompileError::new(CompileErrorKind::InvalidUnicodeProperty, escape_offset)
        })?;
        Ok(CharacterClassTerm::StaticRanges {
            ranges,
            inverted,
            complement_before_case_fold: inverted,
        })
    }

    fn class_range_follows(&self) -> Result<bool, CompileError> {
        if self.peek() != Some(u16::from(b'-')) {
            return Ok(false);
        }
        let next = self
            .position
            .checked_add(1)
            .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
        Ok(self
            .pattern
            .get(next)
            .is_some_and(|unit| *unit != u16::from(b']')))
    }

    fn push_class_term(
        &self,
        terms: &mut Vec<CharacterClassTerm>,
        term: CharacterClassTerm,
    ) -> Result<(), CompileError> {
        if terms.len() >= self.limits.max_character_class_terms {
            return Err(self.error(CompileErrorKind::NodeLimit {
                limit: self.limits.max_character_class_terms,
            }));
        }
        terms.push(term);
        Ok(())
    }
}

const fn is_class_escape_character(value: u16) -> bool {
    is_syntax_character(value) || value == 0x002D
}
