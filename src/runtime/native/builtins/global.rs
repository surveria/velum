#[cfg(not(feature = "std"))]
use crate::prelude::*;

use core::{iter::Peekable, str::CharIndices};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, abstract_operations::is_ecmascript_string_whitespace, call::RuntimeCallArgs,
        native::AnnexBGlobalFunctionKind, numeric::number_to_i32,
    },
    value::{ErrorName, Value},
};

const PARSE_INT_RADIX_CONTEXT: &str = "parseInt radix";
const URI_MALFORMED_ESCAPE_ERROR: &str = "malformed URI escape sequence";
const URI_MALFORMED_SURROGATE_ERROR: &str = "malformed URI surrogate sequence";
const URI_MALFORMED_UTF8_ERROR: &str = "malformed URI UTF-8 sequence";

#[derive(Clone, Copy)]
enum ParseIntRadix {
    Infer,
    Explicit(u32),
    Invalid,
}

impl Context {
    pub(in crate::runtime::native) fn eval_annex_b_global(
        &mut self,
        kind: AnnexBGlobalFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let input = self.global_string_argument(args.as_slice().first())?;
        let output = match kind {
            AnnexBGlobalFunctionKind::Escape => Self::escape_legacy_string(&input)?,
            AnnexBGlobalFunctionKind::Unescape => Self::unescape_legacy_string(&input),
        };
        self.check_string_len(&output)?;
        self.heap_string_value(&output)
    }

    fn escape_legacy_string(input: &str) -> Result<String> {
        let mut output = String::new();
        for unit in input.encode_utf16() {
            if Self::is_legacy_escape_unmodified(unit) {
                let ch = char::from_u32(u32::from(unit))
                    .ok_or_else(|| Error::runtime("legacy escape character is invalid"))?;
                output.push(ch);
            } else if unit < 256 {
                output.push('%');
                output.push(Self::hex_unit_char((unit >> 4) & 0x0f));
                output.push(Self::hex_unit_char(unit & 0x0f));
            } else {
                output.push_str("%u");
                output.push(Self::hex_unit_char((unit >> 12) & 0x0f));
                output.push(Self::hex_unit_char((unit >> 8) & 0x0f));
                output.push(Self::hex_unit_char((unit >> 4) & 0x0f));
                output.push(Self::hex_unit_char(unit & 0x0f));
            }
        }
        Ok(output)
    }

    fn unescape_legacy_string(input: &str) -> String {
        let units = input.encode_utf16().collect::<Vec<_>>();
        let mut output = Vec::with_capacity(units.len());
        let mut index = 0_usize;
        while let Some(unit) = units.get(index).copied() {
            if unit == u16::from(b'%')
                && let Some((decoded, consumed)) = Self::legacy_escape_sequence(&units, index)
            {
                output.push(decoded);
                index = index.saturating_add(consumed);
                continue;
            }
            output.push(unit);
            index = index.saturating_add(1);
        }
        String::from_utf16_lossy(&output)
    }

    fn legacy_escape_sequence(units: &[u16], index: usize) -> Option<(u16, usize)> {
        let marker = units.get(index.checked_add(1)?)?;
        if *marker == u16::from(b'u') {
            let a = Self::legacy_hex_value(*units.get(index.checked_add(2)?)?)?;
            let b = Self::legacy_hex_value(*units.get(index.checked_add(3)?)?)?;
            let c = Self::legacy_hex_value(*units.get(index.checked_add(4)?)?)?;
            let d = Self::legacy_hex_value(*units.get(index.checked_add(5)?)?)?;
            let value = a
                .checked_mul(16)?
                .checked_add(b)?
                .checked_mul(16)?
                .checked_add(c)?
                .checked_mul(16)?
                .checked_add(d)?;
            return Some((value, 6));
        }
        let high = Self::legacy_hex_value(*marker)?;
        let low = Self::legacy_hex_value(*units.get(index.checked_add(2)?)?)?;
        high.checked_mul(16)
            .and_then(|value| value.checked_add(low))
            .map(|value| (value, 3))
    }

