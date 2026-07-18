use crate::error::Result;
use crate::runtime::Context;
use crate::runtime::limits::RuntimeLimits;
use crate::{compiled_module::CompiledModule, compiled_script::CompiledScript};
use core::time::Duration;

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
    pub fn limits(&self) -> RuntimeLimits {
        self.limits.clone()
    }

    #[must_use]
    pub fn context(&self) -> Context {
        Context::new(self.limits.clone())
    }

    /// Creates a context with an embedder-provided monotonic clock source.
    /// The first source reading is the VM-local zero point for
    /// `performance.now()`.
    #[must_use]
    pub fn context_with_clock<F>(&self, read: F) -> Context
    where
        F: Fn() -> Duration + 'static,
    {
        Context::with_monotonic_clock(self.limits.clone(), read)
    }

    /// # Errors
    /// Fails when lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile(&self, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile(source, self.limits.clone())
    }

    /// Compiles source with a stable embedder-provided diagnostic name.
    ///
    /// # Errors
    /// Fails when the source name exceeds configured string limits, or when
    /// lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile_named(&self, source_name: &str, source: &str) -> Result<CompiledScript> {
        CompiledScript::compile_named(source_name, source, self.limits.clone())
    }

    /// Compiles an ECMAScript module with a stable embedder-provided specifier.
    ///
    /// # Errors
    /// Fails when module lexing, parsing, static validation, or configured
    /// compile-time resource limits fail.
    pub fn compile_module_named(&self, source_name: &str, source: &str) -> Result<CompiledModule> {
        CompiledModule::compile_named(source_name, source, self.limits.clone())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
