use crate::{
    CompileError, CompileErrorKind,
    ast::Node,
    character_class::{CharacterClass, CharacterClassTerm},
    unicode_property_ranges, unicode_string_property,
};

use super::{
    Parser, PropertyEscape,
    escape_parser::{control_letter_value, is_decimal_digit},
    is_syntax_character, predefined_class_term,
};

enum ClassAtom {
    Single(u32),
    Term(CharacterClassTerm),
    Strings(Vec<Box<[u32]>>),
}

impl ClassAtom {
    fn into_term(self) -> Option<CharacterClassTerm> {
        match self {
            Self::Single(value) => Some(CharacterClassTerm::Range {
                start: value,
                end: value,
            }),
            Self::Term(term) => Some(term),
            Self::Strings(_) => None,
        }
    }
}

impl Parser<'_> {
    pub(super) fn parse_character_class(&mut self) -> Result<Node, CompileError> {
        if self.flags.unicode_sets() {
            return self.parse_unicode_set_class_node();
        }
        self.advance_one()?;
        let inverted = if self.peek() == Some(u16::from(b'^')) {
            self.advance_one()?;
            true
        } else {
            false
        };
        let mut terms = Vec::new();
        let mut strings = Vec::new();
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
            } else if let ClassAtom::Strings(values) = first {
                strings.extend(values);
            } else {
                let term = first
                    .into_term()
                    .ok_or_else(|| self.error(CompileErrorKind::InvalidCharacterClass))?;
                self.push_class_term(&mut terms, term)?;
            }
        }
        if inverted && !strings.is_empty() {
            return Err(self.error(CompileErrorKind::InvalidCharacterClass));
        }
        self.node(Node::Class(CharacterClass {
            inverted,
            codepoint_work: terms.len(),
            terms,
            strings,
        }))
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
            0x0070 | 0x0050 if self.flags.has_unicode_mode() => {
                match self.parse_property_escape(escaped == 0x0050, escape_offset)? {
                    PropertyEscape::CodePoints(term) => Ok(ClassAtom::Term(term)),
                    PropertyEscape::Strings(strings) => Ok(ClassAtom::Strings(strings)),
                }
            }
            0x0063 => self.parse_class_control_escape(escape_offset),
            0x006E => Ok(ClassAtom::Single(0x000A)),
            0x0072 => Ok(ClassAtom::Single(0x000D)),
            0x0074 => Ok(ClassAtom::Single(0x0009)),
            0x0076 => Ok(ClassAtom::Single(0x000B)),
            0x0066 => Ok(ClassAtom::Single(0x000C)),
            0x0078 => self
                .parse_fixed_hex_or_identity(2, escaped, escape_offset)
                .map(ClassAtom::Single),
            0x0075 if self.peek() == Some(u16::from(b'{')) => {
                if !self.flags.has_unicode_mode() {
                    return Ok(ClassAtom::Single(u32::from(b'u')));
                }
                self.parse_braced_hex().map(ClassAtom::Single)
            }
            0x0075 => self
                .parse_unicode_escape_value(escape_offset)
                .map(ClassAtom::Single),
            value if (0x0030..=0x0037).contains(&value) => {
                if self.flags.has_unicode_mode() {
                    if value == 0x0030 && !self.peek().is_some_and(is_decimal_digit) {
                        return Ok(ClassAtom::Single(0));
                    }
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidEscape,
                        escape_offset,
                    ));
                }
                self.parse_legacy_octal_value(value, escape_offset)
                    .map(ClassAtom::Single)
            }
            value if (0x0038..=0x0039).contains(&value) => {
                if self.flags.has_unicode_mode() {
                    return Err(CompileError::new(
                        CompileErrorKind::InvalidEscape,
                        escape_offset,
                    ));
                }
                Ok(ClassAtom::Single(u32::from(value)))
            }
            value if self.flags.has_unicode_mode() && !is_class_escape_character(value) => Err(
                CompileError::new(CompileErrorKind::InvalidEscape, escape_offset),
            ),
            value => Ok(ClassAtom::Single(u32::from(value))),
        }
    }

    fn parse_class_control_escape(
        &mut self,
        escape_offset: usize,
    ) -> Result<ClassAtom, CompileError> {
        if let Some(unit) = self.peek()
            && let Some(value) = control_letter_value(unit)
        {
            self.advance_one()?;
            return Ok(ClassAtom::Single(value));
        }
        if !self.flags.has_unicode_mode()
            && let Some(unit) = self
                .peek()
                .filter(|unit| is_decimal_digit(*unit) || *unit == u16::from(b'_'))
        {
            self.advance_one()?;
            return Ok(ClassAtom::Single(u32::from(unit) % 32));
        }
        if self.flags.has_unicode_mode() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidEscape,
                escape_offset,
            ));
        }
        Ok(ClassAtom::Term(CharacterClassTerm::Pair {
            first: u32::from(b'\\'),
            second: u32::from(b'c'),
        }))
    }

    pub(super) fn parse_property_class(
        &mut self,
        inverted: bool,
        escape_offset: usize,
    ) -> Result<Node, CompileError> {
        let property = self.parse_property_escape(inverted, escape_offset)?;
        let class = match property {
            PropertyEscape::CodePoints(term) => CharacterClass {
                inverted: false,
                terms: vec![term],
                strings: Vec::new(),
                codepoint_work: 1,
            },
            PropertyEscape::Strings(strings) => CharacterClass {
                inverted: false,
                terms: Vec::new(),
                strings,
                codepoint_work: 0,
            },
        };
        self.node(Node::Class(class))
    }

    pub(super) fn parse_property_escape(
        &mut self,
        inverted: bool,
        escape_offset: usize,
    ) -> Result<PropertyEscape, CompileError> {
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
        if property_name.is_none()
            && self.flags.unicode_sets()
            && let Some(property) = unicode_string_property(property_value)
        {
            if inverted {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidUnicodeProperty,
                    escape_offset,
                ));
            }
            return self
                .copy_string_property(property, escape_offset)
                .map(PropertyEscape::Strings);
        }
        let ranges = unicode_property_ranges(property_name, property_value).ok_or_else(|| {
            CompileError::new(CompileErrorKind::InvalidUnicodeProperty, escape_offset)
        })?;
        Ok(PropertyEscape::CodePoints(
            CharacterClassTerm::StaticRanges {
                ranges,
                inverted,
                complement_before_case_fold: inverted,
            },
        ))
    }

    fn copy_string_property(
        &self,
        property: crate::UnicodeStringProperty,
        escape_offset: usize,
    ) -> Result<Vec<Box<[u32]>>, CompileError> {
        let count = property.sequence_count();
        if count > self.limits.max_class_strings {
            return Err(CompileError::new(
                CompileErrorKind::ClassStringLimit {
                    limit: self.limits.max_class_strings,
                },
                escape_offset,
            ));
        }
        let mut total_units = 0_usize;
        let mut strings = Vec::with_capacity(count);
        for index in 0..count {
            let sequence = property.sequence(index).ok_or_else(|| {
                CompileError::new(CompileErrorKind::InvalidUnicodeProperty, escape_offset)
            })?;
            total_units = total_units
                .checked_add(sequence.len())
                .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, escape_offset))?;
            if total_units > self.limits.max_class_string_units {
                return Err(CompileError::new(
                    CompileErrorKind::ClassStringUnitLimit {
                        limit: self.limits.max_class_string_units,
                    },
                    escape_offset,
                ));
            }
            strings.push(Box::<[u32]>::from(sequence));
        }
        Ok(strings)
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
