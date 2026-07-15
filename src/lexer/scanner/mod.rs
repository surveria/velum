use std::rc::Rc;

use super::{
    LexicalGoal, NumberToken, SourceText, StringToken, Token, TokenKind,
    template::TemplateSubstitutionState,
};
use crate::{
    error::{Error, Result},
    lexer::classification::EscapeContext,
    lexer::support::{
        ASCII_BACKSPACE, ASCII_FORM_FEED, ASCII_VERTICAL_TAB, BIGINT_SUFFIX, DECIMAL_POINT,
        HEX_ESCAPE_DIGITS, LINE_SEPARATOR, MAX_UNICODE_CODE_POINT, NUMERIC_SEPARATOR,
        PARAGRAPH_SEPARATOR, RADIX_DECIMAL, RADIX_OCTAL, UNICODE_ESCAPE_DIGITS, append_utf16_value,
        checked_hex_accumulate, digit_value, digits_to_number, is_exponent_marker,
        is_identifier_part, is_identifier_start, is_line_terminator, numeric_prefix,
        push_utf16_char, unicode_char,
    },
    regexp_syntax::validate_regexp_literal_utf16,
    source::{SourceId, SourceSpan},
    value::JsBigInt,
};
mod names;
mod operators;
mod template;

const ZERO_WIDTH_NO_BREAK_SPACE: char = '\u{FEFF}';

#[derive(Clone)]
pub(super) struct LexerCheckpoint {
    cursor: usize,
    line_terminator_before: bool,
    line_start: bool,
    template_substitutions: Vec<TemplateSubstitutionState>,
}

pub(super) struct Lexer {
    source: SourceText,
    source_id: SourceId,
    cursor: usize,
    pending: Option<Token>,
    line_terminator_before: bool,
    line_start: bool,
    allow_html_comments: bool,
    template_substitutions: Vec<TemplateSubstitutionState>,
}

impl Lexer {
    pub(super) const fn new(source: SourceText, source_id: SourceId, html_comments: bool) -> Self {
        Self {
            source,
            source_id,
            cursor: 0,
            pending: None,
            line_terminator_before: false,
            line_start: true,
            allow_html_comments: html_comments,
            template_substitutions: Vec::new(),
        }
    }

    pub(super) fn checkpoint(&self) -> LexerCheckpoint {
        LexerCheckpoint {
            cursor: self.cursor,
            line_terminator_before: self.line_terminator_before,
            line_start: self.line_start,
            template_substitutions: self.template_substitutions.clone(),
        }
    }

    pub(super) fn restore(&mut self, checkpoint: &LexerCheckpoint) {
        self.cursor = checkpoint.cursor;
        self.line_terminator_before = checkpoint.line_terminator_before;
        self.line_start = checkpoint.line_start;
        self.template_substitutions
            .clone_from(&checkpoint.template_substitutions);
        self.pending = None;
    }

    pub(super) fn is_slash_offset(&self, offset: usize) -> bool {
        self.source
            .rendered()
            .get(offset..)
            .is_some_and(|suffix| suffix.starts_with('/'))
    }

