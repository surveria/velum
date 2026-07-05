use crate::runtime::Context;
use crate::runtime_limits::RuntimeLimits;

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
    pub const fn create_vm(&self) -> Vm {
        Vm::with_config(self.config.default_vm_config())
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
    pub const fn with_config(config: VmConfig) -> Self {
        Self {
            config,
            context: Context::new(config.limits()),
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

    #[must_use]
    pub fn resource_usage(&self) -> VmResourceUsage {
        VmResourceUsage {
            runtime_steps: self.context.runtime_steps(),
            output_entries: self.context.output().len(),
            global_bindings: self.context.global_binding_count(),
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
    pub output_entries: usize,
    pub global_bindings: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct VmTeardownReport {
    pub resources: VmResourceUsage,
}
