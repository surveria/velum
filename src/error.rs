use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Lex { message: String, offset: usize },
    Parse { message: String, offset: usize },
    Runtime { message: String },
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lex { message, offset } => write!(f, "lexer error at {offset}: {message}"),
            Self::Parse { message, offset } => write!(f, "parser error at {offset}: {message}"),
            Self::Runtime { message } => write!(f, "runtime error: {message}"),
            Self::ResourceLimit { message } => write!(f, "resource limit exceeded: {message}"),
        }
    }
}

impl std::error::Error for Error {}
