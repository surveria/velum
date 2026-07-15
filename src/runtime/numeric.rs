use crate::error::{Error, Result};
use crate::runtime::{Context, abstract_operations::NumericValue};
use crate::syntax::BinaryOp;
use crate::value::{ErrorName, JsBigInt, Value};

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
const BINARY16_SIGN_MASK: u16 = 0x8000;
const BINARY16_EXPONENT_MASK: u16 = 0x7c00;
const BINARY16_FRACTION_MASK: u16 = 0x03ff;
const BINARY16_QUIET_NAN: u16 = 0x7e00;
const BINARY16_INFINITY: u16 = 0x7c00;
const BINARY16_EXPONENT_BIAS: i32 = 15;
const BINARY16_FRACTION_BITS: u32 = 10;
const BINARY16_NORMAL_ROUND_SHIFT: u32 = 42;
const MIXED_NUMERIC_TYPES_ERROR: &str = "Cannot mix BigInt and other types";
const BIGINT_DIVISION_BY_ZERO_ERROR: &str = "BigInt division by zero";
const BIGINT_NEGATIVE_EXPONENT_ERROR: &str = "BigInt exponent must be non-negative";
const BIGINT_UNSIGNED_SHIFT_ERROR: &str = "BigInt does not support unsigned right shift";

pub(in crate::runtime) fn binary16_to_f64(bits: u16) -> f64 {
    let sign = if bits & BINARY16_SIGN_MASK == 0 {
        1.0
    } else {
        -1.0
    };
    let exponent = (bits & BINARY16_EXPONENT_MASK) >> BINARY16_FRACTION_BITS;
    let fraction = bits & BINARY16_FRACTION_MASK;
    match exponent {
        0 if fraction == 0 => sign * 0.0,
        0 => sign * f64::from(fraction) * 2.0_f64.powi(-24),
        31 if fraction == 0 => sign * f64::INFINITY,
        31 => f64::NAN,
        _ => {
            let significand = 1.0 + f64::from(fraction) / 1024.0;
            sign * significand * 2.0_f64.powi(i32::from(exponent) - BINARY16_EXPONENT_BIAS)
        }
    }
}

pub(in crate::runtime) fn f64_to_binary16(value: f64) -> u16 {
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 {
        0
    } else {
        BINARY16_SIGN_MASK
    };
    let exponent_bits = (bits >> F64_EXPONENT_SHIFT) & F64_EXPONENT_MASK;
    let fraction = bits & F64_MANTISSA_MASK;
    if exponent_bits == F64_EXPONENT_MASK {
        return if fraction == 0 {
            sign | BINARY16_INFINITY
        } else {
            sign | BINARY16_QUIET_NAN
        };
    }
    if exponent_bits == 0 {
        return sign;
    }
    let Ok(exponent_bits) = i32::try_from(exponent_bits) else {
        return sign | BINARY16_QUIET_NAN;
    };
    let exponent = exponent_bits - F64_EXPONENT_BIAS;
    let significand = F64_IMPLICIT_BIT | fraction;
    let encoded = if exponent >= -14 {
        encode_normal_binary16(significand, exponent)
    } else {
        let Ok(shift) = u32::try_from(28_i32.saturating_sub(exponent)) else {
            return sign;
        };
        round_right_ties_even(significand, shift)
    };
    let Ok(encoded) = u16::try_from(encoded) else {
        return sign | BINARY16_INFINITY;
    };
    sign | encoded
}

pub(in crate::runtime) fn round_to_binary16(value: f64) -> f64 {
    binary16_to_f64(f64_to_binary16(value))
}

fn encode_normal_binary16(significand: u64, exponent: i32) -> u64 {
    let rounded = round_right_ties_even(significand, BINARY16_NORMAL_ROUND_SHIFT);
    let mut half_exponent = exponent + BINARY16_EXPONENT_BIAS;
    let mut half_significand = rounded;
    if half_significand == 2048 {
        half_exponent = half_exponent.saturating_add(1);
        half_significand = 1024;
    }
    if half_exponent >= 31 {
        return u64::from(BINARY16_INFINITY);
    }
    let Ok(exponent_field) = u64::try_from(half_exponent) else {
        return 0;
    };
    (exponent_field << BINARY16_FRACTION_BITS) | half_significand.saturating_sub(1024)
}

const fn round_right_ties_even(value: u64, shift: u32) -> u64 {
    if shift == 0 {
        return value;
    }
    if shift >= u64::BITS {
        return 0;
    }
    let retained = value >> shift;
    let mask = (1_u64 << shift).saturating_sub(1);
    let discarded = value & mask;
    let halfway = 1_u64 << shift.saturating_sub(1);
    if discarded > halfway || (discarded == halfway && retained % 2 == 1) {
        retained.saturating_add(1)
    } else {
        retained
    }
}

