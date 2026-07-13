use crate::error::{Error, Result};

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const HIGH_SURROGATE_END: u16 = 0xdbff;
const HIGH_SURROGATE_START: u16 = 0xd800;
const LOW_SURROGATE_END: u16 = 0xdfff;
const LOW_SURROGATE_START: u16 = 0xdc00;

pub(super) fn quote_json_string(units: &[u16], max_len: usize) -> Result<String> {
    let mut output = String::new();
    push_fragment(&mut output, "\"", max_len)?;
    let mut position = 0_usize;
    while let Some(unit) = units.get(position).copied() {
        match unit {
            0x0008 => push_fragment(&mut output, "\\b", max_len)?,
            0x0009 => push_fragment(&mut output, "\\t", max_len)?,
            0x000a => push_fragment(&mut output, "\\n", max_len)?,
            0x000c => push_fragment(&mut output, "\\f", max_len)?,
            0x000d => push_fragment(&mut output, "\\r", max_len)?,
            0x0000..=0x001f => push_unicode_escape(&mut output, unit, max_len)?,
            value if value == u16::from(b'"') => push_fragment(&mut output, "\\\"", max_len)?,
            value if value == u16::from(b'\\') => push_fragment(&mut output, "\\\\", max_len)?,
            HIGH_SURROGATE_START..=HIGH_SURROGATE_END => {
                let Some(low) = units.get(position.saturating_add(1)).copied() else {
                    push_unicode_escape(&mut output, unit, max_len)?;
                    position = position.saturating_add(1);
                    continue;
                };
                if (LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(&low) {
                    let scalar = surrogate_pair_scalar(unit, low)?;
                    push_char(&mut output, scalar, max_len)?;
                    position = position.saturating_add(1);
                } else {
                    push_unicode_escape(&mut output, unit, max_len)?;
                }
            }
            LOW_SURROGATE_START..=LOW_SURROGATE_END => {
                push_unicode_escape(&mut output, unit, max_len)?;
            }
            _ => {
                let scalar = char::from_u32(u32::from(unit))
                    .ok_or_else(|| Error::runtime("JSON string code unit was not a scalar"))?;
                push_char(&mut output, scalar, max_len)?;
            }
        }
        position = position.saturating_add(1);
    }
    push_fragment(&mut output, "\"", max_len)?;
    Ok(output)
}

fn surrogate_pair_scalar(high: u16, low: u16) -> Result<char> {
    let high_value = u32::from(high - HIGH_SURROGATE_START);
    let low_value = u32::from(low - LOW_SURROGATE_START);
    let scalar = 0x1_0000_u32
        .checked_add(high_value << 10)
        .and_then(|value| value.checked_add(low_value))
        .ok_or_else(|| Error::runtime("JSON surrogate pair overflowed"))?;
    char::from_u32(scalar).ok_or_else(|| Error::runtime("invalid JSON surrogate pair"))
}

fn push_unicode_escape(output: &mut String, unit: u16, max_len: usize) -> Result<()> {
    push_fragment(output, "\\u", max_len)?;
    for shift in [12_u32, 8, 4, 0] {
        let digit = usize::from((unit >> shift) & 0x000f);
        let byte = HEX_DIGITS
            .get(digit)
            .copied()
            .ok_or_else(|| Error::runtime("JSON escape digit was out of range"))?;
        push_char(output, char::from(byte), max_len)?;
    }
    Ok(())
}

fn push_char(output: &mut String, value: char, max_len: usize) -> Result<()> {
    output.push(value);
    check_len(output, max_len)
}

fn push_fragment(output: &mut String, value: &str, max_len: usize) -> Result<()> {
    output.push_str(value);
    check_len(output, max_len)
}

fn check_len(output: &str, max_len: usize) -> Result<()> {
    if output.len() <= max_len {
        return Ok(());
    }
    Err(Error::limit(format!(
        "JSON string length {} exceeded {max_len}",
        output.len()
    )))
}
