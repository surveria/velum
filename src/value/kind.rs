use std::fmt;

use crate::storage::{string_heap::JsString, symbol::JsSymbol};

use super::{FunctionId, HostFunctionId, NativeFunctionId, ObjectId};

#[derive(Clone, Debug)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    HeapString(JsString),
    Symbol(JsSymbol),
    Function(FunctionId),
    NativeFunction(NativeFunctionId),
    HostFunction(HostFunctionId),
    Object(ObjectId),
}

impl Value {
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Undefined => "undefined",
            Self::Null | Self::Object(_) => "object",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) | Self::HeapString(_) => "string",
            Self::Symbol(_) => "symbol",
            Self::Function(_) | Self::NativeFunction(_) | Self::HostFunction(_) => "function",
        }
    }

    pub(crate) const fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Undefined, Self::Undefined) | (Self::Null, Self::Null) => true,
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Number(left), Self::Number(right)) => left == right,
            (Self::String(left), Self::String(right)) => left == right,
            (Self::HeapString(left), Self::HeapString(right)) => left == right,
            (Self::String(left), Self::HeapString(right)) => {
                left.encode_utf16().eq(right.as_utf16().iter().copied())
            }
            (Self::HeapString(left), Self::String(right)) => {
                right.encode_utf16().eq(left.as_utf16().iter().copied())
            }
            (Self::Symbol(left), Self::Symbol(right)) => left == right,
            (Self::Function(left), Self::Function(right)) => left == right,
            (Self::NativeFunction(left), Self::NativeFunction(right)) => left == right,
            (Self::HostFunction(left), Self::HostFunction(right)) => left == right,
            (Self::Object(left), Self::Object(right)) => left == right,
            (
                Self::Undefined
                | Self::Null
                | Self::Bool(_)
                | Self::Number(_)
                | Self::String(_)
                | Self::HeapString(_)
                | Self::Symbol(_)
                | Self::Function(_)
                | Self::NativeFunction(_)
                | Self::HostFunction(_)
                | Self::Object(_),
                _,
            ) => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Undefined => f.write_str("undefined"),
            Self::Null => f.write_str("null"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::Number(value) => f.write_str(&format_ecmascript_number(*value)),
            Self::String(value) => f.write_str(value),
            Self::HeapString(value) => f.write_str(value.as_str()),
            Self::Symbol(value) => f.write_str(&value.display_name()),
            Self::Function(_) | Self::NativeFunction(_) | Self::HostFunction(_) => {
                f.write_str("function()")
            }
            Self::Object(_) => f.write_str("[object Object]"),
        }
    }
}

const ECMASCRIPT_FIXED_EXPONENT_LIMIT: i32 = 21;
const ECMASCRIPT_SMALL_EXPONENT_LIMIT: i32 = -6;

/// ECMAScript `Number::toString` in base ten: shortest round-trip significant
/// digits rendered with the specification's fixed vs. exponential selection.
pub fn format_ecmascript_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value == f64::INFINITY {
        return "Infinity".to_owned();
    }
    if value == f64::NEG_INFINITY {
        return "-Infinity".to_owned();
    }
    if value == 0.0 {
        return "0".to_owned();
    }
    let negative = value < 0.0;
    let (digits, point) = shortest_significant_digits(value.abs());
    let body = render_ecmascript_digits(&digits, point);
    if negative { format!("-{body}") } else { body }
}

/// Extract the shortest round-trip significant digits of a finite positive
/// value and the base-ten position `n` of its decimal point, where
/// `value == digits x 10^(n - digit_count)` and the first digit has place
/// value `10^(n - 1)`.
fn shortest_significant_digits(value: f64) -> (Vec<u8>, i32) {
    let scientific = format!("{value:e}");
    let (mantissa, exponent) = scientific
        .split_once('e')
        .unwrap_or((scientific.as_str(), "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let mut digits: Vec<u8> = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|byte| byte - b'0')
        .collect();
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }
    (digits, exponent + 1)
}

fn render_ecmascript_digits(digits: &[u8], point: i32) -> String {
    let count = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    if (1..=ECMASCRIPT_FIXED_EXPONENT_LIMIT).contains(&point) {
        return render_fixed_positive(digits, point, count);
    }
    if point <= 0 && point > ECMASCRIPT_SMALL_EXPONENT_LIMIT {
        let mut out = String::from("0.");
        for _ in 0..(-point) {
            out.push('0');
        }
        push_ascii_digits(&mut out, digits);
        return out;
    }
    render_exponential(digits, point)
}

fn render_fixed_positive(digits: &[u8], point: i32, count: i32) -> String {
    let mut out = String::new();
    if count <= point {
        push_ascii_digits(&mut out, digits);
        for _ in 0..(point - count) {
            out.push('0');
        }
        return out;
    }
    let split = usize::try_from(point).unwrap_or(0);
    push_ascii_digits(&mut out, digits.get(..split).unwrap_or(&[]));
    out.push('.');
    push_ascii_digits(&mut out, digits.get(split..).unwrap_or(&[]));
    out
}

fn render_exponential(digits: &[u8], point: i32) -> String {
    let mut out = String::new();
    out.push(char::from(b'0' + digits.first().copied().unwrap_or(0)));
    if digits.len() > 1 {
        out.push('.');
        push_ascii_digits(&mut out, digits.get(1..).unwrap_or(&[]));
    }
    let exponent = point - 1;
    out.push('e');
    out.push(if exponent < 0 { '-' } else { '+' });
    out.push_str(&exponent.unsigned_abs().to_string());
    out
}

fn push_ascii_digits(out: &mut String, digits: &[u8]) {
    for digit in digits {
        out.push(char::from(b'0' + *digit));
    }
}
