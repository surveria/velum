use temporal_rs::{Duration, Sign};

use crate::{
    error::{Error, Result},
    runtime::object::DurationFormatValue,
};

use super::duration_format::DURATION_UNITS;

const NANOS_PER_SECOND: i128 = 1_000_000_000;
const NANOS_PER_MILLISECOND: i128 = 1_000_000;
const NANOS_PER_MICROSECOND: i128 = 1_000;

#[derive(Clone)]
pub(super) struct DurationPart {
    pub kind: &'static str,
    pub value: String,
    pub unit: Option<&'static str>,
}

struct DecimalValue {
    integer: String,
    fraction: String,
    zero: bool,
}

pub(super) fn format_duration_parts(
    formatter: &DurationFormatValue,
    duration: &Duration,
) -> Result<Vec<DurationPart>> {
    let values = duration_values(duration);
    let negative = duration.sign() == Sign::Negative;
    let mut groups: Vec<Vec<DurationPart>> = Vec::new();
    let mut need_separator = false;
    let mut first_number = true;

    for index in 0..DURATION_UNITS.len() {
        let Some(unit_options) = formatter.units.get(index) else {
            return Err(Error::runtime("DurationFormat unit table is invalid"));
        };
        let (decimal, done) = duration_decimal(formatter, &values, index)?;
        let display_required = index == 5
            && need_separator
            && (formatter
                .units
                .get(6)
                .is_some_and(|seconds| seconds.display == "always")
                || values.iter().skip(6).any(|value| *value != 0));
        let displayed = !decimal.zero || unit_options.display == "always" || display_required;
        if displayed {
            let unit = duration_unit_name(index)?;
            let mut parts = number_parts(
                formatter,
                &decimal,
                &unit_options.style,
                unit,
                first_number && negative,
            )?;
            first_number = false;
            if need_separator {
                let Some(group) = groups.last_mut() else {
                    return Err(Error::runtime(
                        "DurationFormat numeric group is unavailable",
                    ));
                };
                group.push(DurationPart {
                    kind: "literal",
                    value: ":".to_owned(),
                    unit: None,
                });
                group.append(&mut parts);
            } else {
                need_separator = matches!(unit_options.style.as_str(), "numeric" | "2-digit");
                groups.push(parts);
            }
        }
        if done {
            break;
        }
    }
    Ok(flatten_duration_groups(formatter, groups))
}

fn duration_values(duration: &Duration) -> [i128; 10] {
    [
        i128::from(duration.years()),
        i128::from(duration.months()),
        i128::from(duration.weeks()),
        i128::from(duration.days()),
        i128::from(duration.hours()),
        i128::from(duration.minutes()),
        i128::from(duration.seconds()),
        i128::from(duration.milliseconds()),
        duration.microseconds(),
        duration.nanoseconds(),
    ]
}

fn duration_decimal(
    formatter: &DurationFormatValue,
    values: &[i128; 10],
    index: usize,
) -> Result<(DecimalValue, bool)> {
    let next_numeric = formatter
        .units
        .get(index.saturating_add(1))
        .is_some_and(|unit| unit.style == "numeric");
    let exponent = match (index, next_numeric) {
        (6, true) => Some(9_u8),
        (7, true) => Some(6_u8),
        (8, true) => Some(3_u8),
        _ => None,
    };
    let Some(exponent) = exponent else {
        let value = *values
            .get(index)
            .ok_or_else(|| Error::runtime("DurationFormat value table is invalid"))?;
        return Ok((integer_decimal(value), false));
    };
    let total = combined_subseconds(values, index)?;
    Ok((
        scaled_decimal(total, exponent, formatter.fractional_digits)?,
        true,
    ))
}

