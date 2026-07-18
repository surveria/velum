use core::fmt;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GeneratorError {
    message: String,
}

impl GeneratorError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GeneratorError {}
