use crate::value::{ErrorName, ErrorObject, Value};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("lexer error at {offset}: {message}")]
    Lex { message: String, offset: usize },
    #[error("parser error at {offset}: {message}")]
    Parse { message: String, offset: usize },
    #[error("runtime error: {message}")]
    Runtime { message: String },
    /// An arbitrary JavaScript thrown value owned by the VM that produced it.
    #[error("javascript exception: {value}")]
    JavaScript { value: Value },
    #[error("resource limit exceeded: {message}")]
    ResourceLimit { message: String },
}

impl Error {
    pub(crate) fn lex(message: impl Into<String>, offset: usize) -> Self {
        Self::Lex {
            message: message.into(),
            offset,
        }
    }

    pub(crate) fn parse(message: impl Into<String>, offset: usize) -> Self {
        Self::Parse {
            message: message.into(),
            offset,
        }
    }

    #[must_use]
    pub fn runtime(message: impl Into<String>) -> Self {
        Self::Runtime {
            message: message.into(),
        }
    }

    #[must_use]
    pub(crate) fn exception(name: ErrorName, message: impl Into<String>) -> Self {
        Self::JavaScript {
            value: Value::Error(ErrorObject::new(name, message.into())),
        }
    }

    /// Preserves an arbitrary value thrown by JavaScript across an engine or
    /// host `Result` boundary. VM-owned values are valid only for the VM whose
    /// active call supplied them.
    #[must_use]
    pub const fn javascript(value: Value) -> Self {
        Self::JavaScript { value }
    }

    /// Returns the original JavaScript value when this error represents a
    /// thrown completion. The returned value may contain VM-owned handles and
    /// must not be used with another VM.
    #[must_use]
    pub const fn javascript_value(&self) -> Option<&Value> {
        let Self::JavaScript { value } = self else {
            return None;
        };
        Some(value)
    }

    /// Returns the standard error name when the thrown value uses the current
    /// built-in Error representation.
    #[must_use]
    pub const fn javascript_error_name(&self) -> Option<&'static str> {
        let Some(Value::Error(error)) = self.javascript_value() else {
            return None;
        };
        Some(error.name().as_str())
    }

    /// Returns the standard error message when the thrown value uses the
    /// current built-in Error representation.
    #[must_use]
    pub const fn javascript_error_message(&self) -> Option<&str> {
        let Some(Value::Error(error)) = self.javascript_value() else {
            return None;
        };
        Some(error.message())
    }

    #[must_use]
    pub(crate) fn type_error(message: impl Into<String>) -> Self {
        Self::exception(ErrorName::TypeError, message)
    }

    #[must_use]
    pub fn limit(message: impl Into<String>) -> Self {
        Self::ResourceLimit {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn with_context(self, context: impl AsRef<str>) -> Self {
        let context = context.as_ref();
        match self {
            Self::Lex { message, offset } => Self::Lex {
                message: format!("{context}: {message}"),
                offset,
            },
            Self::Parse { message, offset } => Self::Parse {
                message: format!("{context}: {message}"),
                offset,
            },
            Self::Runtime { message } => Self::Runtime {
                message: format!("{context}: {message}"),
            },
            Self::JavaScript { value } => Self::JavaScript { value },
            Self::ResourceLimit { message } => Self::ResourceLimit {
                message: format!("{context}: {message}"),
            },
        }
    }
}
