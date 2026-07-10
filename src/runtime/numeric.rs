use crate::error::{Error, Result};
use crate::runtime::Context;
use crate::value::Value;

const TO_INT32_MODULUS: f64 = 4_294_967_296.0;
const TO_INT32_SIGN_BOUNDARY: u32 = 2_147_483_648;
const TO_INT32_SIGN_OFFSET: i64 = 4_294_967_296;
const SHIFT_COUNT_MASK: u32 = 0x1f;
const F64_EXPONENT_SHIFT: u32 = 52;
const F64_EXPONENT_MASK: u64 = 0x7ff;
const F64_EXPONENT_BIAS: i32 = 1023;
const F64_SIGNIFICAND_BITS: i32 = 52;
const F64_MANTISSA_MASK: u64 = (1_u64 << F64_EXPONENT_SHIFT) - 1;
const F64_IMPLICIT_BIT: u64 = 1_u64 << F64_EXPONENT_SHIFT;

pub fn numeric_binary(
    context: &mut Context,
    left: &Value,
    right: &Value,
    _op: &str,
    apply: impl FnOnce(f64, f64) -> f64,
) -> Result<Value> {
    let left = context.to_number(left)?;
    let right = context.to_number(right)?;
    Ok(Value::Number(apply(left, right)))
}

pub fn bitwise_and(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    let left = bitwise_i32(context, left, "&")?;
    let right = bitwise_i32(context, right, "&")?;
    Ok(Value::Number(f64::from(left & right)))
}

pub fn bitwise_or(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    let left = bitwise_i32(context, left, "|")?;
    let right = bitwise_i32(context, right, "|")?;
    Ok(Value::Number(f64::from(left | right)))
}

pub fn bitwise_xor(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    let left = bitwise_i32(context, left, "^")?;
    let right = bitwise_i32(context, right, "^")?;
    Ok(Value::Number(f64::from(left ^ right)))
}

pub fn shift_left(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    let left = bitwise_i32(context, left, "<<")?;
    let right = shift_count(context, right, "<<")?;
    Ok(Value::Number(f64::from(left.wrapping_shl(right))))
}

pub fn shift_right(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    let left = bitwise_i32(context, left, ">>")?;
    let right = shift_count(context, right, ">>")?;
    Ok(Value::Number(f64::from(left.wrapping_shr(right))))
}

pub fn shift_right_unsigned(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    let left = bitwise_u32(context, left, ">>>")?;
    let right = shift_count(context, right, ">>>")?;
    Ok(Value::Number(f64::from(left.wrapping_shr(right))))
}

fn shift_count(context: &mut Context, value: &Value, op: &str) -> Result<u32> {
    Ok(bitwise_u32(context, value, op)? & SHIFT_COUNT_MASK)
}

fn bitwise_i32(context: &mut Context, value: &Value, op: &str) -> Result<i32> {
    number_to_i32(context.to_number(value)?, op)
}

fn bitwise_u32(context: &mut Context, value: &Value, op: &str) -> Result<u32> {
    number_to_uint32(context.to_number(value)?, op)
}

pub fn number_to_uint32(value: f64, context: &str) -> Result<u32> {
    if !value.is_finite() || value == 0.0 {
        return Ok(0);
    }

    let truncated = if value.is_sign_negative() {
        value.ceil()
    } else {
        value.floor()
    };
    let modulo = truncated.rem_euclid(TO_INT32_MODULUS);
    modulo_to_u32(modulo, context)
}

pub fn number_to_i32(value: f64, op: &str) -> Result<i32> {
    let unsigned = number_to_uint32(value, op)?;
    uint32_to_int32(unsigned, op)
}

pub fn number_shift_count(value: f64, op: &str) -> Result<u32> {
    Ok(number_to_uint32(value, op)? & SHIFT_COUNT_MASK)
}

fn uint32_to_int32(unsigned: u32, op: &str) -> Result<i32> {
    let signed = if unsigned >= TO_INT32_SIGN_BOUNDARY {
        i64::from(unsigned)
            .checked_sub(TO_INT32_SIGN_OFFSET)
            .ok_or_else(|| Error::runtime(format!("bitwise '{op}' int32 conversion overflowed")))?
    } else {
        i64::from(unsigned)
    };

    i32::try_from(signed)
        .map_err(|_| Error::runtime(format!("bitwise '{op}' failed to convert number to int32")))
}

fn modulo_to_u32(value: f64, op: &str) -> Result<u32> {
    if value == 0.0 {
        return Ok(0);
    }
    if !(0.0..TO_INT32_MODULUS).contains(&value) {
        return Err(Error::runtime(format!(
            "numeric '{op}' uint32 conversion overflowed"
        )));
    }

    let bits = value.to_bits();
    let exponent_bits = u16::try_from((bits >> F64_EXPONENT_SHIFT) & F64_EXPONENT_MASK)
        .map_err(|_| Error::runtime(format!("numeric '{op}' exponent conversion overflowed")))?;
    if exponent_bits == 0 {
        return Ok(0);
    }

    let exponent = i32::from(exponent_bits)
        .checked_sub(F64_EXPONENT_BIAS)
        .ok_or_else(|| Error::runtime(format!("numeric '{op}' exponent conversion overflowed")))?;
    if exponent < 0 {
        return Ok(0);
    }

    let mantissa = bits & F64_MANTISSA_MASK;
    let significand = F64_IMPLICIT_BIT | mantissa;
    let unsigned = if exponent >= F64_SIGNIFICAND_BITS {
        let shift = u32::try_from(
            exponent
                .checked_sub(F64_SIGNIFICAND_BITS)
                .ok_or_else(|| Error::runtime("bitwise exponent shift underflowed"))?,
        )
        .map_err(|_| Error::runtime(format!("numeric '{op}' shift conversion overflowed")))?;
        significand
            .checked_shl(shift)
            .ok_or_else(|| Error::runtime(format!("numeric '{op}' significand overflowed")))?
    } else {
        let shift = u32::try_from(
            F64_SIGNIFICAND_BITS
                .checked_sub(exponent)
                .ok_or_else(|| Error::runtime("bitwise exponent shift underflowed"))?,
        )
        .map_err(|_| Error::runtime(format!("numeric '{op}' shift conversion overflowed")))?;
        significand >> shift
    };

    u32::try_from(unsigned)
        .map_err(|_| Error::runtime(format!("numeric '{op}' failed to convert number to uint32")))
}
