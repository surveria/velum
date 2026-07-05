use crate::error::{Error, Result};
use crate::value::Value;

const TO_INT32_MODULUS: f64 = 4_294_967_296.0;
const TO_INT32_SIGN_BOUNDARY: u64 = 2_147_483_648;
const TO_INT32_SIGN_OFFSET: i64 = 4_294_967_296;

pub fn numeric_binary(
    left: &Value,
    right: &Value,
    op: &str,
    apply: impl FnOnce(f64, f64) -> f64,
) -> Result<Value> {
    let Some(left) = left.as_number() else {
        return Err(Error::runtime(format!("operator '{op}' expects numbers")));
    };
    let Some(right) = right.as_number() else {
        return Err(Error::runtime(format!("operator '{op}' expects numbers")));
    };
    Ok(Value::Number(apply(left, right)))
}

pub fn compare_binary(
    left: &Value,
    right: &Value,
    op: &str,
    apply: impl FnOnce(f64, f64) -> bool,
) -> Result<Value> {
    let Some(left) = left.as_number() else {
        return Err(Error::runtime(format!("operator '{op}' expects numbers")));
    };
    let Some(right) = right.as_number() else {
        return Err(Error::runtime(format!("operator '{op}' expects numbers")));
    };
    Ok(Value::Bool(apply(left, right)))
}

pub fn bitwise_and(left: &Value, right: &Value) -> Result<Value> {
    let left = bitwise_i32(left)?;
    let right = bitwise_i32(right)?;
    Ok(Value::Number(f64::from(left & right)))
}

fn bitwise_i32(value: &Value) -> Result<i32> {
    match value {
        Value::Undefined
        | Value::Null
        | Value::Function(_)
        | Value::Object(_)
        | Value::Error(_) => Ok(0),
        Value::Bool(value) => Ok(i32::from(*value)),
        Value::Number(value) => number_to_i32(*value),
        Value::String(value) => string_to_i32(value),
    }
}

fn string_to_i32(value: &str) -> Result<i32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }
    let Ok(value) = trimmed.parse::<f64>() else {
        return Ok(0);
    };
    number_to_i32(value)
}

fn number_to_i32(value: f64) -> Result<i32> {
    if !value.is_finite() || value == 0.0 {
        return Ok(0);
    }

    let truncated = if value.is_sign_negative() {
        value.ceil()
    } else {
        value.floor()
    };
    let modulo = truncated.rem_euclid(TO_INT32_MODULUS);
    let unsigned = format!("{modulo:.0}")
        .parse::<u64>()
        .map_err(|_| Error::runtime("bitwise '&' failed to convert number to uint32"))?;
    let signed = if unsigned >= TO_INT32_SIGN_BOUNDARY {
        let unsigned = i64::try_from(unsigned)
            .map_err(|_| Error::runtime("bitwise '&' uint32 conversion overflowed"))?;
        unsigned
            .checked_sub(TO_INT32_SIGN_OFFSET)
            .ok_or_else(|| Error::runtime("bitwise '&' int32 conversion overflowed"))?
    } else {
        i64::try_from(unsigned)
            .map_err(|_| Error::runtime("bitwise '&' uint32 conversion overflowed"))?
    };

    i32::try_from(signed)
        .map_err(|_| Error::runtime("bitwise '&' failed to convert number to int32"))
}
