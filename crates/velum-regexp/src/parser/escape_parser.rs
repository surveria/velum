use crate::{CompileError, CompileErrorKind, ast::Node, character_class::CharacterClass};

use super::{Parser, is_syntax_character, predefined_class_term};

impl Parser<'_> {
    pub(super) fn parse_escape(&mut self) -> Result<Node, CompileError> {
        self.advance_one()?;
        let escape_offset = self.position;
        let Some(unit) = self.peek() else {
            return Err(self.error(CompileErrorKind::InvalidEscape));
        };
        self.advance_one()?;
        match unit {
            0x0062 => return self.node(Node::WordBoundary(false)),
            0x0042 => return self.node(Node::WordBoundary(true)),
            0x0064 | 0x0044 | 0x0073 | 0x0053 | 0x0077 | 0x0057 => {
                return self.node(Node::Class(CharacterClass {
                    inverted: false,
                    terms: vec![predefined_class_term(unit)?],
                }));
            }
            0x0070 | 0x0050 if self.flags.has_unicode_mode() => {
                let term = self.parse_property_term(unit == 0x0050, escape_offset)?;
                return self.node(Node::Class(CharacterClass {
                    inverted: false,
                    terms: vec![term],
                }));
            }
            value if (u16::from(b'1')..=u16::from(b'9')).contains(&value) => {
                return self.parse_decimal_escape(value, escape_offset);
            }
            0x006B if self.flags.has_unicode_mode() || self.has_named_capture => {
                return self.parse_named_backreference(escape_offset);
            }
            0x0063 => return self.parse_control_escape(escape_offset),
            _ => {}
        }
        let value = match unit {
            0x006E => 0x000A,
            0x0072 => 0x000D,
            0x0074 => 0x0009,
            0x0076 => 0x000B,
            0x0066 => 0x000C,
            0x0078 => self.parse_fixed_hex_or_identity(2, unit, escape_offset)?,
            0x0075 => return self.parse_unicode_escape(escape_offset),
            0x0030 => return self.parse_zero_escape(escape_offset),
            value if self.flags.has_unicode_mode() && !is_syntax_character(value) => {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidEscape,
                    escape_offset,
                ));
            }
            value => u32::from(value),
        };
        self.node(Node::Literal(value))
    }

    fn parse_decimal_escape(
        &mut self,
        first: u16,
        pattern_offset: usize,
    ) -> Result<Node, CompileError> {
        let mut digits = vec![first];
        let mut number = Some(usize::from(first - u16::from(b'0')));
        while let Some(unit) = self.peek() {
            if !is_decimal_digit(unit) {
                break;
            }
            digits.push(unit);
            number = number.and_then(|current| {
                current
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(usize::from(unit - u16::from(b'0'))))
            });
            self.advance_one()?;
        }
        if let Some(valid) = number.filter(|value| *value <= self.total_capture_count) {
            let id = valid.checked_sub(1).ok_or_else(|| {
                CompileError::new(CompileErrorKind::InvalidBackreference, pattern_offset)
            })?;
            return self.node(Node::Backreference { id, pattern_offset });
        }
        if self.flags.has_unicode_mode() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidBackreference,
                pattern_offset,
            ));
        }
        self.legacy_decimal_nodes(&digits, pattern_offset)
    }

    fn parse_zero_escape(&mut self, pattern_offset: usize) -> Result<Node, CompileError> {
        if !self.peek().is_some_and(is_decimal_digit) {
            return self.node(Node::Literal(0));
        }
        if self.flags.has_unicode_mode() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidEscape,
                pattern_offset,
            ));
        }
        let mut digits = vec![u16::from(b'0')];
        while let Some(unit) = self.peek() {
            if !is_decimal_digit(unit) {
                break;
            }
            digits.push(unit);
            self.advance_one()?;
        }
        self.legacy_decimal_nodes(&digits, pattern_offset)
    }

    fn legacy_decimal_nodes(
        &mut self,
        digits: &[u16],
        pattern_offset: usize,
    ) -> Result<Node, CompileError> {
        let Some(first) = digits.first().copied() else {
            return Err(CompileError::new(
                CompileErrorKind::InvalidEscape,
                pattern_offset,
            ));
        };
        let max_octal_digits = match first {
            0x0030..=0x0033 => 3,
            0x0034..=0x0037 => 2,
            _ => 0,
        };
        let octal_digits = digits
            .iter()
            .take(max_octal_digits)
            .take_while(|unit| is_octal_digit(**unit))
            .count();
        let mut values = Vec::new();
        if octal_digits > 0 {
            let value = digits.iter().take(octal_digits).try_fold(
                0_u32,
                |current, unit| -> Result<u32, CompileError> {
                    current
                        .checked_mul(8)
                        .and_then(|value| value.checked_add(u32::from(*unit - u16::from(b'0'))))
                        .ok_or_else(|| {
                            CompileError::new(CompileErrorKind::SizeOverflow, pattern_offset)
                        })
                },
            )?;
            values.push(value);
        }
        values.extend(digits.iter().skip(octal_digits).copied().map(u32::from));
        self.literal_sequence(&values, pattern_offset)
    }

    fn parse_control_escape(&mut self, pattern_offset: usize) -> Result<Node, CompileError> {
        if let Some(unit) = self.peek()
            && let Some(value) = control_letter_value(unit)
        {
            self.advance_one()?;
            return self.node(Node::Literal(value));
        }
        if self.flags.has_unicode_mode() {
            return Err(CompileError::new(
                CompileErrorKind::InvalidEscape,
                pattern_offset,
            ));
        }
        self.literal_sequence(&[u32::from(b'\\'), u32::from(b'c')], pattern_offset)
    }

    fn parse_unicode_escape(&mut self, pattern_offset: usize) -> Result<Node, CompileError> {
        if self.peek() == Some(u16::from(b'{')) {
            if !self.flags.has_unicode_mode() {
                return self.node(Node::Literal(u32::from(b'u')));
            }
            let value = self.parse_braced_hex()?;
            return self.node(Node::Literal(value));
        }
        let value = self.parse_unicode_escape_value(pattern_offset)?;
        self.node(Node::Literal(value))
    }

    pub(super) fn parse_unicode_escape_value(
        &mut self,
        pattern_offset: usize,
    ) -> Result<u32, CompileError> {
        let value = self.parse_fixed_hex_or_identity(4, u16::from(b'u'), pattern_offset)?;
        if self.flags.has_unicode_mode()
            && (0xD800..=0xDBFF).contains(&value)
            && let Some(low) = self.try_parse_trailing_surrogate_escape()?
        {
            return combine_surrogates(value, low, pattern_offset);
        }
        Ok(value)
    }

    fn try_parse_trailing_surrogate_escape(&mut self) -> Result<Option<u32>, CompileError> {
        let checkpoint = self.position;
        if self.peek() != Some(u16::from(b'\\')) {
            return Ok(None);
        }
        self.advance_one()?;
        if self.peek() != Some(u16::from(b'u')) {
            self.position = checkpoint;
            return Ok(None);
        }
        self.advance_one()?;
        let Ok(value) = self.parse_fixed_hex(4) else {
            self.position = checkpoint;
            return Ok(None);
        };
        if (0xDC00..=0xDFFF).contains(&value) {
            Ok(Some(value))
        } else {
            self.position = checkpoint;
            Ok(None)
        }
    }

    pub(super) fn parse_fixed_hex_or_identity(
        &mut self,
        digits: usize,
        identity: u16,
        pattern_offset: usize,
    ) -> Result<u32, CompileError> {
        let checkpoint = self.position;
        match self.parse_fixed_hex(digits) {
            Ok(value) => Ok(value),
            Err(_) if !self.flags.has_unicode_mode() => {
                self.position = checkpoint;
                Ok(u32::from(identity))
            }
            Err(_) => Err(CompileError::new(
                CompileErrorKind::InvalidEscape,
                pattern_offset,
            )),
        }
    }

    pub(super) fn parse_fixed_hex(&mut self, digits: usize) -> Result<u32, CompileError> {
        let start = self.position;
        let mut value = 0_u32;
        for _ in 0..digits {
            let Some(unit) = self.peek() else {
                return Err(CompileError::new(CompileErrorKind::InvalidEscape, start));
            };
            let Some(digit) = hex_value(unit) else {
                return Err(CompileError::new(CompileErrorKind::InvalidEscape, start));
            };
            value = value
                .checked_mul(16)
                .and_then(|current| current.checked_add(digit))
                .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, start))?;
            self.advance_one()?;
        }
        Ok(value)
    }

    pub(super) fn parse_braced_hex(&mut self) -> Result<u32, CompileError> {
        let start = self.position;
        self.advance_one()?;
        let mut value = 0_u32;
        let mut digits = 0_usize;
        loop {
            let Some(unit) = self.peek() else {
                return Err(CompileError::new(CompileErrorKind::InvalidEscape, start));
            };
            if unit == u16::from(b'}') {
                break;
            }
            let Some(digit) = hex_value(unit) else {
                return Err(CompileError::new(CompileErrorKind::InvalidEscape, start));
            };
            value = value
                .checked_mul(16)
                .and_then(|current| current.checked_add(digit))
                .ok_or_else(|| CompileError::new(CompileErrorKind::InvalidEscape, start))?;
            if value > 0x10_FFFF {
                return Err(CompileError::new(CompileErrorKind::InvalidEscape, start));
            }
            digits = digits
                .checked_add(1)
                .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, start))?;
            self.advance_one()?;
        }
        if digits == 0 {
            return Err(CompileError::new(CompileErrorKind::InvalidEscape, start));
        }
        self.advance_one()?;
        Ok(value)
    }

    pub(super) fn parse_legacy_octal_value(
        &mut self,
        first: u16,
        pattern_offset: usize,
    ) -> Result<u32, CompileError> {
        let max_digits = if first <= u16::from(b'3') { 3 } else { 2 };
        let mut digits = 1_usize;
        let mut value = u32::from(first - u16::from(b'0'));
        while digits < max_digits {
            let Some(unit) = self.peek().filter(|unit| is_octal_digit(*unit)) else {
                break;
            };
            value = value
                .checked_mul(8)
                .and_then(|current| current.checked_add(u32::from(unit - u16::from(b'0'))))
                .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, pattern_offset))?;
            digits = digits
                .checked_add(1)
                .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, pattern_offset))?;
            self.advance_one()?;
        }
        Ok(value)
    }

    fn literal_sequence(
        &mut self,
        values: &[u32],
        pattern_offset: usize,
    ) -> Result<Node, CompileError> {
        let mut nodes = Vec::new();
        for value in values {
            nodes.push(self.node(Node::Literal(*value))?);
        }
        if nodes.len() == 1 {
            return nodes
                .pop()
                .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, pattern_offset));
        }
        self.node(Node::Concat(nodes))
    }
}

