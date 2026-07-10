use crate::{
    error::Result,
    runtime::{Context, abstract_operations::PreferredType},
    syntax::BinaryOp,
    value::Value,
};

pub(super) fn relational_compare(
    context: &mut Context,
    op: BinaryOp,
    left: &Value,
    right: &Value,
) -> Result<Value> {
    let left = context.to_primitive(left, PreferredType::Number)?;
    let right = context.to_primitive(right, PreferredType::Number)?;
    let result = if let (Some(left), Some(right)) = (string_value(&left), string_value(&right)) {
        string_relational_compare(op, left, right)
    } else {
        number_relational_compare(op, context.to_number(&left)?, context.to_number(&right)?)
    };
    Ok(Value::Bool(result))
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