fn combined_subseconds(values: &[i128; 10], index: usize) -> Result<i128> {
    let seconds = if index == 6 {
        checked_scale(value_at(values, 6)?, NANOS_PER_SECOND)?
    } else {
        0
    };
    let milliseconds = if index <= 7 {
        checked_scale(value_at(values, 7)?, NANOS_PER_MILLISECOND)?
    } else {
        0
    };
    let microseconds = if index <= 8 {
        checked_scale(value_at(values, 8)?, NANOS_PER_MICROSECOND)?
    } else {
        0
    };
    let nanoseconds = value_at(values, 9)?;
    let total = seconds
        .checked_add(milliseconds)
        .and_then(|value| value.checked_add(microseconds))
        .and_then(|value| value.checked_add(nanoseconds))
        .ok_or_else(|| Error::limit("DurationFormat subsecond value overflowed"))?;
    if !(6..=8).contains(&index) {
        return Err(Error::runtime("DurationFormat fractional unit is invalid"));
    }
    Ok(total)
}

fn value_at(values: &[i128; 10], index: usize) -> Result<i128> {
    values
        .get(index)
        .copied()
        .ok_or_else(|| Error::runtime("DurationFormat value table is invalid"))
}

fn checked_scale(value: i128, scale: i128) -> Result<i128> {
    value
        .checked_mul(scale)
        .ok_or_else(|| Error::limit("DurationFormat subsecond value overflowed"))
}

fn integer_decimal(value: i128) -> DecimalValue {
    DecimalValue {
        integer: value.unsigned_abs().to_string(),
        fraction: String::new(),
        zero: value == 0,
    }
}

fn scaled_decimal(
    scaled_value: i128,
    exponent: u8,
    fractional_digits: Option<u8>,
) -> Result<DecimalValue> {
    let scale = 10_i128
        .checked_pow(u32::from(exponent))
        .ok_or_else(|| Error::limit("DurationFormat fraction scale overflowed"))?;
    let absolute = scaled_value.unsigned_abs();
    let scale_unsigned = scale.unsigned_abs();
    let integer = (absolute / scale_unsigned).to_string();
    let remainder = absolute % scale_unsigned;
    let width = usize::from(exponent);
    let full_fraction = format!("{remainder:0width$}");
    let fraction = if let Some(digits) = fractional_digits {
        resize_fraction(&full_fraction, usize::from(digits))?
    } else {
        full_fraction.trim_end_matches('0').to_owned()
    };
    Ok(DecimalValue {
        integer,
        fraction,
        zero: scaled_value == 0,
    })
}

fn resize_fraction(value: &str, digits: usize) -> Result<String> {
    if digits <= value.len() {
        return value
            .get(..digits)
            .map(str::to_owned)
            .ok_or_else(|| Error::runtime("DurationFormat fraction boundary is invalid"));
    }
    let mut result = value.to_owned();
    result.extend(std::iter::repeat_n('0', digits.saturating_sub(value.len())));
    Ok(result)
}

fn number_parts(
    formatter: &DurationFormatValue,
    decimal: &DecimalValue,
    style: &str,
    unit: &'static str,
    negative: bool,
) -> Result<Vec<DurationPart>> {
    let numeric = matches!(style, "numeric" | "2-digit");
    let minimum_digits = if style == "2-digit" { 2 } else { 1 };
    let mut integer = decimal.integer.clone();
    if integer.len() < minimum_digits {
        integer = format!("{integer:0>minimum_digits$}");
    }
    let mut parts = Vec::new();
    if negative {
        parts.push(number_part("minusSign", "-", unit));
    }
    if numeric {
        parts.push(number_part(
            "integer",
            &localize_digits(&integer, &formatter.numbering_system)?,
            unit,
        ));
    } else {
        parts.extend(grouped_integer_parts(
            &integer,
            &formatter.numbering_system,
            unit,
        )?);
    }
    if !decimal.fraction.is_empty() {
        parts.push(number_part("decimal", ".", unit));
        parts.push(number_part(
            "fraction",
            &localize_digits(&decimal.fraction, &formatter.numbering_system)?,
            unit,
        ));
    }
    if !numeric {
        let singular = decimal.integer == "1" && decimal.fraction.is_empty();
        let (separator, label) =
            super::duration_units::duration_unit_pattern(unit, style, singular)
                .unwrap_or((" ", unit));
        if !separator.is_empty() {
            parts.push(number_part("literal", separator, unit));
        }
        parts.push(number_part("unit", label, unit));
    }
    Ok(parts)
}

