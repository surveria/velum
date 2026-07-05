pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("lexer error at {offset}: {message}")]
    Lex { message: String, offset: usize },
    #[error("parser error at {offset}: {message}")]
    Parse { message: String, offset: usize },
    #[error("runtime error: {message}")]
    Runtime { message: String },
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
            Self::ResourceLimit { message } => Self::ResourceLimit {
                message: format!("{context}: {message}"),
            },
        }
    }
}
