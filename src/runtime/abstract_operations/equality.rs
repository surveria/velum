use crate::{error::Result, runtime::Context, value::Value};

/// ECMAScript Strict Equality Comparison.
pub(in crate::runtime) fn strict_equality(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => number_strict_equality(*left, *right),
        _ => left == right,
    }
}

/// ECMAScript Abstract Equality Comparison over the conversions currently
/// supported by the runtime. AS-03a2 will replace the boxed-string-specific
/// conversion with the shared `ToPrimitive` operation.
pub(in crate::runtime) fn abstract_equality(
    context: &Context,
    left: &Value,
    right: &Value,
) -> Result<bool> {
    if !matches!(right, Value::Object(_))
        && let Some(left) = string_object_primitive(context, left)?
    {
        return compare_string_primitive(context, left, right);
    }
    if !matches!(left, Value::Object(_))
        && let Some(right) = string_object_primitive(context, right)?
    {
        return compare_string_primitive(context, right, left);
    }

    match (left, right) {
        (Value::Undefined, Value::Null) | (Value::Null, Value::Undefined) => Ok(true),
        (Value::Bool(left), _) => {
            abstract_equality(context, &Value::Number(bool_to_number(*left)), right)
        }
        (_, Value::Bool(right)) => {
            abstract_equality(context, left, &Value::Number(bool_to_number(*right)))
        }
        (Value::String(_) | Value::HeapString(_), Value::Number(_))
        | (Value::Number(_), Value::String(_) | Value::HeapString(_)) => {
            Ok(number_strict_equality(
                Context::value_to_number(left),
                Context::value_to_number(right),
            ))
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

fn string_object_primitive<'a>(context: &'a Context, value: &Value) -> Result<Option<&'a str>> {
    let Value::Object(id) = value else {
        return Ok(None);
    };
    context.string_object_primitive_value(*id)
}

fn compare_string_primitive(context: &Context, left: &str, right: &Value) -> Result<bool> {
    let result = match right {
        Value::Bool(value) => {
            number_strict_equality(Context::string_to_number(left), bool_to_number(*value))
        }
        Value::Number(value) => number_strict_equality(Context::string_to_number(left), *value),
        Value::String(value) => left == value,
        Value::HeapString(value) => left == value.as_str(),
        Value::Object(_) => {
            let Some(right) = string_object_primitive(context, right)? else {
                return Ok(false);
            };
            left == right
        }
        Value::Undefined
        | Value::Null
        | Value::Symbol(_)
        | Value::Function(_)
        | Value::NativeFunction(_)
        | Value::HostFunction(_)
        | Value::Error(_) => false,
    };
    Ok(result)
}

const fn bool_to_number(value: bool) -> f64 {
    if value { 1.0 } else { 0.0 }
}

const fn number_is_zero(value: f64) -> bool {
    value.to_bits() << 1 == 0
}
