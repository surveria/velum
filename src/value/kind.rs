use std::fmt;

use crate::storage::string_heap::JsString;

use super::{ErrorObject, FunctionId, HostFunctionId, NativeFunctionId, ObjectId};

#[derive(Clone, Debug)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    HeapString(JsString),
    Function(FunctionId),
    NativeFunction(NativeFunctionId),
    HostFunction(HostFunctionId),
    Object(ObjectId),
    Error(ErrorObject),
}

impl Value {
    #[must_use]
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Bool(value) => *value,
            Self::Number(value) => *value != 0.0 && !value.is_nan(),
            Self::String(value) => !value.is_empty(),
            Self::HeapString(value) => !value.as_str().is_empty(),
            Self::Function(_)
            | Self::NativeFunction(_)
            | Self::HostFunction(_)
            | Self::Object(_)
            | Self::Error(_) => true,
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Undefined => "undefined",
            Self::Null | Self::Object(_) | Self::Error(_) => "object",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) | Self::HeapString(_) => "string",
            Self::Function(_) | Self::NativeFunction(_) | Self::HostFunction(_) => "function",
        }
    }

    pub(crate) const fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub(crate) fn display_for_concat(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::HeapString(value) => value.as_str().to_owned(),
            _ => self.to_string(),
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
            (Self::String(left), Self::HeapString(right)) => left == right.as_str(),
            (Self::HeapString(left), Self::String(right)) => left.as_str() == right,
            (Self::Function(left), Self::Function(right)) => left == right,
            (Self::NativeFunction(left), Self::NativeFunction(right)) => left == right,
            (Self::HostFunction(left), Self::HostFunction(right)) => left == right,
            (Self::Object(left), Self::Object(right)) => left == right,
            (Self::Error(left), Self::Error(right)) => left == right,
            (
                Self::Undefined
                | Self::Null
                | Self::Bool(_)
                | Self::Number(_)
                | Self::String(_)
                | Self::HeapString(_)
                | Self::Function(_)
                | Self::NativeFunction(_)
                | Self::HostFunction(_)
                | Self::Object(_)
                | Self::Error(_),
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
            Self::Number(value) => {
                if value.is_nan() {
                    f.write_str("NaN")
                } else if *value == f64::INFINITY {
                    f.write_str("Infinity")
                } else if *value == f64::NEG_INFINITY {
                    f.write_str("-Infinity")
                } else if value.fract() == 0.0 && value.is_finite() {
                    write!(f, "{value:.0}")
                } else {
                    write!(f, "{value}")
                }
            }
            Self::String(value) => f.write_str(value),
            Self::HeapString(value) => f.write_str(value.as_str()),
            Self::Function(_) | Self::NativeFunction(_) | Self::HostFunction(_) => {
                f.write_str("function()")
            }
            Self::Object(_) => f.write_str("[object Object]"),
            Self::Error(error) => {
                if error.message().is_empty() {
                    f.write_str(error.name().as_str())
                } else {
                    write!(f, "{}: {}", error.name().as_str(), error.message())
                }
            }
        }
    }
}
