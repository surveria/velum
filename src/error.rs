#[cfg(not(feature = "std"))]
use crate::prelude::*;

use core::fmt;

use crate::ownership::VmIdentity;
use crate::value::{ErrorName, Value};
use crate::{SourceId, SourceSpan};

pub type Result<T> = core::result::Result<T, Error>;

/// Stable diagnostic metadata for a built-in JavaScript Error instance.
///
/// The metadata identifies the built-in error class without using formatted
/// message text. The thrown [`Value`] remains the source of JavaScript object
/// identity and is valid only in its owning VM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaScriptErrorMetadata {
    name: ErrorName,
    message: String,
    span: Option<Box<SourceSpan>>,
}

impl JavaScriptErrorMetadata {
    pub(crate) fn new(name: ErrorName, message: impl Into<String>) -> Self {
        Self {
            name,
            message: message.into(),
            span: None,
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

    /// Returns the source range where this error originated, when known.
    #[must_use]
    pub fn source_span(&self) -> Option<SourceSpan> {
        self.span.as_deref().copied()
    }

    pub(crate) fn set_source_span_if_missing(&mut self, span: SourceSpan) {
        if self.span.is_none() {
            self.span = Some(Box::new(span));
        }
    }

    fn with_source_span(mut self, span: SourceSpan) -> Self {
        self.set_source_span_if_missing(span);
        self
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

/// Opaque payload of an arbitrary JavaScript thrown completion.
///
/// The fields stay private so embedders cannot forge an unowned VM-local
/// value by constructing [`Error::JavaScript`] directly.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub struct JavaScriptException {
    identity: Option<VmIdentity>,
    value: Value,
    metadata: Option<Box<JavaScriptErrorMetadata>>,
    display: Box<str>,
    span: Option<Box<SourceSpan>>,
}

impl fmt::Display for JavaScriptException {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.display)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("lexer error at {}: {message}", span.start())]
    Lex { message: String, span: SourceSpan },
    #[error("parser error at {}: {message}", span.start())]
    Parse { message: String, span: SourceSpan },
    #[error("runtime error: {message}")]
    Runtime {
        message: String,
        span: Option<SourceSpan>,
    },
    /// An arbitrary JavaScript thrown value owned by the VM that produced it.
    #[error("javascript exception: {exception}")]
    JavaScript { exception: Box<JavaScriptException> },
    /// A typed built-in exception request awaiting allocation in the active VM.
    /// This internal form must be converted to a real JavaScript object before
    /// an error crosses the public embedding boundary.
    #[doc(hidden)]
    #[error("javascript exception: {metadata}")]
    JavaScriptError { metadata: JavaScriptErrorMetadata },
    #[error("resource limit exceeded: {message}")]
    ResourceLimit {
        message: String,
        span: Option<SourceSpan>,
    },
}

impl Error {
    pub(crate) fn lex(message: impl Into<String>, offset: usize) -> Self {
        Self::Lex {
            message: message.into(),
            span: SourceSpan::point(SourceId::UNKNOWN, offset),
        }
    }

    pub(crate) fn parse(message: impl Into<String>, offset: usize) -> Self {
        Self::Parse {
            message: message.into(),
            span: SourceSpan::point(SourceId::UNKNOWN, offset),
        }
    }

    pub(crate) fn parse_at(message: impl Into<String>, span: SourceSpan) -> Self {
        Self::Parse {
            message: message.into(),
            span,
        }
    }

