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
pub const ZERO_WIDTH_NON_JOINER: char = '\u{200c}';
pub const ZERO_WIDTH_JOINER: char = '\u{200d}';

pub fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch == '$' || unicode_ident::is_xid_start(ch) || is_other_identifier_start(ch)
}

pub fn is_identifier_part(ch: char) -> bool {
    is_identifier_start(ch)
        || unicode_ident::is_xid_continue(ch)
        || is_other_identifier_continue(ch)
        || matches!(ch, ZERO_WIDTH_NON_JOINER | ZERO_WIDTH_JOINER)
}

const fn is_other_identifier_start(ch: char) -> bool {
    matches!(
        ch,
        '\u{1885}' | '\u{1886}' | '\u{2118}' | '\u{212e}' | '\u{309b}' | '\u{309c}'
    )
}

const fn is_other_identifier_continue(ch: char) -> bool {
    matches!(
        ch,
        '\u{00b7}' | '\u{0387}' | '\u{19da}' | '\u{1369}'..='\u{1371}'
    )
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

pub const fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | LINE_SEPARATOR | PARAGRAPH_SEPARATOR)
}

pub const fn numeric_prefix(ch: Option<char>) -> Option<(u32, &'static str)> {
    match ch {
        Some('b' | 'B') => Some((RADIX_BINARY, "binary numeric literal")),
        Some('o' | 'O') => Some((RADIX_OCTAL, "octal numeric literal")),
        Some('x' | 'X') => Some((RADIX_HEX, "hexadecimal numeric literal")),
        _ => None,
    }
}

pub fn unicode_char(value: u32, offset: usize, description: &str) -> Result<char> {
    char::from_u32(value).ok_or_else(|| Error::lex(format!("{description} is invalid"), offset))
}

pub fn push_utf16_char(output: &mut Vec<u16>, ch: char) {
    let mut buffer = [0_u16; 2];
    output.extend_from_slice(ch.encode_utf16(&mut buffer));
}

pub fn append_utf16_value(output: &mut Vec<u16>, value: u32, offset: usize) -> Result<()> {
    if let Ok(unit) = u16::try_from(value) {
        output.push(unit);
        return Ok(());
    }
    let supplementary = value
        .checked_sub(0x1_0000)
        .ok_or_else(|| Error::lex("unicode escape is invalid", offset))?;
    let high = u16::try_from(0xD800 + (supplementary >> 10))
        .map_err(|_| Error::lex("unicode escape is invalid", offset))?;
    let low = u16::try_from(0xDC00 + (supplementary & 0x3FF))
        .map_err(|_| Error::lex("unicode escape is invalid", offset))?;
    output.push(high);
    output.push(low);
    Ok(())
}
