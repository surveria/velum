use crate::{CompileError, CompileErrorKind, ast::Node, is_id_continue, is_id_start};

use super::Parser;
use super::escape_parser::combine_surrogates;

impl Parser<'_> {
    pub(super) fn parse_named_backreference(
        &mut self,
        pattern_offset: usize,
    ) -> Result<Node, CompileError> {
        if self.peek() != Some(u16::from(b'<')) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidBackreference,
                pattern_offset,
            ));
        }
        self.advance_one()?;
        let name = self.parse_capture_name()?;
        self.node(Node::NamedBackreference {
            name,
            pattern_offset,
        })
    }

    pub(super) fn parse_capture_name(&mut self) -> Result<String, CompileError> {
        let start = self.position;
        let mut name = String::new();
        let mut first = true;
        loop {
            let Some(unit) = self.peek() else {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidCaptureName,
                    start,
                ));
            };
            if unit == u16::from(b'>') {
                break;
            }
            let source_units = self
                .position
                .checked_sub(start)
                .ok_or_else(|| self.error(CompileErrorKind::SizeOverflow))?;
            if source_units >= self.limits.max_capture_name_units {
                return Err(CompileError::new(
                    CompileErrorKind::CaptureNameLimit {
                        limit: self.limits.max_capture_name_units,
                    },
                    start,
                ));
            }
            let value = self.parse_capture_name_value(start)?;
            let valid = if first {
                value == u32::from(b'$') || value == u32::from(b'_') || is_id_start(value)
            } else {
                value == u32::from(b'$')
                    || value == u32::from(b'_')
                    || value == 0x200C
                    || value == 0x200D
                    || is_id_continue(value)
            };
            if !valid {
                return Err(CompileError::new(
                    CompileErrorKind::InvalidCaptureName,
                    start,
                ));
            }
            let character = char::from_u32(value)
                .ok_or_else(|| CompileError::new(CompileErrorKind::InvalidCaptureName, start))?;
            name.push(character);
            first = false;
        }
        if first {
            return Err(CompileError::new(
                CompileErrorKind::InvalidCaptureName,
                start,
            ));
        }
        self.advance_one()?;
        Ok(name)
    }

    fn parse_capture_name_value(&mut self, start: usize) -> Result<u32, CompileError> {
        if self.peek() != Some(u16::from(b'\\')) {
            return self.decode_name_value(start);
        }
        self.advance_one()?;
        if self.peek() != Some(u16::from(b'u')) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidCaptureName,
                start,
            ));
        }
        self.advance_one()?;
        let parsed = if self.peek() == Some(u16::from(b'{')) {
            self.parse_braced_hex()
        } else {
            let value = self.parse_fixed_hex(4)?;
            if (0xD800..=0xDBFF).contains(&value)
                && let Some(low) = self.try_parse_trailing_surrogate_escape()?
            {
                combine_surrogates(value, low, start)
            } else {
                Ok(value)
            }
        };
        parsed.map_err(|_| CompileError::new(CompileErrorKind::InvalidCaptureName, start))
    }

    fn decode_name_value(&mut self, start: usize) -> Result<u32, CompileError> {
        let Some(first) = self.peek() else {
            return Err(CompileError::new(
                CompileErrorKind::InvalidCaptureName,
                start,
            ));
        };
        self.advance_one()?;
        if !(0xD800..=0xDBFF).contains(&first) {
            return Ok(u32::from(first));
        }
        let Some(second) = self.peek() else {
            return Err(CompileError::new(
                CompileErrorKind::InvalidCaptureName,
                start,
            ));
        };
        if !(0xDC00..=0xDFFF).contains(&second) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidCaptureName,
                start,
            ));
        }
        self.advance_one()?;
        let high = u32::from(first - 0xD800);
        let low = u32::from(second - 0xDC00);
        0x1_0000_u32
            .checked_add(
                high.checked_mul(0x400).ok_or_else(|| {
                    CompileError::new(CompileErrorKind::InvalidCaptureName, start)
                })?,
            )
            .and_then(|value| value.checked_add(low))
            .ok_or_else(|| CompileError::new(CompileErrorKind::InvalidCaptureName, start))
    }
}
