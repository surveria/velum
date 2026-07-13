use crate::{
    error::{Error, Result},
    value::ErrorName,
};

const ARRAY_CLOSE: u16 = b']' as u16;
const ARRAY_OPEN: u16 = b'[' as u16;
const BACKSLASH: u16 = b'\\' as u16;
const COLON: u16 = b':' as u16;
const COMMA: u16 = b',' as u16;
const DECIMAL_POINT: u16 = b'.' as u16;
const ESCAPE_BACKSPACE: u16 = b'b' as u16;
const ESCAPE_FORM_FEED: u16 = b'f' as u16;
const ESCAPE_NEWLINE: u16 = b'n' as u16;
const ESCAPE_RETURN: u16 = b'r' as u16;
const ESCAPE_TAB: u16 = b't' as u16;
const ESCAPE_UNICODE: u16 = b'u' as u16;
const EXPONENT_LOWER: u16 = b'e' as u16;
const EXPONENT_UPPER: u16 = b'E' as u16;
const MINUS: u16 = b'-' as u16;
const OBJECT_CLOSE: u16 = b'}' as u16;
const OBJECT_OPEN: u16 = b'{' as u16;
const PLUS: u16 = b'+' as u16;
const QUOTE: u16 = b'"' as u16;

#[derive(Debug)]
pub(super) enum ParsedJson {
    Null,
    Bool(bool),
    Number(f64),
    String(Vec<u16>),
    Array(Vec<Self>),
    Object(Vec<(Vec<u16>, Self)>),
}

impl ParsedJson {
    pub(super) const fn is_scalar(&self) -> bool {
        matches!(
            self,
            Self::Null | Self::Bool(_) | Self::Number(_) | Self::String(_)
        )
    }
}

pub(super) fn parse_json_text(input: &[u16], max_depth: usize) -> Result<ParsedJson> {
    JsonParser::new(input, max_depth).parse()
}

struct JsonParser<'input> {
    input: &'input [u16],
    position: usize,
    max_depth: usize,
}

impl<'input> JsonParser<'input> {
    const fn new(input: &'input [u16], max_depth: usize) -> Self {
        Self {
            input,
            position: 0,
            max_depth,
        }
    }

    fn parse(mut self) -> Result<ParsedJson> {
        self.skip_whitespace();
        let value = self.parse_value(0)?;
        self.skip_whitespace();
        if self.position != self.input.len() {
            return Err(self.syntax_error("unexpected trailing JSON input"));
        }
        Ok(value)
    }

