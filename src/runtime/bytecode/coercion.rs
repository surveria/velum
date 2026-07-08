use crate::{error::Result, runtime::Context, syntax::BinaryOp, value::Value};

pub(super) fn abstract_equality(context: &Context, left: &Value, right: &Value) -> Result<bool> {
    if !matches!(right, Value::Object(_))
        && let Some(left) = string_object_primitive(context, left)?
    {
        return string_abstract_equality(context, left, right);
    }
    if !matches!(left, Value::Object(_))
        && let Some(right) = string_object_primitive(context, right)?
    {
        return string_abstract_equality(context, right, left);
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
            let left = Context::value_to_number(left);
            let right = Context::value_to_number(right);
            Ok(numbers_equal(left, right))
        }
        _ => Ok(left == right),
    }
}

pub(in crate::runtime::bytecode) fn strict_equality(left: &Value, right: &Value) -> bool {
    left == right
}

pub(super) fn relational_compare(op: BinaryOp, left: &Value, right: &Value) -> Value {
    let result = if let (Some(left), Some(right)) = (string_value(left), string_value(right)) {
        string_relational_compare(op, left, right)
    } else {
        number_relational_compare(
            op,
            Context::value_to_number(left),
            Context::value_to_number(right),
        )
    };
    Value::Bool(result)
}

fn string_object_primitive<'a>(context: &'a Context, value: &Value) -> Result<Option<&'a str>> {
    let Value::Object(id) = value else {
        return Ok(None);
    };
    context.string_object_primitive_value(*id)
}

fn string_abstract_equality(context: &Context, left: &str, right: &Value) -> Result<bool> {
    let result = match right {
        Value::Bool(value) => {
            numbers_equal(Context::string_to_number(left), bool_to_number(*value))
        }
        Value::Number(value) => numbers_equal(Context::string_to_number(left), *value),
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

fn string_value(value: &Value) -> Option<&str> {
    match value {
        Value::String(value) => Some(value.as_str()),
        Value::HeapString(value) => Some(value.as_str()),
        _ => None,
    }
}

fn string_relational_compare(op: BinaryOp, left: &str, right: &str) -> bool {
    match op {
        BinaryOp::Less => left < right,
        BinaryOp::LessEqual => left <= right,
        BinaryOp::Greater => left > right,
        BinaryOp::GreaterEqual => left >= right,
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Rem
        | BinaryOp::Pow
        | BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::StrictEqual
        | BinaryOp::StrictNotEqual
        | BinaryOp::In
        | BinaryOp::InstanceOf
        | BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight
        | BinaryOp::ShiftRightUnsigned
        | BinaryOp::LogicalAnd
        | BinaryOp::LogicalOr
        | BinaryOp::NullishCoalescing => false,
    }
}

fn number_relational_compare(op: BinaryOp, left: f64, right: f64) -> bool {
    match op {
        BinaryOp::Less => left < right,
        BinaryOp::LessEqual => left <= right,
        BinaryOp::Greater => left > right,
        BinaryOp::GreaterEqual => left >= right,
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Rem
        | BinaryOp::Pow
        | BinaryOp::Equal
        | BinaryOp::NotEqual
        | BinaryOp::StrictEqual
        | BinaryOp::StrictNotEqual
        | BinaryOp::In
        | BinaryOp::InstanceOf
        | BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::BitXor
        | BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight
        | BinaryOp::ShiftRightUnsigned
        | BinaryOp::LogicalAnd
        | BinaryOp::LogicalOr
        | BinaryOp::NullishCoalescing => false,
    }
}

fn bool_to_number(value: bool) -> f64 {
    f64::from(u8::from(value))
}

fn numbers_equal(left: f64, right: f64) -> bool {
    left.partial_cmp(&right)
        .is_some_and(std::cmp::Ordering::is_eq)
}
