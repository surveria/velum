#[cfg(not(feature = "std"))]
use crate::prelude::*;

use num_bigint::BigUint;
use num_traits::Zero;

use crate::error::{Error, Result};
use crate::runtime::object::NumberFormatValue;

const MAX_DECIMAL_EXPONENT: i32 = 10_000;

#[derive(Clone, Debug)]
pub(super) struct DecimalInput {
    coefficient: BigUint,
    exponent: i32,
    pub(super) negative: bool,
}

impl DecimalInput {
    pub(super) fn scale_power(&mut self, exponent: i32) -> Result<()> {
        self.exponent = self
            .exponent
            .checked_add(exponent)
            .ok_or_else(|| Error::limit("decimal scaling exponent overflowed"))?;
        Ok(())
    }

    pub(super) fn magnitude(&self) -> Result<i32> {
        if self.coefficient.is_zero() {
            return Ok(0);
        }
        let digits = i32::try_from(self.coefficient.to_string().len())
            .map_err(|_| Error::limit("decimal digit count exceeded supported range"))?;
        digits
            .checked_sub(1)
            .and_then(|digits| digits.checked_add(self.exponent))
            .ok_or_else(|| Error::limit("decimal magnitude overflowed"))
    }
}

#[derive(Clone, Debug)]
pub(super) enum NumberInput {
    Finite(DecimalInput),
    Infinity { negative: bool },
    Nan,
}

#[derive(Clone, Debug)]
pub(super) struct RoundedNumber {
    pub(super) text: String,
    pub(super) negative: bool,
    pub(super) zero: bool,
}

pub(super) fn parse_number_input(text: &str) -> Result<NumberInput> {
    let text = text.trim();
    let (negative, unsigned) = text
        .strip_prefix('-')
        .map_or((false, text), |unsigned| (true, unsigned));
    let unsigned = unsigned.strip_prefix('+').unwrap_or(unsigned);
    if unsigned == "Infinity" {
        return Ok(NumberInput::Infinity { negative });
    }
    if unsigned.eq_ignore_ascii_case("nan") || unsigned.is_empty() {
        return Ok(NumberInput::Nan);
    }
    let (mantissa, explicit_exponent) = split_exponent(unsigned)?;
    let (integer, fraction) = mantissa
        .split_once('.')
        .map_or((mantissa, ""), |parts| parts);
    if integer.is_empty() && fraction.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Ok(NumberInput::Nan);
    }
    let digits = format!("{integer}{fraction}");
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return Ok(NumberInput::Finite(DecimalInput {
            coefficient: BigUint::zero(),
            exponent: 0,
            negative,
        }));
    }
    let coefficient = BigUint::parse_bytes(significant.as_bytes(), 10)
        .ok_or_else(|| Error::runtime("decimal coefficient is invalid"))?;
    let fraction_length =
        i32::try_from(fraction.len()).map_err(|_| Error::limit("decimal fraction is too long"))?;
    let exponent = explicit_exponent
        .checked_sub(fraction_length)
        .ok_or_else(|| Error::limit("decimal exponent underflowed"))?;
    if exponent.unsigned_abs() > MAX_DECIMAL_EXPONENT.unsigned_abs() {
        return Err(Error::limit("decimal exponent exceeded supported range"));
    }
    Ok(NumberInput::Finite(DecimalInput {
        coefficient,
        exponent,
        negative,
    }))
}

pub(super) fn round_fraction(
    input: &DecimalInput,
    minimum: u8,
    maximum: u8,
    increment: u16,
    mode: &str,
    trailing_zero_display: &str,
) -> Result<RoundedNumber> {
    let maximum = i32::from(maximum);
    let aligned_exponent = input
        .exponent
        .checked_add(maximum)
        .ok_or_else(|| Error::limit("fraction rounding exponent overflowed"))?;
    let increment = BigUint::from(increment);
    let (numerator, denominator) = if aligned_exponent >= 0 {
        (
            &input.coefficient * decimal_power(aligned_exponent)?,
            increment.clone(),
        )
    } else {
        (
            input.coefficient.clone(),
            increment.clone() * decimal_power(aligned_exponent.saturating_neg())?,
        )
    };
    let quotient = &numerator / &denominator;
    let remainder = &numerator % &denominator;
    let rounded_units = rounded_quotient(quotient, &remainder, &denominator, mode, input.negative);
    let coefficient = rounded_units * increment;
    let exponent = maximum.saturating_neg();
    let mut text = render_decimal(&coefficient, exponent)?;
    let minimum = if trailing_zero_display == "stripIfInteger" && !text.contains('.') {
        0
    } else {
        usize::from(minimum)
    };
    trim_fraction(&mut text, minimum);
    ensure_fraction(&mut text, minimum);
    Ok(RoundedNumber {
        zero: coefficient.is_zero(),
        text,
        negative: input.negative,
    })
}

