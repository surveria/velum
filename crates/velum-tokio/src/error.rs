use core::fmt;

/// Failure reported by a Tokio-owned Velum VM.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RuntimeError {
    /// A worker or command-queue setting is invalid.
    InvalidConfiguration(&'static str),
    /// A worker thread or its Tokio runtime could not be started.
    WorkerStart(String),
    /// The Velum operation failed inside the owning worker.
    Engine(String),
    /// The runtime stopped before it could accept the operation.
    RuntimeClosed,
    /// The owning VM stopped before it could accept the operation.
    VmClosed,
    /// The owning worker disappeared before returning the operation result.
    ResponseDropped,
}

impl RuntimeError {
    pub(crate) fn engine(error: &velum::Error) -> Self {
        Self::Engine(error.to_string())
    }
}

impl From<velum::HostFutureError> for RuntimeError {
    fn from(error: velum::HostFutureError) -> Self {
        Self::Engine(error.to_string())
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) => {
                write!(
                    formatter,
                    "invalid Tokio VM runtime configuration: {message}"
                )
            }
            Self::WorkerStart(message) => {
                write!(formatter, "failed to start Tokio VM worker: {message}")
            }
            Self::Engine(message) => write!(formatter, "Velum engine operation failed: {message}"),
            Self::RuntimeClosed => formatter.write_str("Tokio VM runtime is closed"),
            Self::VmClosed => formatter.write_str("Tokio-owned VM is closed"),
            Self::ResponseDropped => {
                formatter.write_str("Tokio VM worker dropped the operation response")
            }
        }
    }
}

impl std::error::Error for RuntimeError {}