fn grouped_integer_parts(
    integer: &str,
    numbering_system: &str,
    unit: &'static str,
) -> Result<Vec<DurationPart>> {
    if integer.len() <= 3 {
        return Ok(vec![number_part(
            "integer",
            &localize_digits(integer, numbering_system)?,
            unit,
        )]);
    }
    let first_len = {
        let remainder = integer.len() % 3;
        if remainder == 0 { 3 } else { remainder }
    };
    let mut parts = Vec::new();
    let mut start = 0_usize;
    let mut width = first_len;
    while start < integer.len() {
        let end = start
            .checked_add(width)
            .ok_or_else(|| Error::limit("DurationFormat grouping boundary overflowed"))?;
        let group = integer
            .get(start..end)
            .ok_or_else(|| Error::runtime("DurationFormat grouping boundary is invalid"))?;
        if !parts.is_empty() {
            parts.push(number_part("group", ",", unit));
        }
        parts.push(number_part(
            "integer",
            &localize_digits(group, numbering_system)?,
            unit,
        ));
        start = end;
        width = 3;
    }
    Ok(parts)
}

fn number_part(kind: &'static str, value: &str, unit: &'static str) -> DurationPart {
    DurationPart {
        kind,
        value: value.to_owned(),
        unit: Some(unit),
    }
}

fn localize_digits(value: &str, numbering_system: &str) -> Result<String> {
    let digits = super::number_digits::digits(numbering_system)
        .ok_or_else(|| Error::runtime("DurationFormat numbering system is unavailable"))?
        .chars()
        .collect::<Vec<_>>();
    let mut result = String::with_capacity(value.len());
    for character in value.chars() {
        let Some(index) = character.to_digit(10) else {
            result.push(character);
            continue;
        };
        let index = usize::try_from(index)
            .map_err(|_| Error::runtime("DurationFormat digit index is invalid"))?;
        let digit = digits
            .get(index)
            .copied()
            .ok_or_else(|| Error::runtime("DurationFormat digit map is invalid"))?;
        result.push(digit);
    }
    Ok(result)
}

fn duration_unit_name(index: usize) -> Result<&'static str> {
    DURATION_UNITS
        .get(index)
        .and_then(|unit| unit.strip_suffix('s'))
        .ok_or_else(|| Error::runtime("DurationFormat unit name is invalid"))
}

fn flatten_duration_groups(
    formatter: &DurationFormatValue,
    groups: Vec<Vec<DurationPart>>,
) -> Vec<DurationPart> {
    let count = groups.len();
    let list_style = if formatter.style == "digital" {
        "short"
    } else {
        formatter.style.as_str()
    };
    let spanish = formatter
        .locale
        .split('-')
        .next()
        .is_some_and(|language| language.eq_ignore_ascii_case("es"));
    let mut result = Vec::new();
    for (index, mut group) in groups.into_iter().enumerate() {
        if index > 0 {
            result.push(DurationPart {
                kind: "literal",
                value: duration_list_separator(list_style, spanish, count, index).to_owned(),
                unit: None,
            });
        }
        result.append(&mut group);
    }
    result
}

fn duration_list_separator(style: &str, spanish: bool, count: usize, index: usize) -> &'static str {
    if style == "narrow" {
        return " ";
    }
    let last = index == count.saturating_sub(1);
    match (style, spanish, count, last) {
        ("long", true, _, true) | ("short", true, 2, true) => " y ",
        _ => ", ",
    }
}