    pub(super) fn next_token(&mut self, goal: LexicalGoal) -> Result<Token> {
        loop {
            let Some((offset, ch)) = self.peek() else {
                if let Some(substitution) = self.template_substitutions.last() {
                    return Err(Error::lex(
                        "unterminated template literal substitution",
                        substitution.substitution_offset,
                    ));
                }
                return Ok(Token {
                    kind: TokenKind::Eof,
                    span: SourceSpan::point(self.source_id, self.source.rendered_len()),
                    line_terminator_before: self.line_terminator_before,
                    identifier_escaped: false,
                });
            };
            match ch {
                ch if ch.is_whitespace() || ch == ZERO_WIDTH_NO_BREAK_SPACE => {
                    if is_line_terminator(ch) {
                        self.line_terminator_before = true;
                        self.line_start = true;
                    }
                    self.advance();
                }
                '#' if self.cursor == 0 && self.peek_next_char() == Some('!') => {
                    self.hashbang_comment();
                }
                '#' => self.private_name(offset)?,
                '/' if self.peek_next_char() == Some('/') => self.line_comment(),
                '/' if self.peek_next_char() == Some('*') => self.block_comment(offset)?,
                '<' if self.allow_html_comments && self.remaining_source_starts_with("<!--") => {
                    self.line_comment();
                }
                '-' if self.allow_html_comments
                    && self.line_start
                    && self.remaining_source_starts_with("-->") =>
                {
                    self.line_comment();
                }
                '0'..='9' => self.number(offset)?,
                '"' | '\'' => self.string(offset, ch)?,
                '`' => self.template_literal(offset)?,
                ch if is_identifier_start(ch) || self.identifier_escape_start() => {
                    self.identifier(offset)?;
                }
                '+' => self.plus_or_increment(offset),
                '-' => self.minus_or_decrement(offset),
                '*' => self.star_or_power(offset),
                '/' => self.slash_token(offset, goal)?,
                '%' => self.simple_or_equal(offset, TokenKind::Percent, TokenKind::PercentEqual),
                '?' => self.question_token(offset),
                ':' => self.simple(TokenKind::Colon),
                '.' if matches!(self.peek_next_char(), Some('0'..='9')) => {
                    self.leading_decimal_number(offset)?;
                }
                '.' => self.dot_token(offset)?,
                '(' => self.simple(TokenKind::LParen),
                ')' => self.simple(TokenKind::RParen),
                '{' => {
                    self.substitution_brace_open(offset)?;
                    self.simple(TokenKind::LBrace);
                }
                '}' => self.right_brace_or_template_continuation(offset)?,
                '[' => self.simple(TokenKind::LBracket),
                ']' => self.simple(TokenKind::RBracket),
                ';' => self.simple(TokenKind::Semicolon),
                ',' => self.simple(TokenKind::Comma),
                '!' => self.bang_token(offset),
                '~' => self.simple(TokenKind::Tilde),
                '=' => self.equal_token(offset),
                '<' => self.less_token(offset),
                '>' => self.greater_token(offset),
                '&' => self.ampersand_token(offset),
                '|' => self.pipe_token(offset),
                '^' => self.simple_or_equal(offset, TokenKind::Caret, TokenKind::CaretEqual),
                '@' => self.simple(TokenKind::At),
                _ => return Err(Error::lex(format!("unexpected character '{ch}'"), offset)),
            }
            if let Some(token) = self.pending.take() {
                return Ok(token);
            }
        }
    }

    fn number(&mut self, offset: usize) -> Result<()> {
        if self.peek_char() == Some('0')
            && let Some((radix, description)) = numeric_prefix(self.peek_next_char())
        {
            return self.prefixed_number(offset, radix, description);
        }
        self.decimal_number(offset)
    }

    fn slash_token(&mut self, offset: usize, goal: LexicalGoal) -> Result<()> {
        match goal {
            LexicalGoal::Div => {
                self.simple_or_equal(offset, TokenKind::Slash, TokenKind::SlashEqual);
                Ok(())
            }
            LexicalGoal::RegExp => self.regexp_literal(offset),
        }
    }

    fn regexp_literal(&mut self, offset: usize) -> Result<()> {
        self.advance();
        let mut pattern = Vec::new();
        let mut in_class = false;
        let mut escaped = false;
        while let Some((current_offset, ch)) = self.peek() {
            if is_line_terminator(ch) {
                return Err(Error::lex(
                    "unterminated regular expression literal",
                    offset,
                ));
            }
            self.advance();
            let surrogate = self.source.surrogate_at(current_offset);
            if escaped {
                if let Some(unit) = surrogate {
                    pattern.push(unit);
                } else {
                    push_utf16_char(&mut pattern, ch);
                }
                escaped = false;
                continue;
            }
            match ch {
                '\\' => {
                    push_utf16_char(&mut pattern, ch);
                    escaped = true;
                }
                '[' => {
                    push_utf16_char(&mut pattern, ch);
                    in_class = true;
                }
                ']' => {
                    push_utf16_char(&mut pattern, ch);
                    in_class = false;
                }
                '/' if !in_class => {
                    let flags = self.regexp_flags();
                    validate_regexp_literal_utf16(&pattern, &flags)
                        .map_err(|error| Error::lex(error.to_string(), offset))?;
                    self.push(
                        TokenKind::RegExp {
                            pattern: Rc::from(pattern.into_boxed_slice()),
                            flags: flags.into(),
                        },
                        offset,
                    );
                    return Ok(());
                }
                _ => {
                    if let Some(unit) = surrogate {
                        pattern.push(unit);
                    } else {
                        push_utf16_char(&mut pattern, ch);
                    }
                }
            }
        }
        Err(Error::lex(
            "unterminated regular expression literal",
            offset,
        ))
    }

