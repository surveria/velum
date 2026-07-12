use super::{
    Token, TokenKind,
    template::{TemplatePartPosition, TemplateSubstitutionState},
};
use crate::{
    error::{Error, Result},
    lexer::classification::{EscapeContext, token_kind_can_precede_regexp},
    lexer::support::{
        ASCII_BACKSPACE, ASCII_FORM_FEED, ASCII_VERTICAL_TAB, BIGINT_SUFFIX, DECIMAL_POINT,
        HEX_ESCAPE_DIGITS, LINE_SEPARATOR, MAX_BRACED_UNICODE_ESCAPE_DIGITS,
        MAX_UNICODE_CODE_POINT, NUMERIC_SEPARATOR, PARAGRAPH_SEPARATOR, RADIX_DECIMAL,
        TEMPLATE_SUBSTITUTION_START, UNICODE_ESCAPE_DIGITS, append_utf16_value,
        checked_hex_accumulate, digit_value, digits_to_number, is_exponent_marker,
        is_identifier_part, is_identifier_start, is_line_terminator, numeric_prefix,
        push_utf16_char, unicode_char,
    },
    regexp_syntax::validate_regexp_literal,
    source::{SourceId, SourceSpan},
    value::JsBigInt,
};

mod names;
mod operators;

pub fn lex(source: &str, source_id: SourceId) -> Result<Vec<Token>> {
    Lexer::new(source, source_id).lex()
}