pub fn numeric_binary(
    context: &mut Context,
    left: &Value,
    right: &Value,
    op: BinaryOp,
) -> Result<Value> {
    let left = context.to_numeric(left)?;
    let right = context.to_numeric(right)?;
    match (left, right) {
        (NumericValue::Number(left), NumericValue::Number(right)) => {
            Ok(Value::Number(apply_number_binary(op, left, right)?))
        }
        (NumericValue::BigInt(left), NumericValue::BigInt(right)) => {
            let value = apply_bigint_binary(op, &left, &right, context.limits.max_bigint_bits)?;
            context.bigint_value(value)
        }
        (NumericValue::Number(_), NumericValue::BigInt(_))
        | (NumericValue::BigInt(_), NumericValue::Number(_)) => {
            Err(Error::type_error(MIXED_NUMERIC_TYPES_ERROR))
        }
    }
}

pub fn bitwise_and(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    numeric_binary(context, left, right, BinaryOp::BitAnd)
}

pub fn bitwise_or(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    numeric_binary(context, left, right, BinaryOp::BitOr)
}

pub fn bitwise_xor(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    numeric_binary(context, left, right, BinaryOp::BitXor)
}

pub fn bitwise_not(context: &mut Context, value: &Value) -> Result<Value> {
    match context.to_numeric(value)? {
        NumericValue::Number(value) => Ok(Value::Number(f64::from(!number_to_i32(value, "~")?))),
        NumericValue::BigInt(value) => context.bigint_value(value.bitwise_not()),
    }
}

pub fn shift_left(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    numeric_binary(context, left, right, BinaryOp::ShiftLeft)
}

pub fn shift_right(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    numeric_binary(context, left, right, BinaryOp::ShiftRight)
}

pub fn shift_right_unsigned(context: &mut Context, left: &Value, right: &Value) -> Result<Value> {
    numeric_binary(context, left, right, BinaryOp::ShiftRightUnsigned)
}

fn apply_number_binary(op: BinaryOp, left: f64, right: f64) -> Result<f64> {
    Ok(match op {
        BinaryOp::Sub => left - right,
        BinaryOp::Mul => left * right,
        BinaryOp::Div => left / right,
        BinaryOp::Rem => left % right,
        BinaryOp::Pow => number_exponentiate(left, right),
        BinaryOp::BitAnd => f64::from(number_to_i32(left, "&")? & number_to_i32(right, "&")?),
        BinaryOp::BitOr => f64::from(number_to_i32(left, "|")? | number_to_i32(right, "|")?),
        BinaryOp::BitXor => f64::from(number_to_i32(left, "^")? ^ number_to_i32(right, "^")?),
        BinaryOp::ShiftLeft => {
            f64::from(number_to_i32(left, "<<")?.wrapping_shl(number_shift_count(right, "<<")?))
        }
        BinaryOp::ShiftRight => {
            f64::from(number_to_i32(left, ">>")?.wrapping_shr(number_shift_count(right, ">>")?))
        }
        BinaryOp::ShiftRightUnsigned => f64::from(
            number_to_uint32(left, ">>>")?.wrapping_shr(number_shift_count(right, ">>>")?),
        ),
        BinaryOp::Add
        | BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::StrictEqual
        | BinaryOp::StrictNotEqual
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::Greater
        | BinaryOp::GreaterEqual
        | BinaryOp::In
        | BinaryOp::InstanceOf
        | BinaryOp::LogicalAnd
        | BinaryOp::LogicalOr
        | BinaryOp::NullishCoalescing => {
            return Err(Error::runtime(
                "non-numeric operator reached numeric binary owner",
            ));
        }
    })
}

pub(in crate::runtime) fn number_exponentiate(base: f64, exponent: f64) -> f64 {
    if exponent.is_nan() || (is_exact_abs_one(base) && exponent.is_infinite()) {
        return f64::NAN;
    }
    base.powf(exponent)
}

const fn is_exact_abs_one(value: f64) -> bool {
    matches!(
        value.to_bits(),
        bits if bits == 1.0_f64.to_bits() || bits == (-1.0_f64).to_bits()
    )
}

