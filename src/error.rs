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

    pub(crate) fn runtime(message: impl Into<String>) -> Self {
        Self::Runtime {
            message: message.into(),
        }
    }

    pub(crate) fn limit(message: impl Into<String>) -> Self {
        Self::ResourceLimit {
            message: message.into(),
        }
    }
}