    #[must_use]
    pub fn runtime(message: impl Into<String>) -> Self {
        Self::Runtime {
            message: message.into(),
            span: None,
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
    pub(crate) fn javascript(value: Value) -> Self {
        Self::javascript_with_optional_identity(None, value)
    }

    pub(crate) fn javascript_local(identity: VmIdentity, value: Value) -> Self {
        Self::javascript_with_optional_identity(Some(identity), value)
    }

    fn javascript_with_optional_identity(identity: Option<VmIdentity>, value: Value) -> Self {
        let display = value.to_string().into_boxed_str();
        Self::JavaScript {
            exception: Box::new(JavaScriptException {
                identity,
                value,
                metadata: None,
                display,
                span: None,
            }),
        }
    }

    pub(crate) fn javascript_with_metadata(
        identity: VmIdentity,
        value: Value,
        metadata: Option<JavaScriptErrorMetadata>,
        fallback_span: Option<SourceSpan>,
    ) -> Self {
        let display = metadata
            .as_ref()
            .map_or_else(|| value.to_string(), ToString::to_string)
            .into_boxed_str();
        let span = metadata
            .as_ref()
            .and_then(JavaScriptErrorMetadata::source_span)
            .or(fallback_span);
        Self::JavaScript {
            exception: Box::new(JavaScriptException {
                identity: Some(identity),
                value,
                metadata: metadata.map(Box::new),
                display,
                span: span.map(Box::new),
            }),
        }
    }

    /// Returns the original JavaScript value when this error represents a
    /// thrown completion. The returned value may contain VM-owned handles and
    /// must not be used with another VM.
    #[must_use]
    pub const fn javascript_value(&self) -> Option<&Value> {
        let Self::JavaScript { exception } = self else {
            return None;
        };
        Some(&exception.value)
    }

    /// Returns the VM owner of an arbitrary JavaScript thrown value.
    #[must_use]
    pub fn javascript_identity(&self) -> Option<&VmIdentity> {
        let Self::JavaScript { exception } = self else {
            return None;
        };
        exception.identity.as_ref()
    }

    /// Returns structured metadata for a built-in JavaScript Error instance.
    #[must_use]
    pub fn javascript_error_metadata(&self) -> Option<&JavaScriptErrorMetadata> {
        let Self::JavaScript { exception } = self else {
            return None;
        };
        exception.metadata.as_deref()
    }

    /// Returns the standard error name when the thrown value is a built-in
    /// Error instance.
    #[must_use]
    pub fn javascript_error_name(&self) -> Option<&'static str> {
        let metadata = self.javascript_error_metadata()?;
        Some(metadata.name())
    }

    /// Returns the diagnostic message captured for a built-in Error instance.
    #[must_use]
    pub fn javascript_error_message(&self) -> Option<&str> {
        let metadata = self.javascript_error_metadata()?;
        Some(metadata.message())
    }

    /// Returns the source range for a frontend or runtime diagnostic.
    #[must_use]
    pub fn source_span(&self) -> Option<SourceSpan> {
        match self {
            Self::Lex { span, .. } | Self::Parse { span, .. } => Some(*span),
            Self::Runtime { span, .. } | Self::ResourceLimit { span, .. } => *span,
            Self::JavaScript { exception } => exception.span.as_deref().copied(),
            Self::JavaScriptError { metadata } => metadata.source_span(),
        }
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
            span: None,
        }
    }

    #[must_use]
    pub fn with_context(self, context: impl AsRef<str>) -> Self {
        let context = context.as_ref();
        match self {
            Self::Lex { message, span } => Self::Lex {
                message: format!("{context}: {message}"),
                span,
            },
            Self::Parse { message, span } => Self::Parse {
                message: format!("{context}: {message}"),
                span,
            },
            Self::Runtime { message, span } => Self::Runtime {
                message: format!("{context}: {message}"),
                span,
            },
            Self::JavaScript { exception } => Self::JavaScript { exception },
            Self::JavaScriptError { metadata } => Self::JavaScriptError { metadata },
            Self::ResourceLimit { message, span } => Self::ResourceLimit {
                message: format!("{context}: {message}"),
                span,
            },
        }
    }

    pub(crate) fn with_source(self, source_id: SourceId, source: &str) -> Self {
        match self {
            Self::Lex { message, span } => Self::Lex {
                message,
                span: rebind_source_span(span, source_id, source),
            },
            Self::Parse { message, span } => Self::Parse {
                message,
                span: rebind_source_span(span, source_id, source),
            },
            error => error,
        }
    }

    pub(crate) fn with_runtime_span(self, span: SourceSpan) -> Self {
        match self {
            Self::Runtime {
                message,
                span: existing,
            } => Self::Runtime {
                message,
                span: existing.or(Some(span)),
            },
            Self::JavaScript { mut exception } => {
                if exception.span.is_none() {
                    exception.span = Some(Box::new(span));
                }
                Self::JavaScript { exception }
            }
            Self::JavaScriptError { metadata } => Self::JavaScriptError {
                metadata: metadata.with_source_span(span),
            },
            Self::ResourceLimit {
                message,
                span: existing,
            } => Self::ResourceLimit {
                message,
                span: existing.or(Some(span)),
            },
            error @ (Self::Lex { .. } | Self::Parse { .. }) => error,
        }
    }
}

fn rebind_source_span(span: SourceSpan, source_id: SourceId, source: &str) -> SourceSpan {
    if span.source_id() != SourceId::UNKNOWN {
        return span;
    }
    SourceSpan::for_diagnostic(source_id, source, span.start())
}
