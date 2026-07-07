use std::fmt;

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

impl fmt::Display for ErrorName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
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
