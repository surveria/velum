use crate::runtime::Context;
use crate::runtime_limits::RuntimeLimits;

#[derive(Debug, Clone)]
pub struct Runtime {
    limits: RuntimeLimits,
}

impl Runtime {
    #[must_use]
    pub fn new() -> Self {
        Self {
            limits: RuntimeLimits::default(),
        }
    }

    #[must_use]
    pub const fn with_limits(limits: RuntimeLimits) -> Self {
        Self { limits }
    }

    #[must_use]
    pub const fn limits(&self) -> RuntimeLimits {
        self.limits
    }

    #[must_use]
    pub const fn context(&self) -> Context {
        Context::new(self.limits)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