    fn regexp_flags(&mut self) -> String {
        let mut flags = String::new();
        while let Some((_, ch)) = self.peek() {
            if !is_identifier_part(ch) {
                break;
            }
            flags.push(ch);
            self.advance();
        }
        flags
    }

    fn prefixed_number(&mut self, offset: usize, radix: u32, description: &str) -> Result<()> {
        self.advance();
        self.advance();
        let digits = self.digit_sequence(radix, offset, description)?;
        if self.consume_bigint_suffix() {
            let value = JsBigInt::parse_digits(&digits, radix)
                .ok_or_else(|| Error::lex(format!("invalid {description}"), offset))?;
            self.push(TokenKind::BigInt(value), offset);
            return Ok(());
        }
        let value = digits_to_number(&digits, radix, offset, description)?;
        self.reject_numeric_literal_continuation()?;
        self.push(
            TokenKind::Number(NumberToken {
                value,
                legacy: false,
            }),
            offset,
        );
        Ok(())
    }

    fn decimal_number(&mut self, offset: usize) -> Result<()> {
        let mut text = self.digit_sequence(RADIX_DECIMAL, offset, "decimal numeric literal")?;
        let integer_end = self.cursor;
        let leading_zero = text.len() > 1 && text.starts_with('0');
        let integer_has_separator = self
            .source
            .rendered()
            .get(offset..integer_end)
            .is_some_and(|source| source.contains(NUMERIC_SEPARATOR));

        if leading_zero && integer_has_separator {
            return Err(Error::lex(
                "legacy-style numeric literal cannot contain a numeric separator",
                offset,
            ));
        }

        if self.consume_bigint_suffix() {
            if leading_zero {
                return Err(Error::lex(
                    "decimal BigInt literal cannot have a leading zero",
                    offset,
                ));
            }
            let value = JsBigInt::parse_digits(&text, RADIX_DECIMAL)
                .ok_or_else(|| Error::lex("invalid decimal BigInt literal", offset))?;
            self.reject_numeric_literal_continuation()?;
            self.push(TokenKind::BigInt(value), offset);
            return Ok(());
        }

        if self.peek_char() == Some(DECIMAL_POINT) {
            if leading_zero {
                return Err(Error::lex(
                    "decimal literal cannot extend a legacy-style integer",
                    offset,
                ));
            }
            self.advance();
            text.push(DECIMAL_POINT);
            if matches!(self.peek_char(), Some('0'..='9')) {
                let fraction =
                    self.digit_sequence(RADIX_DECIMAL, offset, "decimal fraction literal")?;
                text.push_str(&fraction);
            }
        }

        if self.peek_char().is_some_and(is_exponent_marker) {
            if leading_zero {
                return Err(Error::lex(
                    "decimal literal cannot extend a legacy-style integer",
                    offset,
                ));
            }
            self.decimal_exponent(&mut text, offset)?;
        }

        self.reject_bigint_suffix("decimal numeric literal")?;
        self.reject_numeric_literal_continuation()?;
        let value = if leading_zero && text.chars().all(|ch| matches!(ch, '0'..='7')) {
            digits_to_number(&text, RADIX_OCTAL, offset, "legacy octal numeric literal")?
        } else {
            text.parse::<f64>()
                .map_err(|_| Error::lex("invalid decimal numeric literal", offset))?
        };
        self.push(
            TokenKind::Number(NumberToken {
                value,
                legacy: leading_zero,
            }),
            offset,
        );
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
        self.reject_numeric_literal_continuation()?;
        let value = text
            .parse::<f64>()
            .map_err(|_| Error::lex("invalid decimal numeric literal", offset))?;
        self.push(
            TokenKind::Number(NumberToken {
                value,
                legacy: false,
            }),
            offset,
        );
        Ok(())
    }