struct Lexer<'a> {
    source: &'a str,
    source_id: SourceId,
    chars: Vec<(usize, char)>,
    cursor: usize,
    tokens: Vec<Token>,
    line_terminator_before: bool,
    template_substitutions: Vec<TemplateSubstitutionState>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str, source_id: SourceId) -> Self {
        Self {
            source,
            source_id,
            chars: source.char_indices().collect(),
            cursor: 0,
            tokens: Vec::new(),
            line_terminator_before: false,
            template_substitutions: Vec::new(),
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
                '#' if self.cursor == 0 && self.peek_next_char() == Some('!') => {
                    self.hashbang_comment();
                }
                '#' => self.private_name(offset)?,
                '/' if self.peek_next_char() == Some('/') => self.line_comment(),
                '/' if self.peek_next_char() == Some('*') => self.block_comment(offset)?,
                '0'..='9' => self.number(offset)?,
                '"' | '\'' => self.string(offset, ch)?,
                '`' => self.template_literal(offset)?,
                ch if is_identifier_start(ch) || self.identifier_escape_start() => {
                    self.identifier(offset)?;
                }
                '+' => self.plus_or_increment(offset),
                '-' => self.minus_or_decrement(offset),
                '*' => self.star_or_power(offset),
                '/' => self.slash_or_regexp(offset)?,
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
                _ => return Err(Error::lex(format!("unexpected character '{ch}'"), offset)),
            }
        }

        if let Some(substitution) = self.template_substitutions.last() {
            return Err(Error::lex(
                "unterminated template literal substitution",
                substitution.substitution_offset,
            ));
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: SourceSpan::point(self.source_id, self.source.len()),
            line_terminator_before: self.line_terminator_before,
            identifier_escaped: false,
        });
        Ok(self.tokens)
    }

    fn number(&mut self, offset: usize) -> Result<()> {
        if self.peek_char() == Some('0')
            && let Some((radix, description)) = numeric_prefix(self.peek_next_char())
        {
            return self.prefixed_number(offset, radix, description);
        }
        self.decimal_number(offset)
    }

    fn slash_or_regexp(&mut self, offset: usize) -> Result<()> {
        if self.peek_next_char() == Some('=') || !self.can_start_regexp_literal() {
            self.simple_or_equal(offset, TokenKind::Slash, TokenKind::SlashEqual);
            return Ok(());
        }
        self.regexp_literal(offset)
    }

    fn can_start_regexp_literal(&self) -> bool {
        self.tokens
            .last()
            .is_none_or(|token| token_kind_can_precede_regexp(&token.kind))
    }

    fn regexp_literal(&mut self, offset: usize) -> Result<()> {
        self.advance();
        let mut pattern = String::new();
        let mut in_class = false;
        let mut escaped = false;
        while let Some((_, ch)) = self.peek() {
            if is_line_terminator(ch) {
                return Err(Error::lex(
                    "unterminated regular expression literal",
                    offset,
                ));
            }
            self.advance();
            if escaped {
                pattern.push(ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => {
                    pattern.push(ch);
                    escaped = true;
                }
                '[' => {
                    pattern.push(ch);
                    in_class = true;
                }
                ']' => {
                    pattern.push(ch);
                    in_class = false;
                }
                '/' if !in_class => {
                    let flags = self.regexp_flags();
                    validate_regexp_literal(&pattern, &flags)
                        .map_err(|error| Error::lex(error.to_string(), offset))?;
                    self.push(TokenKind::RegExp { pattern, flags }, offset);
                    return Ok(());
                }
                _ => pattern.push(ch),
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
        self.push(TokenKind::Number(value), offset);
        Ok(())
    }

    fn decimal_number(&mut self, offset: usize) -> Result<()> {
        let mut text = self.digit_sequence(RADIX_DECIMAL, offset, "decimal numeric literal")?;

        if self.consume_bigint_suffix() {
            if text.len() > 1 && text.starts_with('0') {
                return Err(Error::lex(
                    "decimal BigInt literal cannot have a leading zero",
                    offset,
                ));
            }
            let value = JsBigInt::parse_digits(&text, RADIX_DECIMAL)
                .ok_or_else(|| Error::lex("invalid decimal BigInt literal", offset))?;
            self.push(TokenKind::BigInt(value), offset);
            return Ok(());
        }

        if self.peek_char() == Some(DECIMAL_POINT) {
            self.advance();
            text.push(DECIMAL_POINT);
            if matches!(self.peek_char(), Some('0'..='9')) {
                let fraction =
                    self.digit_sequence(RADIX_DECIMAL, offset, "decimal fraction literal")?;
                text.push_str(&fraction);
            }
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

        while let Some((current_offset, ch)) = self.peek() {
            self.advance();
            match ch {
                ch if ch == quote => {
                    self.push(TokenKind::String(output), offset);
                    return Ok(());
                }
                '\\' => self.string_escape(current_offset, &mut output)?,
                '\n' | '\r' => return Err(Error::lex("unterminated string literal", offset)),
                other => push_utf16_char(&mut output, other),
            }
        }

        Err(Error::lex("unterminated string literal", offset))
    }

    fn string_escape(&mut self, slash_offset: usize, output: &mut Vec<u16>) -> Result<()> {
        self.escape_sequence(slash_offset, output, EscapeContext::String)
    }

    fn template_literal(&mut self, offset: usize) -> Result<()> {
        self.advance();
        self.template_part(offset, TemplatePartPosition::Head)
    }

    fn template_part(&mut self, offset: usize, position: TemplatePartPosition) -> Result<()> {
        let mut output = Vec::new();

        while let Some((current_offset, ch)) = self.peek() {
            self.advance();
            match ch {
                '`' => return self.end_template_part(position, output, offset),
                '$' if self.peek_char() == Some(TEMPLATE_SUBSTITUTION_START) => {
                    self.advance();
                    return self.begin_template_substitution(
                        position,
                        output,
                        offset,
                        current_offset,
                    );
                }
                '\\' => self.template_escape(current_offset, &mut output)?,
                '\n' => push_utf16_char(&mut output, '\n'),
                '\r' => {
                    if self.peek_char() == Some('\n') {
                        self.advance();
                    }
                    push_utf16_char(&mut output, '\n');
                }
                LINE_SEPARATOR | PARAGRAPH_SEPARATOR => push_utf16_char(&mut output, ch),
                other => push_utf16_char(&mut output, other),
            }
        }

        Err(Error::lex("unterminated template literal", offset))
    }

    fn end_template_part(
        &mut self,
        position: TemplatePartPosition,
        output: Vec<u16>,
        offset: usize,
    ) -> Result<()> {
        match position {
            TemplatePartPosition::Head => {
                // A template without substitutions stays a plain string token.
                self.push(TokenKind::String(output), offset);
            }
            TemplatePartPosition::Continuation => {
                if self.template_substitutions.pop().is_none() {
                    return Err(Error::lex(
                        "template substitution state underflowed",
                        offset,
                    ));
                }
                self.push(TokenKind::TemplateTail(output), offset);
            }
        }
        Ok(())
    }

    fn begin_template_substitution(
        &mut self,
        position: TemplatePartPosition,
        output: Vec<u16>,
        offset: usize,
        substitution_offset: usize,
    ) -> Result<()> {
        match position {
            TemplatePartPosition::Head => {
                self.push(TokenKind::TemplateHead(output), offset);
                self.template_substitutions.push(TemplateSubstitutionState {
                    open_braces: 0,
                    substitution_offset,
                });
            }
            TemplatePartPosition::Continuation => {
                let Some(substitution) = self.template_substitutions.last_mut() else {
                    return Err(Error::lex(
                        "template substitution state underflowed",
                        offset,
                    ));
                };
                substitution.open_braces = 0;
                substitution.substitution_offset = substitution_offset;
                self.push(TokenKind::TemplateMiddle(output), offset);
            }
        }
        Ok(())
    }

    fn substitution_brace_open(&mut self, offset: usize) -> Result<()> {
        if let Some(substitution) = self.template_substitutions.last_mut() {
            substitution.open_braces =
                substitution.open_braces.checked_add(1).ok_or_else(|| {
                    Error::lex("template substitution brace depth overflowed", offset)
                })?;
        }
        Ok(())
    }

    fn right_brace_or_template_continuation(&mut self, offset: usize) -> Result<()> {
        match self.template_substitutions.last_mut() {
            Some(substitution) if substitution.open_braces == 0 => {
                self.advance();
                self.template_part(offset, TemplatePartPosition::Continuation)
            }
            Some(substitution) => {
                substitution.open_braces = substitution.open_braces.saturating_sub(1);
                self.simple(TokenKind::RBrace);
                Ok(())
            }
            None => {
                self.simple(TokenKind::RBrace);
                Ok(())
            }
        }
    }

    fn template_escape(&mut self, slash_offset: usize, output: &mut Vec<u16>) -> Result<()> {
        self.escape_sequence(slash_offset, output, EscapeContext::Template)
    }

    fn escape_sequence(
        &mut self,
        slash_offset: usize,
        output: &mut Vec<u16>,
        context: EscapeContext,
    ) -> Result<()> {
        let Some((escape_offset, escaped)) = self.peek() else {
            return Err(Error::lex("unterminated escape sequence", slash_offset));
        };
        self.advance();
        match escaped {
            'b' => push_utf16_char(output, ASCII_BACKSPACE),
            'f' => push_utf16_char(output, ASCII_FORM_FEED),
            'n' => push_utf16_char(output, '\n'),
            'r' => push_utf16_char(output, '\r'),
            't' => push_utf16_char(output, '\t'),
            'v' => push_utf16_char(output, ASCII_VERTICAL_TAB),
            '0' => self.zero_escape(escape_offset, output)?,
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
            '1'..='9' => {
                return Err(Error::lex(
                    "legacy octal escape sequences are not supported",
                    escape_offset,
                ));
            }
            other => push_utf16_char(output, other),
        }
        Ok(())
    }

    fn zero_escape(&self, escape_offset: usize, output: &mut Vec<u16>) -> Result<()> {
        if self.peek_char().is_some_and(|ch| ch.is_ascii_digit()) {
            return Err(Error::lex(
                "legacy octal escape sequences are not supported",
                escape_offset,
            ));
        }
        push_utf16_char(output, '\0');
        Ok(())
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
        self.tokens.push(Token {
            kind,
            span,
            line_terminator_before: self.line_terminator_before,
            identifier_escaped,
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
}
