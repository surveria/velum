use crate::compiled_script::CompiledScript;
use crate::error::Result;
use crate::runtime::Context;
use crate::runtime::limits::RuntimeLimits;
use std::time::Duration;

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

    /// Creates a context with an embedder-provided monotonic clock source.
    /// The first source reading is the VM-local zero point for
    /// `performance.now()`.
    #[must_use]
    pub fn context_with_clock<F>(&self, read: F) -> Context
    where
        F: Fn() -> Duration + 'static,
    {
        Context::with_monotonic_clock(self.limits, read)
    }

    /// # Errors
    /// Fails when lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile(&self, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile(source, self.limits)
    }

    /// Compiles source with a stable embedder-provided diagnostic name.
    ///
    /// # Errors
    /// Fails when the source name exceeds configured string limits, or when
    /// lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile_named(&self, source_name: &str, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile_named(source_name, source, self.limits)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
