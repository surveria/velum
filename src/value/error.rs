use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorName {
    AggregateError,
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
            Self::AggregateError => "AggregateError",
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
            "AggregateError" => Some(Self::AggregateError),
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

    pub(crate) const fn constructor_length(self) -> f64 {
        if matches!(self, Self::AggregateError) {
            return 2.0;
        }
        1.0
    }
}

impl fmt::Display for ErrorName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