    fn parse_value(&mut self, depth: usize) -> Result<ParsedJson> {
        match self.current() {
            Some(OBJECT_OPEN) => self.parse_object(depth),
            Some(ARRAY_OPEN) => self.parse_array(depth),
            Some(QUOTE) => self.parse_string().map(ParsedJson::String),
            Some(value) if value == u16::from(b't') => {
                self.expect_keyword("true")?;
                Ok(ParsedJson::Bool(true))
            }
            Some(value) if value == u16::from(b'f') => {
                self.expect_keyword("false")?;
                Ok(ParsedJson::Bool(false))
            }
            Some(value) if value == u16::from(b'n') => {
                self.expect_keyword("null")?;
                Ok(ParsedJson::Null)
            }
            Some(value) if value == MINUS || is_ascii_digit(value) => self.parse_number(),
            Some(_) => Err(self.syntax_error("unexpected JSON token")),
            None => Err(self.syntax_error("expected a JSON value")),
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<ParsedJson> {
        let child_depth = self.enter_container(depth)?;
        self.advance();
        self.skip_whitespace();
        let mut members = Vec::new();
        if self.consume(OBJECT_CLOSE) {
            return Ok(ParsedJson::Object(members));
        }
        loop {
            if self.current() != Some(QUOTE) {
                return Err(self.syntax_error("expected a quoted JSON object key"));
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(COLON, "expected ':' after JSON object key")?;
            self.skip_whitespace();
            let value = self.parse_value(child_depth)?;
            members.push((key, value));
            self.skip_whitespace();
            if self.consume(OBJECT_CLOSE) {
                return Ok(ParsedJson::Object(members));
            }
            self.expect(COMMA, "expected ',' between JSON object members")?;
            self.skip_whitespace();
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<ParsedJson> {
        let child_depth = self.enter_container(depth)?;
        self.advance();
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume(ARRAY_CLOSE) {
            return Ok(ParsedJson::Array(values));
        }
        loop {
            values.push(self.parse_value(child_depth)?);
            self.skip_whitespace();
            if self.consume(ARRAY_CLOSE) {
                return Ok(ParsedJson::Array(values));
            }
            self.expect(COMMA, "expected ',' between JSON array elements")?;
            self.skip_whitespace();
        }
    }

    fn parse_string(&mut self) -> Result<Vec<u16>> {
        self.expect(QUOTE, "expected a JSON string")?;
        let mut output = Vec::new();
        loop {
            let Some(unit) = self.current() else {
                return Err(self.syntax_error("unterminated JSON string"));
            };
            self.advance();
            match unit {
                QUOTE => return Ok(output),
                BACKSLASH => output.push(self.parse_escape()?),
                0x0000..=0x001f => {
                    return Err(self.syntax_error("unescaped control character in JSON string"));
                }
                _ => output.push(unit),
            }
        }
    }

    fn parse_escape(&mut self) -> Result<u16> {
        let Some(escape) = self.current() else {
            return Err(self.syntax_error("unterminated JSON string escape"));
        };
        self.advance();
        match escape {
            QUOTE | BACKSLASH => Ok(escape),
            value if value == u16::from(b'/') => Ok(value),
            ESCAPE_BACKSPACE => Ok(0x0008),
            ESCAPE_FORM_FEED => Ok(0x000c),
            ESCAPE_NEWLINE => Ok(0x000a),
            ESCAPE_RETURN => Ok(0x000d),
            ESCAPE_TAB => Ok(0x0009),
            ESCAPE_UNICODE => self.parse_hex_escape(),
            _ => Err(self.syntax_error("invalid JSON string escape")),
        }
    }

    fn parse_hex_escape(&mut self) -> Result<u16> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let Some(unit) = self.current() else {
                return Err(self.syntax_error("incomplete JSON Unicode escape"));
            };
            let Some(digit) = hex_digit(unit) else {
                return Err(self.syntax_error("invalid JSON Unicode escape"));
            };
            value = value
                .checked_mul(16)
                .and_then(|current| current.checked_add(digit))
                .ok_or_else(|| self.syntax_error("JSON Unicode escape overflowed"))?;
            self.advance();
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<ParsedJson> {
        let start = self.position;
        self.consume(MINUS);
        match self.current() {
            Some(value) if value == u16::from(b'0') => {
                self.advance();
                if self.current().is_some_and(is_ascii_digit) {
                    return Err(self.syntax_error("leading zero in JSON number"));
                }
            }
            Some(value) if is_nonzero_ascii_digit(value) => self.consume_decimal_digits(),
            _ => return Err(self.syntax_error("invalid JSON number")),
        }
        if self.consume(DECIMAL_POINT) {
            self.require_decimal_digit("missing JSON fraction digits")?;
            self.consume_decimal_digits();
        }
        if matches!(self.current(), Some(EXPONENT_LOWER | EXPONENT_UPPER)) {
            self.advance();
            if matches!(self.current(), Some(PLUS | MINUS)) {
                self.advance();
            }
            self.require_decimal_digit("missing JSON exponent digits")?;
            self.consume_decimal_digits();
        }
        let text = self.input[start..self.position]
            .iter()
            .filter_map(|unit| u8::try_from(*unit).ok().map(char::from))
            .collect::<String>();
        let value = text
            .parse::<f64>()
            .map_err(|_| self.syntax_error("invalid JSON number"))?;
        Ok(ParsedJson::Number(value))
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<()> {
        for expected in keyword.encode_utf16() {
            self.expect(expected, "invalid JSON keyword")?;
        }
        Ok(())
    }

    fn enter_container(&self, depth: usize) -> Result<usize> {
        let next = depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("JSON nesting depth overflowed"))?;
        if next > self.max_depth {
            return Err(Error::limit(format!(
                "JSON nesting depth {next} exceeded {}",
                self.max_depth
            )));
        }
        Ok(next)
    }

    fn require_decimal_digit(&self, message: &str) -> Result<()> {
        if self.current().is_some_and(is_ascii_digit) {
            return Ok(());
        }
        Err(self.syntax_error(message))
    }

    fn consume_decimal_digits(&mut self) {
        while self.current().is_some_and(is_ascii_digit) {
            self.advance();
        }
    }

    fn skip_whitespace(&mut self) {
        while self.current().is_some_and(is_json_whitespace) {
            self.advance();
        }
    }

    fn expect(&mut self, expected: u16, message: &str) -> Result<()> {
        if self.consume(expected) {
            return Ok(());
        }
        Err(self.syntax_error(message))
    }

    fn consume(&mut self, expected: u16) -> bool {
        if self.current() != Some(expected) {
            return false;
        }
        self.advance();
        true
    }

    fn current(&self) -> Option<u16> {
        self.input.get(self.position).copied()
    }

    const fn advance(&mut self) {
        self.position = self.position.saturating_add(1);
    }

    fn syntax_error(&self, message: &str) -> Error {
        Error::exception(
            ErrorName::SyntaxError,
            format!("{message} at UTF-16 offset {}", self.position),
        )
    }
}

const fn is_json_whitespace(unit: u16) -> bool {
    matches!(unit, 0x0009 | 0x000a | 0x000d | 0x0020)
}

const fn is_ascii_digit(unit: u16) -> bool {
    unit >= b'0' as u16 && unit <= b'9' as u16
}

const fn is_nonzero_ascii_digit(unit: u16) -> bool {
    unit >= b'1' as u16 && unit <= b'9' as u16
}

const fn hex_digit(unit: u16) -> Option<u16> {
    match unit {
        value if value >= b'0' as u16 && value <= b'9' as u16 => Some(value - b'0' as u16),
        value if value >= b'a' as u16 && value <= b'f' as u16 => Some(value - b'a' as u16 + 10),
        value if value >= b'A' as u16 && value <= b'F' as u16 => Some(value - b'A' as u16 + 10),
        _ => None,
    }
}
