use super::Lexer;
use crate::{
    error::{Error, Result},
    lexer::TokenKind,
    lexer::classification::{IdentifierPosition, identifier_kind, identifier_position_allows},
    lexer::support::{is_identifier_part, is_identifier_start},
};

/// Marker character that introduces a `#name` private identifier.
const PRIVATE_NAME_MARKER: char = '#';

impl Lexer {
    pub(super) fn identifier(&mut self, offset: usize) -> Result<()> {
        let (text, escaped) = self.identifier_name_text(offset)?;
        let kind = identifier_kind(text, escaped);
        self.push_with_identifier_escape(kind, offset, escaped);
        Ok(())
    }

    /// Scans a `#name` private identifier after peeking its `#` marker per
    /// the `PrivateIdentifier :: # IdentifierName` production. Keywords are
    /// intentionally not classified: `#in` or `#static` are valid names.
    pub(super) fn private_name(&mut self, offset: usize) -> Result<()> {
        self.advance();
        let starts_name =
            self.peek_char().is_some_and(is_identifier_start) || self.identifier_escape_start();
        if !starts_name {
            return Err(Error::lex(
                "expected identifier after private name marker '#'",
                offset,
            ));
        }
        let name_offset = self.current_offset();
        let (text, _escaped) = self.identifier_name_text(name_offset)?;
        let mut name = String::with_capacity(text.len().saturating_add(1));
        name.push(PRIVATE_NAME_MARKER);
        name.push_str(&text);
        self.push(
            TokenKind::PrivateName(Rc::from(name.into_boxed_str())),
            offset,
        );
        Ok(())
    }

    /// Reads one `IdentifierName` starting at the current cursor and returns
    /// its text plus whether any unicode escape appeared inside it.
    fn identifier_name_text(&mut self, offset: usize) -> Result<(String, bool)> {
        let mut text = String::new();
        let mut escaped = self.identifier_escape_start();
        let start = self.identifier_char(offset, IdentifierPosition::Start)?;
        text.push(start);

        while let Some((current_offset, ch)) = self.peek() {
            if self.identifier_escape_start() {
                escaped = true;
                let escaped_char =
                    self.identifier_char(current_offset, IdentifierPosition::Part)?;
                text.push(escaped_char);
            } else if is_identifier_part(ch) {
                self.advance();
                text.push(ch);
            } else {
                break;
            }
        }

        Ok((text, escaped))
    }

    pub(super) fn identifier_escape_start(&self) -> bool {
        self.peek_char() == Some('\\') && self.peek_next_char() == Some('u')
    }

    fn identifier_char(&mut self, offset: usize, position: IdentifierPosition) -> Result<char> {
        let Some((current_offset, ch)) = self.peek() else {
            return Err(Error::lex("unterminated identifier", offset));
        };
        let value = if ch == '\\' {
            self.identifier_escape(current_offset)?
        } else {
            self.advance();
            ch
        };
        if identifier_position_allows(value, position) {
            return Ok(value);
        }
        Err(Error::lex(
            format!("invalid identifier character '{value}'"),
            current_offset,
        ))
    }

    fn identifier_escape(&mut self, slash_offset: usize) -> Result<char> {
        self.advance();
        let Some((escape_offset, escaped)) = self.peek() else {
            return Err(Error::lex("unterminated identifier escape", slash_offset));
        };
        if escaped != 'u' {
            return Err(Error::lex(
                "identifier escape must use a unicode escape",
                escape_offset,
            ));
        }
        self.advance();
        self.unicode_escape(escape_offset)
    }
}
use alloc::rc::Rc;