    fn legacy_hex_value(unit: u16) -> Option<u16> {
        match unit {
            0x30..=0x39 => unit.checked_sub(0x30),
            0x41..=0x46 => unit
                .checked_sub(0x41)
                .and_then(|value| value.checked_add(10)),
            0x61..=0x66 => unit
                .checked_sub(0x61)
                .and_then(|value| value.checked_add(10)),
            _ => None,
        }
    }

    const fn is_legacy_escape_unmodified(unit: u16) -> bool {
        matches!(
            unit,
            0x30..=0x39
                | 0x41..=0x5a
                | 0x61..=0x7a
                | 0x40
                | 0x2a
                | 0x5f
                | 0x2b
                | 0x2d
                | 0x2e
                | 0x2f
        )
    }

    const fn hex_unit_char(nibble: u16) -> char {
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

    pub(in crate::runtime::native) fn eval_global_parse_int(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_global_parse_int(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_parse_int(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let input = self.global_string_argument(args.first())?;
        let radix = self.parse_int_radix(args.get(1))?;
        Ok(Value::Number(Self::parse_int_string(&input, radix)))
    }

    pub(in crate::runtime::native) fn eval_global_parse_float(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_global_parse_float(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_parse_float(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let input = self.global_string_argument(args.first())?;
        Ok(Value::Number(Self::parse_float_string(&input)))
    }

    pub(in crate::runtime::native) fn eval_global_is_nan(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_global_is_nan(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_is_nan(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let number = match args.first() {
            Some(value) => self.to_number(value)?,
            None => f64::NAN,
        };
        Ok(Value::Bool(number.is_nan()))
    }

    pub(in crate::runtime::native) fn eval_global_is_finite(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_global_is_finite(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_global_is_finite(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let number = match args.first() {
            Some(value) => self.to_number(value)?,
            None => f64::NAN,
        };
        Ok(Value::Bool(number.is_finite()))
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
                | Value::BigInt(_)
                | Value::String(_)
                | Value::Symbol(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
                | Value::Object(_),
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
                | Value::BigInt(_)
                | Value::String(_)
                | Value::Symbol(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
                | Value::Object(_),
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
        let text = self.global_utf16_string_argument(args.first())?;
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
        let text = self.global_utf16_string_argument(args.first())?;
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
        let text = self.global_utf16_string_argument(args.first())?;
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
        let text = self.global_utf16_string_argument(args.first())?;
        self.decode_uri_value(&text, UriDecodeMode::DecodeReserved)
    }

    fn global_string_argument(&mut self, value: Option<&Value>) -> Result<String> {
        match value {
            Some(value) => self.to_string(value),
            None => self.to_string(&Value::Undefined),
        }
    }

    fn global_utf16_string_argument(&mut self, value: Option<&Value>) -> Result<Vec<u16>> {
        match value {
            Some(value) => self.to_utf16_string(value),
            None => self.to_utf16_string(&Value::Undefined),
        }
    }

    fn parse_int_radix(&mut self, value: Option<&Value>) -> Result<ParseIntRadix> {
        let Some(value) = value else {
            return Ok(ParseIntRadix::Infer);
        };
        let radix = number_to_i32(self.to_number(value)?, PARSE_INT_RADIX_CONTEXT)?;
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
        let trimmed = input.trim_start_matches(is_ecmascript_string_whitespace);
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
        let trimmed = input.trim_start_matches(is_ecmascript_string_whitespace);
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

    fn encode_uri_value(&mut self, input: &[u16], encode_set: UriEncodeSet) -> Result<Value> {
        let mut encoded = String::new();
        let mut index = 0_usize;
        while let Some(unit) = input.get(index).copied() {
            let ch = match unit {
                0xD800..=0xDBFF => {
                    let low_index = index
                        .checked_add(1)
                        .ok_or_else(|| Error::limit("URI encode index overflowed"))?;
                    let Some(low) = input.get(low_index).copied() else {
                        return Err(Self::uri_error(URI_MALFORMED_SURROGATE_ERROR));
                    };
                    if !(0xDC00..=0xDFFF).contains(&low) {
                        return Err(Self::uri_error(URI_MALFORMED_SURROGATE_ERROR));
                    }
                    let high_ten = u32::from(unit)
                        .checked_sub(0xD800)
                        .and_then(|value| value.checked_mul(0x400))
                        .ok_or_else(|| Error::runtime("URI surrogate value overflowed"))?;
                    let low_ten = u32::from(low)
                        .checked_sub(0xDC00)
                        .ok_or_else(|| Error::runtime("URI surrogate value underflowed"))?;
                    let code_point = 0x1_0000_u32
                        .checked_add(high_ten)
                        .and_then(|value| value.checked_add(low_ten))
                        .ok_or_else(|| Error::runtime("URI code point overflowed"))?;
                    index = low_index;
                    char::from_u32(code_point)
                        .ok_or_else(|| Self::uri_error(URI_MALFORMED_SURROGATE_ERROR))?
                }
                0xDC00..=0xDFFF => {
                    return Err(Self::uri_error(URI_MALFORMED_SURROGATE_ERROR));
                }
                _ => char::from_u32(u32::from(unit))
                    .ok_or_else(|| Self::uri_error(URI_MALFORMED_SURROGATE_ERROR))?,
            };
            if encode_set.should_leave_unescaped(ch) {
                encoded.push(ch);
            } else {
                Self::push_percent_encoded_char(&mut encoded, ch);
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::limit("URI encode index overflowed"))?;
        }
        self.check_string_len(&encoded)?;
        self.heap_string_value(&encoded)
    }

    fn decode_uri_value(&mut self, input: &[u16], mode: UriDecodeMode) -> Result<Value> {
        let mut decoded = Vec::new();
        let mut index = 0;
        while let Some(unit) = input.get(index).copied() {
            if unit != u16::from(b'%') {
                decoded.push(unit);
                index = index
                    .checked_add(1)
                    .ok_or_else(|| Error::runtime("URI decode index overflowed"))?;
                continue;
            }
            let sequence_start = index;
            let (text, next_index) = Self::decode_uri_escape(input, index, mode)?;
            if mode.should_preserve(&text) {
                let Some(original) = input.get(sequence_start..next_index) else {
                    return Err(Self::uri_error(URI_MALFORMED_ESCAPE_ERROR));
                };
                decoded.extend_from_slice(original);
            } else {
                decoded.extend(text.encode_utf16());
            }
            index = next_index;
        }
        self.heap_utf16_string_value(&decoded)
    }

    fn decode_uri_escape(
        input: &[u16],
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
            let Some(unit) = input.get(next_index).copied() else {
                return Err(Self::uri_error(URI_MALFORMED_ESCAPE_ERROR));
            };
            if unit != u16::from(b'%') {
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

    fn percent_byte(input: &[u16], index: usize) -> Result<u8> {
        if input.get(index).copied() != Some(u16::from(b'%')) {
            return Err(Self::uri_error(URI_MALFORMED_ESCAPE_ERROR));
        }
        let high_index = index
            .checked_add(1)
            .ok_or_else(|| Error::runtime("URI escape index overflowed"))?;
        let low_index = index
            .checked_add(2)
            .ok_or_else(|| Error::runtime("URI escape index overflowed"))?;
        let high = input
            .get(high_index)
            .copied()
            .and_then(|unit| u8::try_from(unit).ok())
            .and_then(Self::hex_value)
            .ok_or_else(|| Self::uri_error(URI_MALFORMED_ESCAPE_ERROR))?;
        let low = input
            .get(low_index)
            .copied()
            .and_then(|unit| u8::try_from(unit).ok())
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
