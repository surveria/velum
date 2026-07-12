use crate::{
    error::Result,
    runtime::{
        Context,
        abstract_operations::{PreferredType, is_primitive},
    },
    value::Value,
};

/// ECMAScript Strict Equality Comparison.
pub(in crate::runtime) fn strict_equality(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => number_strict_equality(*left, *right),
        _ => left == right,
    }
}

/// ECMAScript Abstract Equality Comparison.
pub(in crate::runtime) fn abstract_equality(
    context: &mut Context,
    left: &Value,
    right: &Value,
) -> Result<bool> {
    match (left, right) {
        (Value::Undefined, Value::Null) | (Value::Null, Value::Undefined) => Ok(true),
        (Value::Bool(left), _) => {
            abstract_equality(context, &Value::Number(bool_to_number(*left)), right)
        }
        (_, Value::Bool(right)) => {
            abstract_equality(context, left, &Value::Number(bool_to_number(*right)))
        }
        (Value::String(_) | Value::HeapString(_), Value::Number(_))
        | (Value::Number(_), Value::String(_) | Value::HeapString(_)) => Ok(
            number_strict_equality(context.to_number(left)?, context.to_number(right)?),
        ),
        (Value::BigInt(left), Value::Number(right)) => Ok(left.equals_number(*right)),
        (Value::Number(left), Value::BigInt(right)) => Ok(right.equals_number(*left)),
        (Value::BigInt(left), right) if right.is_string() => Ok(right
            .string_text()
            .and_then(crate::value::JsBigInt::parse_string)
            .is_some_and(|right| &right == left)),
        (left, Value::BigInt(right)) if left.is_string() => Ok(left
            .string_text()
            .and_then(crate::value::JsBigInt::parse_string)
            .is_some_and(|left| &left == right)),
        (left, right) if !is_primitive(left) && is_primitive(right) => {
            let primitive = context.to_primitive(left, PreferredType::Default)?;
            abstract_equality(context, &primitive, right)
        }
        (left, right) if is_primitive(left) && !is_primitive(right) => {
            let primitive = context.to_primitive(right, PreferredType::Default)?;
            abstract_equality(context, left, &primitive)
        }
        _ => Ok(strict_equality(left, right)),
    }
}

/// ECMAScript `SameValue` comparison used by `Object.is`.
pub(in crate::runtime) fn same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => number_same_value(*left, *right),
        _ => left == right,
    }
}

/// ECMAScript `SameValueZero` comparison used by collections and `includes`.
pub(in crate::runtime) fn same_value_zero(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => number_same_value_zero(*left, *right),
        _ => left == right,
    }
}

pub(in crate::runtime) const fn number_strict_equality(left: f64, right: f64) -> bool {
    if left.is_nan() || right.is_nan() {
        return false;
    }
    if number_is_zero(left) && number_is_zero(right) {
        return true;
    }
    left.to_bits() == right.to_bits()
}

pub(in crate::runtime) const fn number_same_value(left: f64, right: f64) -> bool {
    if left.is_nan() && right.is_nan() {
        return true;
    }
    left.to_bits() == right.to_bits()
}

pub(in crate::runtime) const fn number_same_value_zero(left: f64, right: f64) -> bool {
    if left.is_nan() && right.is_nan() {
        return true;
    }
    if number_is_zero(left) && number_is_zero(right) {
        return true;
    }
    left.to_bits() == right.to_bits()
}

const fn bool_to_number(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

const fn number_is_zero(value: f64) -> bool {
    value.to_bits() << 1 == 0
}
