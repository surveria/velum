use std::cmp::Ordering;

use crate::{
    error::Result,
    runtime::{
        Context,
        abstract_operations::{NumericValue, PreferredType},
    },
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
        match bigint_string_ordering(&left, &right) {
            BigIntStringOrdering::Ordered(ordering) => ordering_matches(op, ordering),
            BigIntStringOrdering::Unordered => false,
            BigIntStringOrdering::NotApplicable => numeric_relational_compare(
                op,
                context.to_numeric(&left)?,
                context.to_numeric(&right)?,
            ),
        }
    };
    Ok(Value::Bool(result))
}

enum BigIntStringOrdering {
    NotApplicable,
    Unordered,
    Ordered(Ordering),
}

fn bigint_string_ordering(left: &Value, right: &Value) -> BigIntStringOrdering {
    let ordering = match (left, right) {
        (Value::BigInt(left), right) if right.is_string() => right
            .string_text()
            .and_then(crate::value::JsBigInt::parse_string)
            .map(|right| left.cmp(&right)),
        (left, Value::BigInt(right)) if left.is_string() => left
            .string_text()
            .and_then(crate::value::JsBigInt::parse_string)
            .map(|left| left.cmp(right)),
        _ => return BigIntStringOrdering::NotApplicable,
    };
    ordering.map_or(
        BigIntStringOrdering::Unordered,
        BigIntStringOrdering::Ordered,
    )
}

fn string_value(value: &Value) -> Option<&str> {
    value.string_text()
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

fn numeric_relational_compare(op: BinaryOp, left: NumericValue, right: NumericValue) -> bool {
    let ordering = match (left, right) {
        (NumericValue::Number(left), NumericValue::Number(right)) => {
            return number_relational_compare(op, left, right);
        }
        (NumericValue::BigInt(left), NumericValue::BigInt(right)) => Some(left.cmp(&right)),
        (NumericValue::BigInt(left), NumericValue::Number(right)) => left.compare_number(right),
        (NumericValue::Number(left), NumericValue::BigInt(right)) => {
            right.compare_number(left).map(Ordering::reverse)
        }
    };
    ordering.is_some_and(|ordering| ordering_matches(op, ordering))
}

fn ordering_matches(op: BinaryOp, ordering: Ordering) -> bool {
    match op {
        BinaryOp::Less => ordering == Ordering::Less,
        BinaryOp::LessEqual => ordering != Ordering::Greater,
        BinaryOp::Greater => ordering == Ordering::Greater,
        BinaryOp::GreaterEqual => ordering != Ordering::Less,
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