    fn reject_numeric_literal_continuation(&self) -> Result<()> {
        let Some((offset, ch)) = self.peek() else {
            return Ok(());
        };
        if ch.is_ascii_digit() || is_identifier_start(ch) || ch == '\\' {
            return Err(Error::lex(
                "numeric literal cannot be immediately followed by an identifier or digit",
                offset,
            ));
        }
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

    fn consume_bigint_suffix(&mut self) -> bool {
        if self.peek_char() != Some(BIGINT_SUFFIX) {
            return false;
        }
        self.advance();
        true
    }

    fn string(&mut self, offset: usize, quote: char) -> Result<()> {
        self.advance();
        let mut output = Vec::new();
        let mut escape_free = true;
        let mut legacy_escape = false;

        while let Some((current_offset, ch)) = self.peek() {
            self.advance();
            if let Some(unit) = self.source.surrogate_at(current_offset) {
                output.push(unit);
                continue;
            }
            match ch {
                ch if ch == quote => {
                    self.push(
                        TokenKind::String(StringToken {
                            cooked: output.into(),
                            escape_free,
                            legacy_escape,
                        }),
                        offset,
                    );
                    return Ok(());
                }
                '\\' => {
                    escape_free = false;
                    legacy_escape |= self.string_escape(current_offset, &mut output)?;
                }
                '\n' | '\r' => return Err(Error::lex("unterminated string literal", offset)),
                other => push_utf16_char(&mut output, other),
            }
        }

        Err(Error::lex("unterminated string literal", offset))
    }

    fn string_escape(&mut self, slash_offset: usize, output: &mut Vec<u16>) -> Result<bool> {
        self.escape_sequence(slash_offset, output, EscapeContext::String)
    }

    fn escape_sequence(
        &mut self,
        slash_offset: usize,
        output: &mut Vec<u16>,
        context: EscapeContext,
    ) -> Result<bool> {
        let Some((escape_offset, escaped)) = self.peek() else {
            return Err(Error::lex("unterminated escape sequence", slash_offset));
        };
        self.advance();
        if let Some(unit) = self.source.surrogate_at(escape_offset) {
            output.push(unit);
            return Ok(false);
        }
        match escaped {
            'b' => push_utf16_char(output, ASCII_BACKSPACE),
            'f' => push_utf16_char(output, ASCII_FORM_FEED),
            'n' => push_utf16_char(output, '\n'),
            'r' => push_utf16_char(output, '\r'),
            't' => push_utf16_char(output, '\t'),
            'v' => push_utf16_char(output, ASCII_VERTICAL_TAB),
            '0'..='7' => {
                return self.legacy_octal_escape(escaped, output);
            }
            'x' => {
                let ch = self.fixed_hex_escape(escape_offset, HEX_ESCAPE_DIGITS, "hex escape")?;
                push_utf16_char(output, ch);
            }
            'u' => self.string_unicode_escape(escape_offset, output)?,
            '\\' => push_utf16_char(output, '\\'),
            '"' => push_utf16_char(output, '"'),
            '\'' => push_utf16_char(output, '\''),
            '`' if context == EscapeContext::Template => push_utf16_char(output, '`'),
            '$' if context == EscapeContext::Template => push_utf16_char(output, '$'),
            '\n' | LINE_SEPARATOR | PARAGRAPH_SEPARATOR => {}
            '\r' => {
                if self.peek_char() == Some('\n') {
                    self.advance();
                }
            }
            '8' | '9' => {
                push_utf16_char(output, escaped);
                return Ok(true);
            }
            other => push_utf16_char(output, other),
        }
        Ok(false)
    }

    fn legacy_octal_escape(&mut self, first: char, output: &mut Vec<u16>) -> Result<bool> {
        let legacy = first != '0' || self.peek_char().is_some_and(|ch| ch.is_ascii_digit());
        let mut value = first
            .to_digit(RADIX_OCTAL)
            .ok_or_else(|| Error::lex("invalid legacy octal escape", self.current_offset()))?;
        let max_additional_digits = if matches!(first, '0'..='3') { 2 } else { 1 };
        for _ in 0..max_additional_digits {
            let Some(digit) = self.peek_char().and_then(|ch| ch.to_digit(RADIX_OCTAL)) else {
                break;
            };
            self.advance();
            value = value
                .checked_mul(RADIX_OCTAL)
                .and_then(|current| current.checked_add(digit))
                .ok_or_else(|| {
                    Error::lex("legacy octal escape overflowed", self.current_offset())
                })?;
        }
        let ch = char::from_u32(value)
            .ok_or_else(|| Error::lex("invalid legacy octal escape", self.current_offset()))?;
        push_utf16_char(output, ch);
        Ok(legacy)
    }

    fn string_unicode_escape(&mut self, escape_offset: usize, output: &mut Vec<u16>) -> Result<()> {
        let value = if self.match_char('{') {
            self.braced_unicode_escape_value(escape_offset)?
        } else {
            self.hex_digits(escape_offset, UNICODE_ESCAPE_DIGITS, "unicode escape")?
        };
        append_utf16_value(output, value, escape_offset)
    }

    fn unicode_escape(&mut self, escape_offset: usize) -> Result<char> {
        if self.match_char('{') {
            let value = self.braced_unicode_escape_value(escape_offset)?;
            return unicode_char(value, escape_offset, "braced unicode escape");
        }
        self.fixed_hex_escape(escape_offset, UNICODE_ESCAPE_DIGITS, "unicode escape")
    }

    fn braced_unicode_escape_value(&mut self, escape_offset: usize) -> Result<u32> {
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
                return self.finish_braced_unicode_escape_value(escape_offset, value, digits);
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

    fn finish_braced_unicode_escape_value(
        &mut self,
        escape_offset: usize,
        value: u32,
        digits: usize,
    ) -> Result<u32> {
        if digits == 0 {
            return Err(Error::lex("empty braced unicode escape", escape_offset));
        }
        self.advance();
        Ok(value)
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

    fn hashbang_comment(&mut self) {
        self.advance();
        self.advance();
        self.line_comment();
    }

    fn line_comment(&mut self) {
        while let Some(ch) = self.peek_char() {
            self.advance();
            if is_line_terminator(ch) {
                self.line_terminator_before = true;
                self.line_start = true;
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
                self.line_start = true;
            }
            self.advance();
        }

        Err(Error::lex("unterminated block comment", offset))
    }

    fn push(&mut self, kind: TokenKind, offset: usize) {
        self.push_with_identifier_escape(kind, offset, false);
    }

    fn push_with_identifier_escape(
        &mut self,
        kind: TokenKind,
        offset: usize,
        identifier_escaped: bool,
    ) {
        let span = SourceSpan::from_valid_bounds(self.source_id, offset, self.current_offset());
        self.pending = Some(Token {
            kind,
            span,
            line_terminator_before: self.line_terminator_before,
            identifier_escaped,
        });
        self.line_terminator_before = false;
        self.line_start = false;
    }

    fn advance(&mut self) -> Option<(usize, char)> {
        let (offset, ch) = self.peek()?;
        self.cursor = self.cursor.saturating_add(ch.len_utf8());
        Some((offset, ch))
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
        self.source
            .rendered()
            .get(self.cursor..)?
            .chars()
            .next()
            .map(|ch| (self.cursor, ch))
    }

    fn peek_char(&self) -> Option<char> {
        self.peek().map(|(_, ch)| ch)
    }

    fn peek_next_char(&self) -> Option<char> {
        let current = self.peek_char()?;
        let next_cursor = self.cursor.checked_add(current.len_utf8())?;
        self.source.rendered().get(next_cursor..)?.chars().next()
    }

    fn remaining_source_starts_with(&self, prefix: &str) -> bool {
        self.source
            .rendered()
            .get(self.cursor..)
            .is_some_and(|source| source.starts_with(prefix))
    }

    const fn current_offset(&self) -> usize {
        self.cursor
    }
}
