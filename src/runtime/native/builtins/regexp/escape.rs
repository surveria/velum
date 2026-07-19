#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, object::PropertyEnumerable},
    value::{NativeFunctionId, Value},
};

use super::{NativeFunctionKind, REGEXP_ESCAPE_NAME};

const ESCAPE_OUTPUT_LIMIT_ERROR: &str = "RegExp.escape output exceeded string limit";
const ESCAPE_STRING_INPUT_ERROR: &str = "RegExp.escape requires a string";
const REVERSE_SOLIDUS: u16 = 0x005C;
const LOWER_X: u16 = 0x0078;
const LOWER_U: u16 = 0x0075;

impl Context {
    pub(super) fn install_regexp_static_methods(
        &mut self,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        let function =
            self.create_native_function(NativeFunctionKind::RegExpEscape, Value::Undefined)?;
        let key = self.intern_property_key(REGEXP_ESCAPE_NAME)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, function, PropertyEnumerable::No)
    }

    pub(in crate::runtime::native) fn eval_regexp_escape(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let Some(input) = args.as_slice().first().and_then(Value::string_units) else {
            return Err(Error::type_error(ESCAPE_STRING_INPUT_ERROR));
        };
        let input = input.into_owned();
        self.charge_runtime_steps(input.len())?;
        let output = escape_regexp_utf16(&input, self.limits.max_string_len)?;
        self.heap_utf16_string_value(&output)
    }
}

fn escape_regexp_utf16(input: &[u16], max_string_len: usize) -> Result<Vec<u16>> {
    let mut output = Vec::with_capacity(input.len().min(max_string_len));
    let mut index = 0usize;
    while let Some(unit) = input.get(index).copied() {
        let next = input.get(index.saturating_add(1)).copied();
        if is_leading_surrogate(unit)
            && let Some(trailing) = next.filter(|next| is_trailing_surrogate(*next))
        {
            extend_escape_output(&mut output, &[unit, trailing], max_string_len)?;
            index = index
                .checked_add(2)
                .ok_or_else(|| Error::limit(ESCAPE_OUTPUT_LIMIT_ERROR))?;
            continue;
        }
        encode_regexp_escape_unit(&mut output, unit, index == 0, max_string_len)?;
        index = index
            .checked_add(1)
            .ok_or_else(|| Error::limit(ESCAPE_OUTPUT_LIMIT_ERROR))?;
    }
    Ok(output)
}

fn encode_regexp_escape_unit(
    output: &mut Vec<u16>,
    unit: u16,
    first: bool,
    max_string_len: usize,
) -> Result<()> {
    if first && is_ascii_alphanumeric(unit) {
        return append_hex_escape(output, unit, max_string_len);
    }
    if is_syntax_character(unit) || unit == 0x002F {
        return extend_escape_output(output, &[REVERSE_SOLIDUS, unit], max_string_len);
    }
    if let Some(control) = control_escape(unit) {
        return extend_escape_output(output, &[REVERSE_SOLIDUS, control], max_string_len);
    }
    if is_other_punctuator(unit) || is_ecmascript_whitespace_or_line_terminator(unit) {
        return if unit <= 0x00FF {
            append_hex_escape(output, unit, max_string_len)
        } else {
            append_unicode_escape(output, unit, max_string_len)
        };
    }
    if is_leading_surrogate(unit) || is_trailing_surrogate(unit) {
        return append_unicode_escape(output, unit, max_string_len);
    }
    extend_escape_output(output, &[unit], max_string_len)
}

fn append_hex_escape(output: &mut Vec<u16>, unit: u16, max_string_len: usize) -> Result<()> {
    extend_escape_output(
        output,
        &[
            REVERSE_SOLIDUS,
            LOWER_X,
            hex_digit((unit >> 4) & 0x000F),
            hex_digit(unit & 0x000F),
        ],
        max_string_len,
    )
}

fn append_unicode_escape(output: &mut Vec<u16>, unit: u16, max_string_len: usize) -> Result<()> {
    extend_escape_output(
        output,
        &[
            REVERSE_SOLIDUS,
            LOWER_U,
            hex_digit((unit >> 12) & 0x000F),
            hex_digit((unit >> 8) & 0x000F),
            hex_digit((unit >> 4) & 0x000F),
            hex_digit(unit & 0x000F),
        ],
        max_string_len,
    )
}

fn extend_escape_output(output: &mut Vec<u16>, units: &[u16], max_string_len: usize) -> Result<()> {
    let length = output
        .len()
        .checked_add(units.len())
        .ok_or_else(|| Error::limit(ESCAPE_OUTPUT_LIMIT_ERROR))?;
    if length > max_string_len {
        return Err(Error::limit(ESCAPE_OUTPUT_LIMIT_ERROR));
    }
    output.extend_from_slice(units);
    Ok(())
}

const fn is_ascii_alphanumeric(unit: u16) -> bool {
    matches!(unit, 0x0030..=0x0039 | 0x0041..=0x005A | 0x0061..=0x007A)
}

const fn is_syntax_character(unit: u16) -> bool {
    matches!(
        unit,
        0x0024
            | 0x0028..=0x002B
            | 0x002E
            | 0x003F
            | 0x005B..=0x005E
            | 0x007B..=0x007D
    )
}

const fn is_other_punctuator(unit: u16) -> bool {
    matches!(
        unit,
        0x0021..=0x0023
            | 0x0025..=0x0026
            | 0x0027
            | 0x002C..=0x002D
            | 0x003A..=0x003E
            | 0x0040
            | 0x0060
            | 0x007E
    )
}

const fn control_escape(unit: u16) -> Option<u16> {
    match unit {
        0x0009 => Some(0x0074),
        0x000A => Some(0x006E),
        0x000B => Some(0x0076),
        0x000C => Some(0x0066),
        0x000D => Some(0x0072),
        _ => None,
    }
}

const fn is_ecmascript_whitespace_or_line_terminator(unit: u16) -> bool {
    matches!(
        unit,
        0x0020
            | 0x00A0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028..=0x2029
            | 0x202F
            | 0x205F
            | 0x3000
            | 0xFEFF
    )
}

const fn is_leading_surrogate(unit: u16) -> bool {
    matches!(unit, 0xD800..=0xDBFF)
}

const fn is_trailing_surrogate(unit: u16) -> bool {
    matches!(unit, 0xDC00..=0xDFFF)
}

const fn hex_digit(nibble: u16) -> u16 {
    match nibble {
        0 => 0x0030,
        1 => 0x0031,
        2 => 0x0032,
        3 => 0x0033,
        4 => 0x0034,
        5 => 0x0035,
        6 => 0x0036,
        7 => 0x0037,
        8 => 0x0038,
        9 => 0x0039,
        10 => 0x0061,
        11 => 0x0062,
        12 => 0x0063,
        13 => 0x0064,
        14 => 0x0065,
        15 => 0x0066,
        _ => 0x003F,
    }
}
