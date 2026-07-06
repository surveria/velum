use crate::error::{Error, Result};

pub const HEX_ESCAPE_DIGITS: usize = 2;
pub const UNICODE_ESCAPE_DIGITS: usize = 4;
pub const MAX_BRACED_UNICODE_ESCAPE_DIGITS: usize = 6;
pub const MAX_UNICODE_CODE_POINT: u32 = 0x10_FFFF;
pub const RADIX_BINARY: u32 = 2;
pub const RADIX_OCTAL: u32 = 8;
pub const RADIX_DECIMAL: u32 = 10;
pub const RADIX_HEX: u32 = 16;
pub const ASCII_BACKSPACE: char = '\u{0008}';
pub const ASCII_FORM_FEED: char = '\u{000c}';
pub const ASCII_VERTICAL_TAB: char = '\u{000b}';
pub const LINE_SEPARATOR: char = '\u{2028}';
pub const PARAGRAPH_SEPARATOR: char = '\u{2029}';
pub const DECIMAL_POINT: char = '.';
pub const NUMERIC_SEPARATOR: char = '_';
pub const BIGINT_SUFFIX: char = 'n';
pub const TEMPLATE_SUBSTITUTION_START: char = '{';

pub const fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphabetic()
}

pub const fn is_identifier_part(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

pub fn checked_hex_accumulate(
    value: u32,
    digit: u32,
    offset: usize,
    description: &str,
) -> Result<u32> {
    value
        .checked_mul(16)
        .and_then(|value| value.checked_add(digit))
        .ok_or_else(|| Error::lex(format!("{description} value overflow"), offset))
}

pub fn digits_to_number(digits: &str, radix: u32, offset: usize, description: &str) -> Result<f64> {
    let mut value = 0.0f64;
    let radix_value = f64::from(radix);
    for ch in digits.chars() {
        let Some(digit) = digit_value(ch, radix) else {
            return Err(Error::lex(
                format!("{description} has invalid digit '{ch}'"),
                offset,
            ));
        };
        value = value.mul_add(radix_value, f64::from(digit));
    }
    Ok(value)
}

pub const fn digit_value(ch: char, radix: u32) -> Option<u32> {
    ch.to_digit(radix)
}

pub const fn is_exponent_marker(ch: char) -> bool {
    matches!(ch, 'e' | 'E')
}

pub fn unicode_char(value: u32, offset: usize, description: &str) -> Result<char> {
    char::from_u32(value).ok_or_else(|| Error::lex(format!("{description} is invalid"), offset))
}