pub(super) const fn is_decimal_digit(value: u16) -> bool {
    matches!(value, 0x0030..=0x0039)
}

pub(super) const fn is_octal_digit(value: u16) -> bool {
    matches!(value, 0x0030..=0x0037)
}

pub(super) fn control_letter_value(value: u16) -> Option<u32> {
    match value {
        0x0041..=0x005A => Some(u32::from(value - 0x0040)),
        0x0061..=0x007A => Some(u32::from(value - 0x0060)),
        _ => None,
    }
}

fn combine_surrogates(high: u32, low: u32, pattern_offset: usize) -> Result<u32, CompileError> {
    let shifted = high
        .checked_sub(0xD800)
        .and_then(|value| value.checked_mul(0x400))
        .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, pattern_offset))?;
    low.checked_sub(0xDC00)
        .and_then(|value| value.checked_add(shifted))
        .and_then(|value| value.checked_add(0x1_0000))
        .ok_or_else(|| CompileError::new(CompileErrorKind::SizeOverflow, pattern_offset))
}

const fn hex_value(value: u16) -> Option<u32> {
    match value {
        0x0030..=0x0039 => Some((value - 0x0030) as u32),
        0x0041..=0x0046 => Some((value - 0x0041 + 10) as u32),
        0x0061..=0x0066 => Some((value - 0x0061 + 10) as u32),
        _ => None,
    }
}
