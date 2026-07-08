use std::{iter::Peekable, str::CharIndices};

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, numeric::number_to_i32},
    value::{ErrorName, Value},
};

const PARSE_INT_RADIX_CONTEXT: &str = "parseInt radix";
const URI_MALFORMED_ESCAPE_ERROR: &str = "malformed URI escape sequence";
const URI_MALFORMED_UTF8_ERROR: &str = "malformed URI UTF-8 sequence";

#[derive(Clone, Copy)]
enum ParseIntRadix {
    Infer,
    Explicit(u32),
    Invalid,
}

impl Context {
    pub(in crate::runtime::native) fn eval_global_parse_int(
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        Self::eval_direct_global_parse_int(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_parse_int(
        args: &[Value],
    ) -> Result<Value> {
        let input = Self::global_string_argument(args.first());
        let radix = Self::parse_int_radix(args.get(1))?;
        Ok(Value::Number(Self::parse_int_string(&input, radix)))
    }

    pub(in crate::runtime::native) fn eval_global_parse_float(args: RuntimeCallArgs<'_>) -> Value {
        Value::Number(Self::parse_float_string(&Self::global_string_argument(
            args.as_slice().first(),
        )))
    }

    pub(in crate::runtime::native) fn eval_direct_global_parse_float(args: &[Value]) -> Value {
        Value::Number(Self::parse_float_string(&Self::global_string_argument(
            args.first(),
        )))
    }

    pub(in crate::runtime::native) fn eval_global_is_nan(args: RuntimeCallArgs<'_>) -> Value {
        Self::eval_direct_global_is_nan(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_is_nan(args: &[Value]) -> Value {
        Value::Bool(
            args.first()
                .map_or(f64::NAN, Self::value_to_number)
                .is_nan(),
        )
    }

    pub(in crate::runtime::native) fn eval_global_is_finite(args: RuntimeCallArgs<'_>) -> Value {
        Self::eval_direct_global_is_finite(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_is_finite(args: &[Value]) -> Value {
        Value::Bool(
            args.first()
                .map_or(f64::NAN, Self::value_to_number)
                .is_finite(),
        )
    }

    pub(in crate::runtime::native) const fn eval_number_is_nan(args: RuntimeCallArgs<'_>) -> Value {
        Self::eval_direct_number_is_nan(args.as_slice())
    }

    pub(in crate::runtime::native) const fn eval_direct_number_is_nan(args: &[Value]) -> Value {
        match args.first() {
            Some(Value::Number(value)) => Value::Bool(value.is_nan()),
            Some(
                Value::Undefined
                | Value::Null
                | Value::Bool(_)
                | Value::String(_)
                | Value::HeapString(_)
                | Value::Symbol(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
                | Value::Object(_)
                | Value::Error(_),
            )
            | None => Value::Bool(false),
        }
    }

    pub(in crate::runtime::native) const fn eval_number_is_finite(
        args: RuntimeCallArgs<'_>,
    ) -> Value {
        Self::eval_direct_number_is_finite(args.as_slice())
    }

    pub(in crate::runtime::native) const fn eval_direct_number_is_finite(args: &[Value]) -> Value {
        match args.first() {
            Some(Value::Number(value)) => Value::Bool(value.is_finite()),
            Some(
                Value::Undefined
                | Value::Null
                | Value::Bool(_)
                | Value::String(_)
                | Value::HeapString(_)
                | Value::Symbol(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
                | Value::Object(_)
                | Value::Error(_),
            )
            | None => Value::Bool(false),
        }
    }

    pub(in crate::runtime::native) fn eval_global_encode_uri(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_global_encode_uri(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_encode_uri(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let text = Self::global_string_argument(args.first());
        self.encode_uri_value(&text, UriEncodeSet::Uri)
    }

    pub(in crate::runtime::native) fn eval_global_encode_uri_component(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_global_encode_uri_component(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_encode_uri_component(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let text = Self::global_string_argument(args.first());
        self.encode_uri_value(&text, UriEncodeSet::Component)
    }

    pub(in crate::runtime::native) fn eval_global_decode_uri(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_global_decode_uri(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_decode_uri(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let text = Self::global_string_argument(args.first());
        self.decode_uri_value(&text, UriDecodeMode::PreserveReserved)
    }

    pub(in crate::runtime::native) fn eval_global_decode_uri_component(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_global_decode_uri_component(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_decode_uri_component(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let text = Self::global_string_argument(args.first());
        self.decode_uri_value(&text, UriDecodeMode::DecodeReserved)
    }

    fn global_string_argument(value: Option<&Value>) -> String {
        value.map_or_else(|| Value::Undefined.to_string(), ToString::to_string)
    }

    fn parse_int_radix(value: Option<&Value>) -> Result<ParseIntRadix> {
        let Some(value) = value else {
            return Ok(ParseIntRadix::Infer);
        };
        let radix = number_to_i32(Self::value_to_number(value), PARSE_INT_RADIX_CONTEXT)?;
        if radix == 0 {
            return Ok(ParseIntRadix::Infer);
        }
        if !(2..=36).contains(&radix) {
            return Ok(ParseIntRadix::Invalid);
        }
        u32::try_from(radix)
            .map(ParseIntRadix::Explicit)
            .map_err(|_| Error::runtime("parseInt radix conversion failed"))
    }

    fn parse_int_string(input: &str, radix: ParseIntRadix) -> f64 {
        let trimmed = input.trim_start();
        let (sign, rest) = Self::strip_numeric_sign(trimmed);
        let (radix, digits) = match radix {
            ParseIntRadix::Infer => Self::parse_int_inferred_digits(rest),
            ParseIntRadix::Explicit(16) => (16, Self::strip_hex_prefix(rest)),
            ParseIntRadix::Explicit(radix) => (radix, rest),
            ParseIntRadix::Invalid => return f64::NAN,
        };

        let mut parsed = 0.0_f64;
        let mut consumed = false;
        for ch in digits.chars() {
            let Some(digit) = Self::ascii_digit_value(ch).filter(|digit| *digit < radix) else {
                break;
            };
            parsed = parsed.mul_add(f64::from(radix), f64::from(digit));
            consumed = true;
        }
        if consumed { parsed * sign } else { f64::NAN }
    }

    fn parse_float_string(input: &str) -> f64 {
        let trimmed = input.trim_start();
        let (sign, rest) = Self::strip_numeric_sign(trimmed);
        if rest.starts_with("Infinity") {
            return sign * f64::INFINITY;
        }

        let sign_width = trimmed.len().saturating_sub(rest.len());
        let mut end = sign_width;
        let mut chars = rest.char_indices().peekable();
        let digits_before_dot = Self::consume_parse_float_digits(&mut chars, sign_width, &mut end);
        let digits_after_dot = Self::consume_parse_float_dot(&mut chars, sign_width, &mut end);
        if !digits_before_dot && !digits_after_dot {
            return f64::NAN;
        }
        Self::consume_parse_float_exponent(&mut chars, sign_width, &mut end);
        let Some(prefix) = trimmed.get(..end) else {
            return f64::NAN;
        };
        prefix.parse::<f64>().map_or(f64::NAN, |value| value)
    }

    fn strip_numeric_sign(input: &str) -> (f64, &str) {
        if let Some(rest) = input.strip_prefix('-') {
            return (-1.0, rest);
        }
        if let Some(rest) = input.strip_prefix('+') {
            return (1.0, rest);
        }
        (1.0, input)
    }

    fn parse_int_inferred_digits(input: &str) -> (u32, &str) {
        if let Some(rest) = input
            .strip_prefix("0x")
            .or_else(|| input.strip_prefix("0X"))
        {
            return (16, rest);
        }
        (10, input)
    }

    fn strip_hex_prefix(input: &str) -> &str {
        input
            .strip_prefix("0x")
            .or_else(|| input.strip_prefix("0X"))
            .unwrap_or(input)
    }

    fn ascii_digit_value(ch: char) -> Option<u32> {
        match ch {
            '0'..='9' => u32::from(ch).checked_sub(u32::from('0')),
            'A'..='Z' => u32::from(ch)
                .checked_sub(u32::from('A'))
                .and_then(|value| value.checked_add(10)),
            'a'..='z' => u32::from(ch)
                .checked_sub(u32::from('a'))
                .and_then(|value| value.checked_add(10)),
            _ => None,
        }
    }

    fn consume_parse_float_digits(
        chars: &mut Peekable<CharIndices<'_>>,
        base_offset: usize,
        end: &mut usize,
    ) -> bool {
        let mut consumed = false;
        while let Some((offset, ch)) = chars.peek().copied() {
            if !ch.is_ascii_digit() {
                break;
            }
            *end = base_offset
                .saturating_add(offset)
                .saturating_add(ch.len_utf8());
            consumed = true;
            if chars.next().is_none() {
                break;
            }
        }
        consumed
    }

    fn consume_parse_float_dot(
        chars: &mut Peekable<CharIndices<'_>>,
        base_offset: usize,
        end: &mut usize,
    ) -> bool {
        let Some((offset, '.')) = chars.peek().copied() else {
            return false;
        };
        *end = base_offset.saturating_add(offset).saturating_add(1);
        if chars.next().is_none() {
            return false;
        }
        Self::consume_parse_float_digits(chars, base_offset, end)
    }

    fn consume_parse_float_exponent(
        chars: &mut Peekable<CharIndices<'_>>,
        base_offset: usize,
        end: &mut usize,
    ) {
        let Some((offset, 'e' | 'E')) = chars.peek().copied() else {
            return;
        };
        let mut candidate = chars.clone();
        let mut exponent_end = base_offset.saturating_add(offset).saturating_add(1);
        if candidate.next().is_none() {
            return;
        }
        if let Some((sign_offset, '+' | '-')) = candidate.peek().copied() {
            exponent_end = base_offset.saturating_add(sign_offset).saturating_add(1);
            if candidate.next().is_none() {
                return;
            }
        }
        if !Self::consume_parse_float_digits(&mut candidate, base_offset, &mut exponent_end) {
            return;
        }
        *chars = candidate;
        *end = exponent_end;
    }

    fn encode_uri_value(&mut self, input: &str, encode_set: UriEncodeSet) -> Result<Value> {
        let mut encoded = String::new();
        for ch in input.chars() {
            if encode_set.should_leave_unescaped(ch) {
                encoded.push(ch);
            } else {
                Self::push_percent_encoded_char(&mut encoded, ch);
            }
        }
        self.check_string_len(&encoded)?;
        self.heap_string_value(&encoded)
    }

    fn decode_uri_value(&mut self, input: &str, mode: UriDecodeMode) -> Result<Value> {
        let mut decoded = String::new();
        let mut index = 0;
        while index < input.len() {
            let Some(tail) = input.get(index..) else {
                return Err(Self::uri_error(URI_MALFORMED_ESCAPE_ERROR));
            };
            let Some(ch) = tail.chars().next() else {
                break;
            };
            if ch != '%' {
                decoded.push(ch);
                index = index
                    .checked_add(ch.len_utf8())
                    .ok_or_else(|| Error::runtime("URI decode index overflowed"))?;
                continue;
            }
            let sequence_start = index;
            let (text, next_index) = Self::decode_uri_escape(input, index, mode)?;
            if mode.should_preserve(&text) {
                let Some(original) = input.get(sequence_start..next_index) else {
                    return Err(Self::uri_error(URI_MALFORMED_ESCAPE_ERROR));
                };
                decoded.push_str(original);
            } else {
                decoded.push_str(&text);
            }
            index = next_index;
        }
        self.check_string_len(&decoded)?;
        self.heap_string_value(&decoded)
    }

    fn decode_uri_escape(
        input: &str,
        index: usize,
        mode: UriDecodeMode,
    ) -> Result<(String, usize)> {
        let first = Self::percent_byte(input, index)?;
        let width = Self::utf8_sequence_width(first)
            .ok_or_else(|| Self::uri_error(URI_MALFORMED_UTF8_ERROR))?;
        let mut bytes = Vec::with_capacity(width);
        bytes.push(first);
        let mut next_index = Self::next_percent_index(index)?;
        for _ in 1..width {
            let Some(byte) = input.as_bytes().get(next_index).copied() else {
                return Err(Self::uri_error(URI_MALFORMED_ESCAPE_ERROR));
            };
            if byte != b'%' {
                return Err(Self::uri_error(URI_MALFORMED_ESCAPE_ERROR));
            }
            bytes.push(Self::percent_byte(input, next_index)?);
            next_index = Self::next_percent_index(next_index)?;
        }
        let text =
            String::from_utf8(bytes).map_err(|_| Self::uri_error(URI_MALFORMED_UTF8_ERROR))?;
        if mode.should_preserve(&text) {
            return Ok((text, next_index));
        }
        Ok((text, next_index))
    }

    fn percent_byte(input: &str, index: usize) -> Result<u8> {
        let bytes = input.as_bytes();
        if bytes.get(index).copied() != Some(b'%') {
            return Err(Self::uri_error(URI_MALFORMED_ESCAPE_ERROR));
        }
        let high_index = index
            .checked_add(1)
            .ok_or_else(|| Error::runtime("URI escape index overflowed"))?;
        let low_index = index
            .checked_add(2)
            .ok_or_else(|| Error::runtime("URI escape index overflowed"))?;
        let high = bytes
            .get(high_index)
            .copied()
            .and_then(Self::hex_value)
            .ok_or_else(|| Self::uri_error(URI_MALFORMED_ESCAPE_ERROR))?;
        let low = bytes
            .get(low_index)
            .copied()
            .and_then(Self::hex_value)
            .ok_or_else(|| Self::uri_error(URI_MALFORMED_ESCAPE_ERROR))?;
        high.checked_mul(16)
            .and_then(|value| value.checked_add(low))
            .ok_or_else(|| Error::runtime("URI byte conversion overflowed"))
    }

    fn next_percent_index(index: usize) -> Result<usize> {
        index
            .checked_add(3)
            .ok_or_else(|| Error::runtime("URI escape index overflowed"))
    }

    fn hex_value(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => byte.checked_sub(b'0'),
            b'A'..=b'F' => byte
                .checked_sub(b'A')
                .and_then(|value| value.checked_add(10)),
            b'a'..=b'f' => byte
                .checked_sub(b'a')
                .and_then(|value| value.checked_add(10)),
            _ => None,
        }
    }

    const fn utf8_sequence_width(first: u8) -> Option<usize> {
        match first {
            0x00..=0x7f => Some(1),
            0xc2..=0xdf => Some(2),
            0xe0..=0xef => Some(3),
            0xf0..=0xf4 => Some(4),
            _ => None,
        }
    }

    fn push_percent_encoded_char(output: &mut String, ch: char) {
        let mut buffer = [0; 4];
        for byte in ch.encode_utf8(&mut buffer).as_bytes() {
            output.push('%');
            output.push(Self::hex_char(byte >> 4));
            output.push(Self::hex_char(byte & 0x0f));
        }
    }

    const fn hex_char(nibble: u8) -> char {
        match nibble {
            0 => '0',
            1 => '1',
            2 => '2',
            3 => '3',
            4 => '4',
            5 => '5',
            6 => '6',
            7 => '7',
            8 => '8',
            9 => '9',
            10 => 'A',
            11 => 'B',
            12 => 'C',
            13 => 'D',
            14 => 'E',
            15 => 'F',
            _ => '?',
        }
    }

    const fn is_uri_unescaped(ch: char) -> bool {
        matches!(
            ch,
            'A'..='Z'
                | 'a'..='z'
                | '0'..='9'
                | '-'
                | '_'
                | '.'
                | '!'
                | '~'
                | '*'
                | '\''
                | '('
                | ')'
        )
    }

    const fn is_uri_reserved(ch: char) -> bool {
        matches!(
            ch,
            ';' | '/' | '?' | ':' | '@' | '&' | '=' | '+' | '$' | ',' | '#'
        )
    }

    fn uri_error(message: &'static str) -> Error {
        Error::exception(ErrorName::UriError, message)
    }
}

#[derive(Clone, Copy)]
enum UriEncodeSet {
    Uri,
    Component,
}

impl UriEncodeSet {
    const fn should_leave_unescaped(self, ch: char) -> bool {
        Context::is_uri_unescaped(ch) || matches!(self, Self::Uri) && Context::is_uri_reserved(ch)
    }
}

#[derive(Clone, Copy)]
enum UriDecodeMode {
    PreserveReserved,
    DecodeReserved,
}

impl UriDecodeMode {
    fn should_preserve(self, text: &str) -> bool {
        if !matches!(self, Self::PreserveReserved) {
            return false;
        }
        let mut chars = text.chars();
        let Some(ch) = chars.next() else {
            return false;
        };
        chars.next().is_none() && Context::is_uri_reserved(ch)
    }
}