fn apply_bigint_binary(
    op: BinaryOp,
    left: &JsBigInt,
    right: &JsBigInt,
    max_bits: usize,
) -> Result<JsBigInt> {
    match op {
        BinaryOp::Sub => Ok(left.sub(right)),
        BinaryOp::Mul => Ok(bigint_multiply(left, right)),
        BinaryOp::Div => left.div(right).ok_or_else(bigint_division_by_zero),
        BinaryOp::Rem => left.rem(right).ok_or_else(bigint_division_by_zero),
        BinaryOp::Pow => bigint_pow(left, right, max_bits),
        BinaryOp::BitAnd => Ok(left.bitand(right)),
        BinaryOp::BitOr => Ok(left.bitor(right)),
        BinaryOp::BitXor => Ok(left.bitxor(right)),
        BinaryOp::ShiftLeft => bigint_shift(left, right, true, max_bits),
        BinaryOp::ShiftRight => bigint_shift(left, right, false, max_bits),
        BinaryOp::ShiftRightUnsigned => Err(Error::type_error(BIGINT_UNSIGNED_SHIFT_ERROR)),
        BinaryOp::Add
        | BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::StrictEqual
        | BinaryOp::StrictNotEqual
        | BinaryOp::Less
        | BinaryOp::LessEqual
        | BinaryOp::Greater
        | BinaryOp::GreaterEqual
        | BinaryOp::In
        | BinaryOp::InstanceOf
        | BinaryOp::LogicalAnd
        | BinaryOp::LogicalOr
        | BinaryOp::NullishCoalescing => Err(Error::runtime(
            "non-numeric operator reached BigInt binary owner",
        )),
    }
}

fn bigint_division_by_zero() -> Error {
    Error::exception(ErrorName::RangeError, BIGINT_DIVISION_BY_ZERO_ERROR)
}

fn bigint_multiply(left: &JsBigInt, right: &JsBigInt) -> JsBigInt {
    if left.is_zero() || right.is_zero() {
        return JsBigInt::zero();
    }
    left.mul(right)
}

fn bigint_pow(base: &JsBigInt, exponent: &JsBigInt, max_bits: usize) -> Result<JsBigInt> {
    if exponent.is_negative() {
        return Err(Error::exception(
            ErrorName::RangeError,
            BIGINT_NEGATIVE_EXPONENT_ERROR,
        ));
    }
    if exponent.is_zero() {
        return Ok(JsBigInt::from_u64(1));
    }
    if base.is_zero() {
        return Ok(JsBigInt::zero());
    }
    if base.is_one() {
        return Ok(base.clone());
    }
    if base.is_negative_one() {
        return Ok(if exponent.is_odd() {
            base.clone()
        } else {
            JsBigInt::from_u64(1)
        });
    }
    let exponent_u64 = exponent
        .to_u64()
        .ok_or_else(|| Error::limit("BigInt exponent exceeded supported resource range"))?;
    let minimum_result_bits = base
        .bit_len()
        .saturating_sub(1)
        .checked_mul(exponent_u64)
        .and_then(|bits| bits.checked_add(1))
        .ok_or_else(|| Error::limit("BigInt exponentiation size overflowed"))?;
    let max_bits = u64::try_from(max_bits)
        .map_err(|_| Error::limit("BigInt bit limit exceeded supported range"))?;
    if minimum_result_bits > max_bits {
        return Err(Error::limit(
            "BigInt exponentiation exceeded the configured bit limit",
        ));
    }
    let exponent = u32::try_from(exponent_u64)
        .map_err(|_| Error::limit("BigInt exponent exceeded supported resource range"))?;
    Ok(base.pow(exponent))
}

fn bigint_shift(
    value: &JsBigInt,
    count: &JsBigInt,
    left: bool,
    max_bits: usize,
) -> Result<JsBigInt> {
    let reverse = count.is_negative();
    let shifts_left = matches!((left, reverse), (true, false) | (false, true));
    let magnitude = count.abs().to_usize();
    if !shifts_left {
        return Ok(magnitude.map_or_else(
            || {
                if value.is_negative() {
                    JsBigInt::from_i64(-1)
                } else {
                    JsBigInt::zero()
                }
            },
            |magnitude| value.shift_right(magnitude),
        ));
    }
    if value.is_zero() {
        return Ok(JsBigInt::zero());
    }
    let magnitude = magnitude
        .ok_or_else(|| Error::limit("BigInt shift count exceeded supported resource range"))?;
    let result_bits = usize::try_from(value.bit_len())
        .ok()
        .and_then(|bits| bits.checked_add(magnitude))
        .ok_or_else(|| Error::limit("BigInt shift size overflowed"))?;
    if result_bits > max_bits {
        return Err(Error::limit(
            "BigInt shift exceeded the configured bit limit",
        ));
    }
    Ok(value.shift_left(magnitude))
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
