use crate::compiled_script::CompiledScript;
use crate::error::Result;
use crate::runtime::Context;
use crate::runtime::limits::RuntimeLimits;

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
    pub fn context(&self) -> Context {
        Context::new(self.limits)
    }

    /// # Errors
    /// Fails when lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile(&self, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile(source, self.limits)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