pub(super) fn round_significant(
    input: &DecimalInput,
    minimum: u8,
    maximum: u8,
    mode: &str,
) -> Result<RoundedNumber> {
    if input.coefficient.is_zero() {
        let minimum = usize::from(minimum.saturating_sub(1));
        let text = if minimum == 0 {
            "0".to_owned()
        } else {
            format!("0.{}", "0".repeat(minimum))
        };
        return Ok(RoundedNumber {
            text,
            negative: input.negative,
            zero: true,
        });
    }
    let digits = input.coefficient.to_string();
    let maximum = usize::from(maximum);
    let removed = digits.len().saturating_sub(maximum);
    let divisor = decimal_power_usize(removed)?;
    let quotient = &input.coefficient / &divisor;
    let remainder = &input.coefficient % &divisor;
    let coefficient = rounded_quotient(quotient, &remainder, &divisor, mode, input.negative);
    let removed = i32::try_from(removed)
        .map_err(|_| Error::limit("significant digit count exceeded supported range"))?;
    let exponent = input
        .exponent
        .checked_add(removed)
        .ok_or_else(|| Error::limit("significant rounding exponent overflowed"))?;
    let mut text = render_decimal(&coefficient, exponent)?;
    trim_significant(&mut text, usize::from(minimum));
    ensure_significant(&mut text, usize::from(minimum));
    Ok(RoundedNumber {
        zero: coefficient.is_zero(),
        text,
        negative: input.negative,
    })
}

pub(super) fn round_standard(
    input: &DecimalInput,
    formatter: &NumberFormatValue,
) -> Result<RoundedNumber> {
    let Some(maximum_significant) = formatter.maximum_significant_digits else {
        return round_fraction(
            input,
            formatter.minimum_fraction_digits,
            formatter.maximum_fraction_digits,
            formatter.rounding_increment,
            &formatter.rounding_mode,
            &formatter.trailing_zero_display,
        );
    };
    let significant = round_significant(
        input,
        formatter.minimum_significant_digits.unwrap_or(1),
        maximum_significant,
        &formatter.rounding_mode,
    )?;
    if formatter.rounding_priority == "auto" {
        return Ok(significant);
    }
    let fraction = round_fraction(
        input,
        formatter.minimum_fraction_digits,
        formatter.maximum_fraction_digits,
        formatter.rounding_increment,
        &formatter.rounding_mode,
        &formatter.trailing_zero_display,
    )?;
    let significant_magnitude = input
        .magnitude()?
        .saturating_sub(i32::from(maximum_significant))
        .saturating_add(1);
    let fraction_magnitude = i32::from(formatter.maximum_fraction_digits).saturating_neg();
    let significant_wins = if formatter.rounding_priority == "morePrecision" {
        significant_magnitude <= fraction_magnitude
    } else {
        significant_magnitude >= fraction_magnitude
    };
    Ok(if significant_wins {
        significant
    } else {
        fraction
    })
}

fn split_exponent(value: &str) -> Result<(&str, i32)> {
    let Some(index) = value.find(['e', 'E']) else {
        return Ok((value, 0));
    };
    let mantissa = value
        .get(..index)
        .ok_or_else(|| Error::runtime("decimal mantissa boundary is invalid"))?;
    let exponent = value
        .get(index.saturating_add(1)..)
        .ok_or_else(|| Error::runtime("decimal exponent boundary is invalid"))?
        .parse::<i32>()
        .map_err(|_| Error::limit("decimal exponent is invalid"))?;
    Ok((mantissa, exponent))
}

