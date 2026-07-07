use crate::{
    error::{Error, Result},
    lexer::support::{
        ASCII_BACKSPACE, ASCII_FORM_FEED, ASCII_VERTICAL_TAB, BIGINT_SUFFIX, DECIMAL_POINT,
        HEX_ESCAPE_DIGITS, LINE_SEPARATOR, MAX_BRACED_UNICODE_ESCAPE_DIGITS,
        MAX_UNICODE_CODE_POINT, NUMERIC_SEPARATOR, PARAGRAPH_SEPARATOR, RADIX_BINARY,
        RADIX_DECIMAL, RADIX_HEX, RADIX_OCTAL, TEMPLATE_SUBSTITUTION_START, UNICODE_ESCAPE_DIGITS,
        checked_hex_accumulate, digit_value, digits_to_number, is_exponent_marker,
        is_identifier_part, is_identifier_start, unicode_char,
    },
};

use super::{Token, TokenKind};

pub fn lex(source: &str) -> Result<Vec<Token>> {
    Lexer::new(source).lex()
}

struct Lexer<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    cursor: usize,
    tokens: Vec<Token>,
    line_terminator_before: bool,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum EscapeContext {
    String,
    Template,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.char_indices().collect(),
            cursor: 0,
            tokens: Vec::new(),
            line_terminator_before: false,
        }
    }

    fn lex(mut self) -> Result<Vec<Token>> {
        while let Some((offset, ch)) = self.peek() {
            match ch {
                ch if ch.is_whitespace() => {
                    if is_line_terminator(ch) {
                        self.line_terminator_before = true;
                    }
                    self.advance();
                }
                '/' if self.peek_next_char() == Some('/') => self.line_comment(),
                '/' if self.peek_next_char() == Some('*') => self.block_comment(offset)?,
                '0'..='9' => self.number(offset)?,
                '"' | '\'' => self.string(offset, ch)?,
                '`' => self.template_literal(offset)?,
                ch if is_identifier_start(ch) => self.identifier(offset)?,
                '+' => self.plus_or_increment(offset),
                '-' => self.minus_or_decrement(offset),
                '*' => self.star_or_power(offset),
                '/' => self.simple_or_equal(offset, TokenKind::Slash, TokenKind::SlashEqual),
                '%' => self.simple_or_equal(offset, TokenKind::Percent, TokenKind::PercentEqual),
                '?' => self.simple(TokenKind::Question),
                ':' => self.simple(TokenKind::Colon),
                '.' if matches!(self.peek_next_char(), Some('0'..='9')) => {
                    self.leading_decimal_number(offset)?;
                }
                '.' => self.simple(TokenKind::Dot),
                '(' => self.simple(TokenKind::LParen),
                ')' => self.simple(TokenKind::RParen),
                '{' => self.simple(TokenKind::LBrace),
                '}' => self.simple(TokenKind::RBrace),
                '[' => self.simple(TokenKind::LBracket),
                ']' => self.simple(TokenKind::RBracket),
                ';' => self.simple(TokenKind::Semicolon),
                ',' => self.simple(TokenKind::Comma),
                '!' => {
                    self.advance();
                    if self.match_char('=') {
                        if self.match_char('=') {
                            self.push(TokenKind::StrictNotEqual, offset);
                        } else {
                            self.push(TokenKind::BangEqual, offset);
                        }
                    } else {
                        self.push(TokenKind::Bang, offset);
                    }
                }
                '=' => {
                    self.advance();
                    if self.match_char('>') {
                        self.push(TokenKind::Arrow, offset);
                    } else if self.match_char('=') {
                        if self.match_char('=') {
                            self.push(TokenKind::StrictEqual, offset);
                        } else {
                            self.push(TokenKind::EqualEqual, offset);
                        }
                    } else {
                        self.push(TokenKind::Equal, offset);
                    }
                }
                '<' => self.less_token(offset),
                '>' => self.greater_token(offset),
                '&' => {
                    self.advance();
                    if self.match_char('&') {
                        self.push(TokenKind::AndAnd, offset);
                    } else if self.match_char('=') {
                        self.push(TokenKind::AmpersandEqual, offset);
                    } else {
                        self.push(TokenKind::Ampersand, offset);
                    }
                }
                '|' => {
                    self.advance();
                    if self.match_char('|') {
                        self.push(TokenKind::OrOr, offset);
                    } else if self.match_char('=') {
                        self.push(TokenKind::PipeEqual, offset);
                    } else {
                        self.push(TokenKind::Pipe, offset);
                    }
                }
                '^' => self.simple_or_equal(offset, TokenKind::Caret, TokenKind::CaretEqual),
                _ => return Err(Error::lex(format!("unexpected character '{ch}'"), offset)),
            }
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            offset: self.source.len(),
            line_terminator_before: self.line_terminator_before,
        });
        Ok(self.tokens)
    }

    fn number(&mut self, offset: usize) -> Result<()> {
        if self.peek_char() == Some('0')
            && let Some((radix, description)) = self.numeric_prefix()
        {
            return self.prefixed_number(offset, radix, description);
        }
        self.decimal_number(offset)
    }

    fn numeric_prefix(&self) -> Option<(u32, &'static str)> {
        match self.peek_next_char()? {
            'b' | 'B' => Some((RADIX_BINARY, "binary numeric literal")),
            'o' | 'O' => Some((RADIX_OCTAL, "octal numeric literal")),
            'x' | 'X' => Some((RADIX_HEX, "hexadecimal numeric literal")),
            _ => None,
        }
    }

    fn prefixed_number(&mut self, offset: usize, radix: u32, description: &str) -> Result<()> {
        self.advance();
        self.advance();
        let digits = self.digit_sequence(radix, offset, description)?;
        self.reject_bigint_suffix(description)?;
        let value = digits_to_number(&digits, radix, offset, description)?;
        self.push(TokenKind::Number(value), offset);
        Ok(())
    }

    fn decimal_number(&mut self, offset: usize) -> Result<()> {
        let mut text = self.digit_sequence(RADIX_DECIMAL, offset, "decimal numeric literal")?;

        if self.peek_char() == Some(DECIMAL_POINT)
            && matches!(self.peek_next_char(), Some('0'..='9'))
        {
            self.advance();
            text.push(DECIMAL_POINT);
            let fraction =
                self.digit_sequence(RADIX_DECIMAL, offset, "decimal fraction literal")?;
            text.push_str(&fraction);
        }

        if self.peek_char().is_some_and(is_exponent_marker) {
            self.decimal_exponent(&mut text, offset)?;
        }

        self.reject_bigint_suffix("decimal numeric literal")?;
        let value = text
            .parse::<f64>()
            .map_err(|_| Error::lex("invalid decimal numeric literal", offset))?;
        self.push(TokenKind::Number(value), offset);
        Ok(())
    }

    fn leading_decimal_number(&mut self, offset: usize) -> Result<()> {
        self.advance();
        let mut text = "0.".to_owned();
        let fraction = self.digit_sequence(RADIX_DECIMAL, offset, "decimal fraction literal")?;
        text.push_str(&fraction);

        if self.peek_char().is_some_and(is_exponent_marker) {
            self.decimal_exponent(&mut text, offset)?;
        }

        self.reject_bigint_suffix("decimal numeric literal")?;
        let value = text
            .parse::<f64>()
            .map_err(|_| Error::lex("invalid decimal numeric literal", offset))?;
        self.push(TokenKind::Number(value), offset);
        Ok(())
    }

    fn decimal_exponent(&mut self, text: &mut String, offset: usize) -> Result<()> {
        let Some((exponent_offset, marker)) = self.peek() else {
            return Err(Error::lex("decimal exponent requires marker", offset));
        };
        self.advance();
        text.push(marker);
        if let Some(sign @ ('+' | '-')) = self.peek_char() {
            self.advance();
            text.push(sign);
        }
        let digits =
            self.digit_sequence(RADIX_DECIMAL, exponent_offset, "decimal exponent literal")?;
        text.push_str(&digits);
        Ok(())
    }

    fn digit_sequence(&mut self, radix: u32, offset: usize, description: &str) -> Result<String> {
        let mut output = String::new();
        let mut seen_digit = false;
        let mut previous_separator = false;

        while let Some((current_offset, ch)) = self.peek() {
            if ch == NUMERIC_SEPARATOR {
                if !seen_digit || previous_separator {
                    return Err(Error::lex(
                        format!("{description} has misplaced numeric separator"),
                        current_offset,
                    ));
                }
                let Some(next) = self.peek_next_char() else {
                    return Err(Error::lex(
                        format!("{description} separator must be followed by a digit"),
                        current_offset,
                    ));
                };
                if digit_value(next, radix).is_none() {
                    return Err(Error::lex(
                        format!("{description} separator must be followed by a digit"),
                        current_offset,
                    ));
                }
                previous_separator = true;
                self.advance();
                continue;
            }
            if digit_value(ch, radix).is_some() {
                seen_digit = true;
                previous_separator = false;
                output.push(ch);
                self.advance();
                continue;
            }
            break;
        }

        if seen_digit {
            return Ok(output);
        }
        Err(Error::lex(
            format!("{description} requires at least one digit"),
            offset,
        ))
    }

    fn reject_bigint_suffix(&self, description: &str) -> Result<()> {
        if self.peek_char() == Some(BIGINT_SUFFIX) {
            return Err(Error::lex(
                format!("{description} cannot use BigInt suffix without BigInt support"),
                self.current_offset(),
            ));
        }
        Ok(())
    }

    fn string(&mut self, offset: usize, quote: char) -> Result<()> {
        self.advance();
        let mut output = String::new();

        while let Some((current_offset, ch)) = self.peek() {
            self.advance();
            match ch {
                ch if ch == quote => {
                    self.push(TokenKind::String(output), offset);
                    return Ok(());
                }
                '\\' => self.string_escape(current_offset, &mut output)?,
                '\n' | '\r' => return Err(Error::lex("unterminated string literal", offset)),
                other => output.push(other),
            }
        }

        Err(Error::lex("unterminated string literal", offset))
    }

    fn string_escape(&mut self, slash_offset: usize, output: &mut String) -> Result<()> {
        self.escape_sequence(slash_offset, output, EscapeContext::String)
    }

    fn template_literal(&mut self, offset: usize) -> Result<()> {
        self.advance();
        let mut output = String::new();

        while let Some((current_offset, ch)) = self.peek() {
            self.advance();
            match ch {
                '`' => {
                    self.push(TokenKind::String(output), offset);
                    return Ok(());
                }
                '$' if self.peek_char() == Some(TEMPLATE_SUBSTITUTION_START) => {
                    return Err(Error::lex(
                        "template literal substitutions are not supported",
                        current_offset,
                    ));
                }
                '\\' => self.template_escape(current_offset, &mut output)?,
                '\n' => output.push('\n'),
                '\r' => {
                    if self.peek_char() == Some('\n') {
                        self.advance();
                    }
                    output.push('\n');
                }
                LINE_SEPARATOR | PARAGRAPH_SEPARATOR => output.push(ch),
                other => output.push(other),
            }
        }

        Err(Error::lex("unterminated template literal", offset))
    }

    fn template_escape(&mut self, slash_offset: usize, output: &mut String) -> Result<()> {
        self.escape_sequence(slash_offset, output, EscapeContext::Template)
    }

    fn escape_sequence(
        &mut self,
        slash_offset: usize,
        output: &mut String,
        context: EscapeContext,
    ) -> Result<()> {
        let Some((escape_offset, escaped)) = self.peek() else {
            return Err(Error::lex("unterminated escape sequence", slash_offset));
        };
        self.advance();
        match escaped {
            'b' => output.push(ASCII_BACKSPACE),
            'f' => output.push(ASCII_FORM_FEED),
            'n' => output.push('\n'),
            'r' => output.push('\r'),
            't' => output.push('\t'),
            'v' => output.push(ASCII_VERTICAL_TAB),
            '0' => self.zero_escape(escape_offset, output)?,
            'x' => output.push(self.fixed_hex_escape(
                escape_offset,
                HEX_ESCAPE_DIGITS,
                "hex escape",
            )?),
            'u' => output.push(self.unicode_escape(escape_offset)?),
            '\\' => output.push('\\'),
            '"' => output.push('"'),
            '\'' => output.push('\''),
            '`' if context == EscapeContext::Template => output.push('`'),
            '$' if context == EscapeContext::Template => output.push('$'),
            '\n' | LINE_SEPARATOR | PARAGRAPH_SEPARATOR => {}
            '\r' => {
                if self.peek_char() == Some('\n') {
                    self.advance();
                }
            }
            other => {
                return Err(Error::lex(
                    format!("unsupported escape sequence '\\{other}'"),
                    escape_offset,
                ));
            }
        }
        Ok(())
    }

    fn zero_escape(&self, escape_offset: usize, output: &mut String) -> Result<()> {
        if self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
            return Err(Error::lex(
                "legacy octal escape sequences are not supported",
                escape_offset,
            ));
        }
        output.push('\0');
        Ok(())
    }

    fn unicode_escape(&mut self, escape_offset: usize) -> Result<char> {
        if self.match_char('{') {
            return self.braced_unicode_escape(escape_offset);
        }
        self.fixed_hex_escape(escape_offset, UNICODE_ESCAPE_DIGITS, "unicode escape")
    }

    fn braced_unicode_escape(&mut self, escape_offset: usize) -> Result<char> {
        let mut value = 0u32;
        let mut digits = 0usize;
        loop {
            let Some((digit_offset, ch)) = self.peek() else {
                return Err(Error::lex(
                    "unterminated braced unicode escape",
                    escape_offset,
                ));
            };
            if ch == '}' {
                return self.finish_braced_unicode_escape(escape_offset, value, digits);
            }
            if digits >= MAX_BRACED_UNICODE_ESCAPE_DIGITS {
                return Err(Error::lex(
                    "braced unicode escape has too many digits",
                    digit_offset,
                ));
            }
            let Some(digit) = ch.to_digit(16) else {
                return Err(Error::lex(
                    format!("braced unicode escape has non-hex digit '{ch}'"),
                    digit_offset,
                ));
            };
            self.advance();
            digits = digits.saturating_add(1);
            value = checked_hex_accumulate(value, digit, digit_offset, "braced unicode escape")?;
            if value > MAX_UNICODE_CODE_POINT {
                return Err(Error::lex(
                    "braced unicode escape exceeds maximum code point",
                    digit_offset,
                ));
            }
        }
    }

    fn finish_braced_unicode_escape(
        &mut self,
        escape_offset: usize,
        value: u32,
        digits: usize,
    ) -> Result<char> {
        if digits == 0 {
            return Err(Error::lex("empty braced unicode escape", escape_offset));
        }
        self.advance();
        unicode_char(value, escape_offset, "braced unicode escape")
    }

    fn fixed_hex_escape(
        &mut self,
        escape_offset: usize,
        digits: usize,
        description: &str,
    ) -> Result<char> {
        let value = self.hex_digits(escape_offset, digits, description)?;
        unicode_char(value, escape_offset, description)
    }

    fn hex_digits(
        &mut self,
        escape_offset: usize,
        digits: usize,
        description: &str,
    ) -> Result<u32> {
        let mut value = 0u32;
        for _ in 0..digits {
            let Some((digit_offset, ch)) = self.peek() else {
                return Err(Error::lex(
                    format!("{description} requires {digits} hex digits"),
                    escape_offset,
                ));
            };
            let Some(digit) = ch.to_digit(16) else {
                return Err(Error::lex(
                    format!("{description} has non-hex digit '{ch}'"),
                    digit_offset,
                ));
            };
            self.advance();
            value = checked_hex_accumulate(value, digit, digit_offset, description)?;
        }
        Ok(value)
    }

    fn identifier(&mut self, offset: usize) -> Result<()> {
        let start = self.cursor;
        self.advance();
        while self.peek_char().is_some_and(is_identifier_part) {
            self.advance();
        }

        let start_offset = self.char_offset(start, offset, "identifier")?;
        let end_offset = self.current_offset();
        let text = self.source_slice(start_offset, end_offset, offset, "identifier")?;
        let kind = match text {
            "let" => TokenKind::Let,
            "const" => TokenKind::Const,
            "var" => TokenKind::Var,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "switch" => TokenKind::Switch,
            "case" => TokenKind::Case,
            "default" => TokenKind::Default,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "try" => TokenKind::Try,
            "catch" => TokenKind::Catch,
            "finally" => TokenKind::Finally,
            "throw" => TokenKind::Throw,
            "return" => TokenKind::Return,
            "function" => TokenKind::Function,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "new" => TokenKind::New,
            "this" => TokenKind::This,
            "in" => TokenKind::In,
            "instanceof" => TokenKind::InstanceOf,
            "typeof" => TokenKind::Typeof,
            "void" => TokenKind::Void,
            "delete" => TokenKind::Delete,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "undefined" => TokenKind::Undefined,
            _ => TokenKind::Identifier(text.to_owned()),
        };
        self.push(kind, offset);
        Ok(())
    }

    fn line_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            self.advance();
            if is_line_terminator(ch) {
                self.line_terminator_before = true;
                break;
            }
        }
    }

    fn block_comment(&mut self, offset: usize) -> Result<()> {
        self.advance();
        self.advance();

        while self.peek().is_some() {
            if self.peek_char() == Some('*') && self.peek_next_char() == Some('/') {
                self.advance();
                self.advance();
                return Ok(());
            }
            if self.peek_char().is_some_and(is_line_terminator) {
                self.line_terminator_before = true;
            }
            self.advance();
        }

        Err(Error::lex("unterminated block comment", offset))
    }

    fn simple(&mut self, kind: TokenKind) {
        let offset = self.peek().map_or(self.source.len(), |(offset, _)| offset);
        self.advance();
        self.push(kind, offset);
    }

    fn plus_or_increment(&mut self, offset: usize) {
        self.advance();
        if self.match_char('+') {
            self.push(TokenKind::PlusPlus, offset);
        } else if self.match_char('=') {
            self.push(TokenKind::PlusEqual, offset);
        } else {
            self.push(TokenKind::Plus, offset);
        }
    }

    fn minus_or_decrement(&mut self, offset: usize) {
        self.advance();
        if self.match_char('-') {
            self.push(TokenKind::MinusMinus, offset);
        } else if self.match_char('=') {
            self.push(TokenKind::MinusEqual, offset);
        } else {
            self.push(TokenKind::Minus, offset);
        }
    }

    fn star_or_power(&mut self, offset: usize) {
        self.advance();
        if !self.match_char('*') {
            if self.match_char('=') {
                self.push(TokenKind::StarEqual, offset);
            } else {
                self.push(TokenKind::Star, offset);
            }
            return;
        }
        if self.match_char('=') {
            self.push(TokenKind::StarStarEqual, offset);
        } else {
            self.push(TokenKind::StarStar, offset);
        }
    }

    fn less_token(&mut self, offset: usize) {
        self.advance();
        if self.match_char('<') {
            if self.match_char('=') {
                self.push(TokenKind::LessLessEqual, offset);
            } else {
                self.push(TokenKind::LessLess, offset);
            }
        } else if self.match_char('=') {
            self.push(TokenKind::LessEqual, offset);
        } else {
            self.push(TokenKind::Less, offset);
        }
    }

    fn greater_token(&mut self, offset: usize) {
        self.advance();
        if self.match_char('>') {
            self.greater_shift_token(offset);
        } else if self.match_char('=') {
            self.push(TokenKind::GreaterEqual, offset);
        } else {
            self.push(TokenKind::Greater, offset);
        }
    }

    fn greater_shift_token(&mut self, offset: usize) {
        if self.match_char('>') {
            if self.match_char('=') {
                self.push(TokenKind::GreaterGreaterGreaterEqual, offset);
            } else {
                self.push(TokenKind::GreaterGreaterGreater, offset);
            }
        } else if self.match_char('=') {
            self.push(TokenKind::GreaterGreaterEqual, offset);
        } else {
            self.push(TokenKind::GreaterGreater, offset);
        }
    }

    fn simple_or_equal(&mut self, offset: usize, plain: TokenKind, assigned: TokenKind) {
        self.advance();
        if self.match_char('=') {
            self.push(assigned, offset);
        } else {
            self.push(plain, offset);
        }
    }

    fn push(&mut self, kind: TokenKind, offset: usize) {
        self.tokens.push(Token {
            kind,
            offset,
            line_terminator_before: self.line_terminator_before,
        });
        self.line_terminator_before = false;
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let value = self.peek();
        if value.is_some() {
            self.cursor = self.cursor.saturating_add(1);
        }
        value
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<(usize, char)> {
        self.chars.get(self.cursor).copied()
    }

    fn peek_char(&self) -> Option<char> {
        self.peek().map(|(_, ch)| ch)
    }

    fn peek_next_char(&self) -> Option<char> {
        let next_cursor = self.cursor.checked_add(1)?;
        self.chars.get(next_cursor).map(|(_, ch)| *ch)
    }

    fn current_offset(&self) -> usize {
        self.chars
            .get(self.cursor)
            .map_or(self.source.len(), |(offset, _)| *offset)
    }

    fn char_offset(&self, cursor: usize, offset: usize, description: &str) -> Result<usize> {
        self.chars
            .get(cursor)
            .map(|(offset, _)| *offset)
            .ok_or_else(|| Error::lex(format!("invalid {description} start"), offset))
    }

    fn source_slice(
        &self,
        start: usize,
        end: usize,
        offset: usize,
        description: &str,
    ) -> Result<&str> {
        self.source
            .get(start..end)
            .ok_or_else(|| Error::lex(format!("invalid {description} span"), offset))
    }
}

const fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | LINE_SEPARATOR | PARAGRAPH_SEPARATOR)
}
