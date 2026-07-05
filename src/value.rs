use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FunctionId(usize);

impl FunctionId {
    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeFunctionId(usize);

impl NativeFunctionId {
    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectId(usize);

impl ObjectId {
    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }

    pub(crate) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorName {
    Base,
    EvalError,
    RangeError,
    ReferenceError,
    SyntaxError,
    Test262Error,
    TypeError,
    UriError,
}

impl ErrorName {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Base => "Error",
            Self::EvalError => "EvalError",
            Self::RangeError => "RangeError",
            Self::ReferenceError => "ReferenceError",
            Self::SyntaxError => "SyntaxError",
            Self::Test262Error => "Test262Error",
            Self::TypeError => "TypeError",
            Self::UriError => "URIError",
        }
    }

    pub(crate) fn from_constructor_name(name: &str) -> Option<Self> {
        match name {
            "Error" => Some(Self::Base),
            "EvalError" => Some(Self::EvalError),
            "RangeError" => Some(Self::RangeError),
            "ReferenceError" => Some(Self::ReferenceError),
            "SyntaxError" => Some(Self::SyntaxError),
            "Test262Error" => Some(Self::Test262Error),
            "TypeError" => Some(Self::TypeError),
            "URIError" => Some(Self::UriError),
            _ => None,
        }
    }

    pub(crate) const fn is_standard(self) -> bool {
        !matches!(self, Self::Test262Error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorObject {
    name: ErrorName,
    message: String,
}

impl ErrorObject {
    pub(crate) fn new(name: ErrorName, message: impl Into<String>) -> Self {
        Self {
            name,
            message: message.into(),
        }
    }

    pub(crate) const fn name(&self) -> ErrorName {
        self.name
    }

    pub(crate) const fn message(&self) -> &str {
        self.message.as_str()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Function(FunctionId),
    NativeFunction(NativeFunctionId),
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
            Self::Function(_) | Self::NativeFunction(_) | Self::Object(_) | Self::Error(_) => true,
        }
    }

    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Undefined => "undefined",
            Self::Null | Self::Object(_) | Self::Error(_) => "object",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Function(_) | Self::NativeFunction(_) => "function",
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
            _ => self.to_string(),
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
                if value.fract() == 0.0 && value.is_finite() {
                    write!(f, "{value:.0}")
                } else {
                    write!(f, "{value}")
                }
            }
            Self::String(value) => f.write_str(value),
            Self::Function(_) | Self::NativeFunction(_) => f.write_str("function()"),
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
