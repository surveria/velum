use crate::api::host::{HostCall, IntoJsValue};
use crate::compiled_script::CompiledScript;
use crate::error::Result;
use crate::runtime::Context;
use crate::runtime::limits::RuntimeLimits;
use crate::value::Value;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct EngineConfig {
    default_vm_config: VmConfig,
}

impl EngineConfig {
    #[must_use]
    pub const fn with_default_vm_config(default_vm_config: VmConfig) -> Self {
        Self { default_vm_config }
    }

    #[must_use]
    pub const fn default_vm_config(self) -> VmConfig {
        self.default_vm_config
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Engine {
    config: EngineConfig,
}

impl Engine {
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(EngineConfig::default())
    }

    #[must_use]
    pub const fn with_config(config: EngineConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub const fn config(&self) -> EngineConfig {
        self.config
    }

    #[must_use]
    pub fn create_vm(&self) -> Vm {
        Vm::with_config(self.config.default_vm_config())
    }

    /// Creates a VM with an embedder-provided monotonic clock source. The
    /// first source reading is the VM-local zero point for
    /// `performance.now()`.
    #[must_use]
    pub fn create_vm_with_clock<F>(&self, read: F) -> Vm
    where
        F: Fn() -> Duration + 'static,
    {
        Vm::with_config_and_clock(self.config.default_vm_config(), read)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct VmConfig {
    limits: RuntimeLimits,
}

impl VmConfig {
    #[must_use]
    pub const fn with_limits(limits: RuntimeLimits) -> Self {
        Self { limits }
    }

    #[must_use]
    pub const fn limits(self) -> RuntimeLimits {
        self.limits
    }
}

#[derive(Debug, Clone)]
pub struct Vm {
    config: VmConfig,
    context: Context,
}

impl Vm {
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(VmConfig::default())
    }

    #[must_use]
    pub fn with_config(config: VmConfig) -> Self {
        Self {
            config,
            context: Context::new(config.limits()),
        }
    }

    /// Creates a configured VM with an embedder-provided monotonic clock
    /// source. The first source reading is the VM-local zero point for
    /// `performance.now()`.
    #[must_use]
    pub fn with_config_and_clock<F>(config: VmConfig, read: F) -> Self
    where
        F: Fn() -> Duration + 'static,
    {
        Self {
            config,
            context: Context::with_monotonic_clock(config.limits(), read),
        }
    }

    #[must_use]
    pub const fn config(&self) -> VmConfig {
        self.config
    }

    #[must_use]
    pub const fn context(&mut self) -> &mut Context {
        &mut self.context
    }

    /// # Errors
    /// Fails when lexing, parsing, evaluation, or configured resource limits fail.
    pub fn eval(&mut self, source: &str) -> Result<Value> {
        self.context.eval(source)
    }

    /// # Errors
    /// Fails when lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile(&self, source: &str) -> Result<CompiledScript> {
        self.context.compile(source)
    }

    /// # Errors
    /// Fails when the compiled script exceeds this VM's limits or evaluation fails.
    pub fn eval_compiled(&mut self, script: &CompiledScript) -> Result<crate::Value> {
        self.context.eval_compiled(script)
    }

    #[must_use]
    pub fn output(&self) -> &[String] {
        self.context.output()
    }

    #[must_use]
    pub fn take_output(&mut self) -> Vec<String> {
        self.context.take_output()
    }

    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<Value> {
        self.context.get_global(name)
    }

    /// # Errors
    /// Fails when the name is empty, exceeds string limits, duplicates an
    /// existing binding, or would exceed the binding limit.
    pub fn register_host_function<F>(&mut self, name: impl Into<String>, callback: F) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Value> + 'static,
    {
        self.context.register_host_function(name, callback)
    }

    /// # Errors
    /// Fails when the name is empty, exceeds string limits, duplicates an
    /// existing binding, or would exceed the binding limit.
    pub fn register_host_function_typed<F, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        self.context.register_host_function_typed(name, callback)
    }

    #[must_use]
    pub fn resource_usage(&self) -> VmResourceUsage {
        VmResourceUsage {
            runtime_steps: self.context.runtime_steps(),
            bytecode_linear_segment_runs: self.context.bytecode_linear_segment_runs(),
            bytecode_linear_direct_runs: self.context.bytecode_linear_direct_runs(),
            output_entries: self.context.output().len(),
            global_bindings: self.context.global_binding_count(),
            atom_count: self.context.atom_count(),
            string_count: self.context.string_count(),
            string_bytes: self.context.string_bytes(),
            shape_count: self.context.shape_count(),
            native_function_count: self.context.native_function_count(),
            prototype_lookup_version: self.context.prototype_lookup_version(),
            upvalue_cell_count: self.context.upvalue_cell_count(),
            native_call_cache_hits: self.context.native_call_cache_hits(),
            native_call_cache_misses: self.context.native_call_cache_misses(),
            native_call_cache_slow_paths: self.context.native_call_cache_slow_paths(),
            call_value_cache_hits: self.context.call_value_cache_hits(),
            call_value_cache_misses: self.context.call_value_cache_misses(),
            call_value_cache_slow_paths: self.context.call_value_cache_slow_paths(),
        }
    }

    #[must_use]
    pub fn teardown_report(&self) -> VmTeardownReport {
        VmTeardownReport {
            resources: self.resource_usage(),
        }
    }

    #[must_use]
    pub fn finish(self) -> VmTeardownReport {
        self.teardown_report()
    }
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct VmResourceUsage {
    pub runtime_steps: usize,
    pub bytecode_linear_segment_runs: usize,
    pub bytecode_linear_direct_runs: usize,
    pub output_entries: usize,
    pub global_bindings: usize,
    pub atom_count: usize,
    pub string_count: usize,
    pub string_bytes: usize,
    pub shape_count: usize,
    pub native_function_count: usize,
    pub prototype_lookup_version: u64,
    pub upvalue_cell_count: usize,
    pub native_call_cache_hits: usize,
    pub native_call_cache_misses: usize,
    pub native_call_cache_slow_paths: usize,
    pub call_value_cache_hits: usize,
    pub call_value_cache_misses: usize,
    pub call_value_cache_slow_paths: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct VmTeardownReport {
    pub resources: VmResourceUsage,
}
