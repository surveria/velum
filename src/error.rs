use std::fmt;

use crate::value::{ErrorName, Value};

pub type Result<T> = std::result::Result<T, Error>;

/// Stable diagnostic metadata for a built-in JavaScript Error instance.
///
/// The metadata identifies the built-in error class without using formatted
/// message text. The thrown [`Value`] remains the source of JavaScript object
/// identity and is valid only in its owning VM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaScriptErrorMetadata {
    name: ErrorName,
    message: String,
}

impl JavaScriptErrorMetadata {
    pub(crate) fn new(name: ErrorName, message: impl Into<String>) -> Self {
        Self {
            name,
            message: message.into(),
        }
    }

    pub(crate) const fn error_name(&self) -> ErrorName {
        self.name
    }

    /// Returns the built-in Error constructor name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name.as_str()
    }

    /// Returns the diagnostic message captured when the Error was created.
    #[must_use]
    pub const fn message(&self) -> &str {
        self.message.as_str()
    }
}

impl fmt::Display for JavaScriptErrorMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.message.is_empty() {
            return formatter.write_str(self.name());
        }
        write!(formatter, "{}: {}", self.name(), self.message)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("lexer error at {offset}: {message}")]
    Lex { message: String, offset: usize },
    #[error("parser error at {offset}: {message}")]
    Parse { message: String, offset: usize },
    #[error("runtime error: {message}")]
    Runtime { message: String },
    /// An arbitrary JavaScript thrown value owned by the VM that produced it.
    #[error("javascript exception: {display}")]
    JavaScript {
        value: Value,
        metadata: Option<JavaScriptErrorMetadata>,
        display: String,
    },
    /// A typed built-in exception request awaiting allocation in the active VM.
    /// This internal form must be converted to a real JavaScript object before
    /// an error crosses the public embedding boundary.
    #[doc(hidden)]
    #[error("javascript exception: {metadata}")]
    JavaScriptError { metadata: JavaScriptErrorMetadata },
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
        Self::JavaScriptError {
            metadata: JavaScriptErrorMetadata::new(name, message),
        }
    }

    /// Preserves an arbitrary value thrown by JavaScript across an engine or
    /// host `Result` boundary. VM-owned values are valid only for the VM whose
    /// active call supplied them.
    #[must_use]
    pub fn javascript(value: Value) -> Self {
        let display = value.to_string();
        Self::JavaScript {
            value,
            metadata: None,
            display,
        }
    }

    pub(crate) fn javascript_with_metadata(
        value: Value,
        metadata: Option<JavaScriptErrorMetadata>,
    ) -> Self {
        let display = metadata
            .as_ref()
            .map_or_else(|| value.to_string(), ToString::to_string);
        Self::JavaScript {
            value,
            metadata,
            display,
        }
    }

    /// Returns the original JavaScript value when this error represents a
    /// thrown completion. The returned value may contain VM-owned handles and
    /// must not be used with another VM.
    #[must_use]
    pub const fn javascript_value(&self) -> Option<&Value> {
        let Self::JavaScript { value, .. } = self else {
            return None;
        };
        Some(value)
    }

    /// Returns structured metadata for a built-in JavaScript Error instance.
    #[must_use]
    pub const fn javascript_error_metadata(&self) -> Option<&JavaScriptErrorMetadata> {
        let Self::JavaScript { metadata, .. } = self else {
            return None;
        };
        metadata.as_ref()
    }

    /// Returns the standard error name when the thrown value is a built-in
    /// Error instance.
    #[must_use]
    pub const fn javascript_error_name(&self) -> Option<&'static str> {
        let Some(metadata) = self.javascript_error_metadata() else {
            return None;
        };
        Some(metadata.name())
    }

    /// Returns the diagnostic message captured for a built-in Error instance.
    #[must_use]
    pub const fn javascript_error_message(&self) -> Option<&str> {
        let Some(metadata) = self.javascript_error_metadata() else {
            return None;
        };
        Some(metadata.message())
    }

    pub(crate) const fn javascript_error_request(&self) -> Option<&JavaScriptErrorMetadata> {
        let Self::JavaScriptError { metadata } = self else {
            return None;
        };
        Some(metadata)
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
            Self::JavaScript {
                value,
                metadata,
                display,
            } => Self::JavaScript {
                value,
                metadata,
                display,
            },
            Self::JavaScriptError { metadata } => Self::JavaScriptError { metadata },
            Self::ResourceLimit { message } => Self::ResourceLimit {
                message: format!("{context}: {message}"),
            },
        }
    }
}