fn rounded_quotient(
    quotient: BigUint,
    remainder: &BigUint,
    denominator: &BigUint,
    mode: &str,
    negative: bool,
) -> BigUint {
    if remainder.is_zero() {
        return quotient;
    }
    let twice_remainder = remainder * BigUint::from(2_u8);
    let above_half = twice_remainder > *denominator;
    let tie = twice_remainder == *denominator;
    let odd = (&quotient % BigUint::from(2_u8)) == BigUint::from(1_u8);
    let increment = match mode {
        "ceil" => !negative,
        "floor" => negative,
        "expand" => true,
        "trunc" => false,
        "halfCeil" => above_half || (tie && !negative),
        "halfFloor" => above_half || (tie && negative),
        "halfTrunc" => above_half,
        "halfEven" => above_half || (tie && odd),
        _ => above_half || tie,
    };
    if increment {
        quotient + BigUint::from(1_u8)
    } else {
        quotient
    }
}

fn decimal_power(exponent: i32) -> Result<BigUint> {
    let exponent =
        u32::try_from(exponent).map_err(|_| Error::limit("decimal exponent cannot be negative"))?;
    Ok(BigUint::from(10_u8).pow(exponent))
}

fn decimal_power_usize(exponent: usize) -> Result<BigUint> {
    let exponent = u32::try_from(exponent)
        .map_err(|_| Error::limit("decimal exponent exceeded supported range"))?;
    Ok(BigUint::from(10_u8).pow(exponent))
}

fn render_decimal(coefficient: &BigUint, exponent: i32) -> Result<String> {
    if coefficient.is_zero() {
        return Ok("0".to_owned());
    }
    let digits = coefficient.to_string();
    let digit_count = i32::try_from(digits.len())
        .map_err(|_| Error::limit("decimal digit count exceeded supported range"))?;
    let decimal_position = digit_count
        .checked_add(exponent)
        .ok_or_else(|| Error::limit("decimal position overflowed"))?;
    if decimal_position <= 0 {
        let zeros = usize::try_from(decimal_position.saturating_neg())
            .map_err(|_| Error::limit("decimal prefix exceeded supported range"))?;
        return Ok(format!("0.{}{digits}", "0".repeat(zeros)));
    }
    if decimal_position >= digit_count {
        let zeros = usize::try_from(decimal_position.saturating_sub(digit_count))
            .map_err(|_| Error::limit("decimal suffix exceeded supported range"))?;
        return Ok(format!("{digits}{}", "0".repeat(zeros)));
    }
    let position = usize::try_from(decimal_position)
        .map_err(|_| Error::limit("decimal position exceeded supported range"))?;
    let integer = digits
        .get(..position)
        .ok_or_else(|| Error::runtime("decimal integer boundary is invalid"))?;
    let fraction = digits
        .get(position..)
        .ok_or_else(|| Error::runtime("decimal fraction boundary is invalid"))?;
    Ok(format!("{integer}.{fraction}"))
}

fn trim_fraction(text: &mut String, minimum: usize) {
    let Some(dot) = text.find('.') else {
        return;
    };
    while text.len().saturating_sub(dot).saturating_sub(1) > minimum && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
}

fn ensure_fraction(text: &mut String, minimum: usize) {
    if minimum == 0 {
        return;
    }
    let current = text
        .find('.')
        .map_or(0, |dot| text.len().saturating_sub(dot).saturating_sub(1));
    if current == 0 {
        text.push('.');
    }
    text.push_str(&"0".repeat(minimum.saturating_sub(current)));
}

fn significant_count(text: &str) -> usize {
    let mut nonzero_seen = false;
    let mut count = 0_usize;
    for digit in text.bytes().filter(u8::is_ascii_digit) {
        if digit != b'0' {
            nonzero_seen = true;
        }
        if nonzero_seen {
            count = count.saturating_add(1);
        }
    }
    count.max(1)
}

fn trim_significant(text: &mut String, minimum: usize) {
    while text.contains('.') && text.ends_with('0') && significant_count(text) > minimum {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
}

fn ensure_significant(text: &mut String, minimum: usize) {
    let missing = minimum.saturating_sub(significant_count(text));
    if missing == 0 {
        return;
    }
    if !text.contains('.') {
        text.push('.');
    }
    text.push_str(&"0".repeat(missing));
}
