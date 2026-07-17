use crate::api::host::{HostCall, IntoJsValue};
use crate::api::owned_value::OwnedValue;
use crate::api::shared_array_buffer::SharedArrayBufferHandle;
use crate::compiled_module::{CompiledModule, ModuleLoader};
use crate::compiled_script::CompiledScript;
use crate::error::Result;
use crate::ownership::VmIdentity;
use crate::runtime::Context;
use crate::runtime::VmRootSnapshot;
use crate::runtime::limits::RuntimeLimits;
use crate::runtime::{
    OptimizationMode, RealmId, RetainedValue, VmAsyncEdgeSnapshot, VmCallableEdgeSnapshot,
    VmGarbageCollectionReport, VmHeapReachabilitySnapshot, VmObjectEdgeSnapshot,
    VmOptimizationSnapshot, VmStorageSnapshot,
};
use crate::value::Value;
use std::time::Duration;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct EngineConfig {
    default_vm_config: VmConfig,
}

impl EngineConfig {
    #[must_use]
    pub const fn with_default_vm_config(default_vm_config: VmConfig) -> Self {
        Self { default_vm_config }
    }

    #[must_use]
    pub fn default_vm_config(&self) -> VmConfig {
        self.default_vm_config.clone()
    }
}

#[derive(Debug, Clone)]
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
    pub fn config(&self) -> EngineConfig {
        self.config.clone()
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

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct VmConfig {
    limits: RuntimeLimits,
    optimization_mode: OptimizationMode,
}

impl VmConfig {
    #[must_use]
    pub const fn with_limits(limits: RuntimeLimits) -> Self {
        Self {
            limits,
            optimization_mode: OptimizationMode::Enabled,
        }
    }

    #[must_use]
    pub fn limits(&self) -> RuntimeLimits {
        self.limits.clone()
    }

    /// Selects whether VMs use optional optimized execution paths.
    #[must_use]
    pub const fn with_optimization_mode(mut self, mode: OptimizationMode) -> Self {
        self.optimization_mode = mode;
        self
    }

    /// Returns the configured optional-optimization policy.
    #[must_use]
    pub const fn optimization_mode(&self) -> OptimizationMode {
        self.optimization_mode
    }
}

#[derive(Debug)]
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
        let limits = config.limits();
        let optimization_mode = config.optimization_mode();
        Self {
            config,
            context: Context::with_optimization(limits, optimization_mode),
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
        let limits = config.limits();
        let optimization_mode = config.optimization_mode();
        Self {
            config,
            context: Context::with_optimization_and_monotonic_clock(
                limits,
                optimization_mode,
                read,
            ),
        }
    }

    #[must_use]
    pub fn config(&self) -> VmConfig {
        self.config.clone()
    }

    /// Returns the opaque identity of this VM-owned storage generation.
    #[must_use]
    pub const fn identity(&self) -> &VmIdentity {
        self.context.identity()
    }

    #[must_use]
    pub const fn context(&mut self) -> &mut Context {
        &mut self.context
    }

    pub(super) const fn embedding_context_ref(&self) -> &Context {
        &self.context
    }

    pub(super) const fn embedding_context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// Runs queued Promise reactions until the VM job queue is empty.
    ///
    /// # Errors
    /// Fails when a job raises an unhandled runtime or resource-limit error.
    pub fn run_jobs(&mut self) -> Result<usize> {
        self.context.run_jobs()
    }

    /// Returns the number of Promise jobs currently ready to run.
    #[must_use]
    pub fn pending_job_count(&self) -> usize {
        self.context.pending_job_count()
    }

    /// Discards all ready Promise jobs and reactions waiting on pending
    /// Promises, including parked async function activations.
    ///
    /// # Errors
    /// Fails if VM storage-accounting invariants cannot be reconciled.
    pub fn cancel_jobs(&mut self) -> Result<usize> {
        self.context.cancel_jobs()
    }

    /// Executes queued calls submitted by asynchronous Rust host functions.
    ///
    /// Promise-valued JavaScript results remain on the ordinary job path; call
    /// [`Self::run_jobs`] to deliver settled results back to Rust futures.
    ///
    /// # Errors
    /// Fails when command accounting, JavaScript dispatch, or Promise reaction
    /// admission cannot be completed.
    pub fn run_host_commands(&mut self) -> Result<usize> {
        self.context.run_host_commands()
    }

    /// Returns queued or awaiting-result Rust-to-JavaScript calls.
    #[must_use]
    pub fn pending_host_command_count(&self) -> usize {
        self.context.pending_host_command_count()
    }

    /// Returns Rust-to-JavaScript calls not yet entered into JavaScript.
    #[must_use]
    pub fn queued_host_command_count(&self) -> usize {
        self.context.queued_host_command_count()
    }

    /// Cancels queued and awaiting-result Rust-to-JavaScript calls.
    ///
    /// # Errors
    /// Fails when command storage accounting cannot be reconciled.
    pub fn cancel_host_commands(&mut self) -> Result<usize> {
        self.context.cancel_host_commands()
    }

    /// # Errors
    /// Fails when lexing, parsing, evaluation, or configured resource limits
    /// fail. An uncaught JavaScript value is returned as
    /// [`Error::JavaScript`](crate::Error::JavaScript).
    ///
    /// The returned raw value is not a durable root. Use [`Self::eval_owned`]
    /// for portable primitives or [`Self::eval_retained`] when a value must
    /// survive across later VM calls.
    pub fn eval(&mut self, source: &str) -> Result<Value> {
        self.context.eval(source)
    }

    /// Creates an independent realm inside this VM.
    ///
    /// # Errors
    /// Fails when realm bookkeeping exceeds configured VM storage limits.
    pub fn create_realm(&mut self) -> Result<RealmId> {
        self.context.create_realm()
    }

    /// Returns a realm's global object as a raw VM-local value.
    ///
    /// # Errors
    /// Fails when `realm` belongs to another VM or is unavailable.
    pub fn realm_global(&mut self, realm: &RealmId) -> Result<Value> {
        self.context.realm_global(realm)
    }

    /// Evaluates script source in a realm owned by this VM.
    ///
    /// # Errors
    /// Fails for a foreign realm or when compilation or evaluation fails.
    pub fn eval_in_realm(&mut self, realm: &RealmId, source: &str) -> Result<Value> {
        self.context.eval_in_realm(realm, source)
    }

    /// Evaluates source and copies its result into a VM-independent primitive.
    ///
    /// # Errors
    /// Fails when evaluation fails or returns a Symbol, object, or function.
    pub fn eval_owned(&mut self, source: &str) -> Result<OwnedValue> {
        self.context.eval_owned(source)
    }

    /// Evaluates source and retains its result as a VM-bound root.
    ///
    /// # Errors
    /// Fails when evaluation or retained-slot allocation fails.
    pub fn eval_retained(&mut self, source: &str) -> Result<RetainedValue> {
        self.context.eval_retained(source)
    }

    /// Evaluates source with a stable embedder-provided diagnostic and module-referrer name.
    ///
    /// # Errors
    /// Fails when lexing, parsing, evaluation, or configured resource limits fail.
    pub fn eval_named(&mut self, source_name: &str, source: &str) -> Result<Value> {
        self.context.eval_named(source_name, source)
    }

    /// # Errors
    /// Fails when lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile(&self, source: &str) -> Result<CompiledScript> {
        self.context.compile(source)
    }

    /// Compiles source with a stable embedder-provided diagnostic name.
    ///
    /// # Errors
    /// Fails when the source name exceeds configured string limits, or when
    /// lexing, parsing, or configured compile-time resource limits fail.
    pub fn compile_named(&self, source_name: &str, source: &str) -> Result<CompiledScript> {
        self.context.compile_named(source_name, source)
    }

    /// Compiles an ECMAScript module with a stable embedder-provided specifier.
    ///
    /// # Errors
    /// Fails when module lexing, parsing, static validation, or configured
    /// compile-time resource limits fail.
    pub fn compile_module_named(&self, source_name: &str, source: &str) -> Result<CompiledModule> {
        self.context.compile_module_named(source_name, source)
    }

    /// Compiles, links, and evaluates one ECMAScript module graph through an
    /// embedder-controlled loader.
    ///
    /// # Errors
    /// Fails when loading, module compilation, linking, evaluation, or
    /// configured resource limits fail.
    pub fn eval_module_named<L: ModuleLoader>(
        &mut self,
        source_name: &str,
        source: &str,
        loader: &mut L,
    ) -> Result<Value> {
        self.context.eval_module_named(source_name, source, loader)
    }

    /// Installs the application-owned loader used by dynamic module requests.
    pub fn set_dynamic_module_loader(&mut self, loader: impl ModuleLoader + 'static) {
        self.context.set_dynamic_module_loader(loader);
    }

    /// Installs a VM-local wrapper for a shared backing store.
    ///
    /// # Errors
    /// Fails when the binding conflicts or wrapper allocation exceeds limits.
    pub fn register_shared_array_buffer(
        &mut self,
        name: &str,
        handle: &SharedArrayBufferHandle,
    ) -> Result<()> {
        self.context.register_shared_array_buffer(name, handle)
    }

    #[must_use]
    pub const fn loaded_module_count(&self) -> usize {
        self.context.loaded_module_count()
    }

    #[must_use]
    pub fn has_loaded_module(&self, source_name: &str) -> bool {
        self.context.has_loaded_module(source_name)
    }

    /// # Errors
    /// Fails when the compiled script exceeds this VM's limits or evaluation
    /// fails. An uncaught JavaScript value is returned as
    /// [`Error::JavaScript`](crate::Error::JavaScript).
    ///
    /// The returned raw value is not a durable root. Use
    /// [`Self::eval_compiled_owned`] or [`Self::eval_compiled_retained`] when
    /// the result must survive across later VM calls.
    pub fn eval_compiled(&mut self, script: &CompiledScript) -> Result<crate::Value> {
        self.context.eval_compiled(script)
    }

    /// Evaluates compiled source and copies its result into a VM-independent
    /// primitive.
    ///
    /// # Errors
    /// Fails when evaluation fails or returns a Symbol, object, or function.
    pub fn eval_compiled_owned(&mut self, script: &CompiledScript) -> Result<OwnedValue> {
        self.context.eval_compiled_owned(script)
    }

    /// Evaluates compiled source and retains its result as a VM-bound root.
    ///
    /// # Errors
    /// Fails when evaluation or retained-slot allocation fails.
    pub fn eval_compiled_retained(&mut self, script: &CompiledScript) -> Result<RetainedValue> {
        self.context.eval_compiled_retained(script)
    }

    #[must_use]
    pub fn output(&self) -> &[String] {
        self.context.output()
    }

    #[must_use]
    pub fn take_output(&mut self) -> Vec<String> {
        self.context.take_output()
    }

    /// Returns the current raw binding value without retaining it.
    ///
    /// Use [`Self::get_global_retained`] when the result must survive across
    /// later VM calls.
    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<Value> {
        self.context.get_global(name)
    }

    /// Retains the current value of a global binding when it exists.
    ///
    /// # Errors
    /// Fails when retained-slot allocation fails.
    pub fn get_global_retained(&self, name: &str) -> Result<Option<RetainedValue>> {
        self.context.get_global_retained(name)
    }

    /// Returns the ECMAScript type name of a retained value.
    ///
    /// # Errors
    /// Fails for a foreign or stale handle.
    pub fn retained_type_name(&self, handle: &RetainedValue) -> Result<&'static str> {
        self.context.retained_type_name(handle)
    }

    /// Copies a retained primitive into a VM-independent value.
    ///
    /// # Errors
    /// Fails for a foreign or stale handle, or when the retained value is a
    /// Symbol, object, or function.
    pub fn retained_to_owned(&self, handle: &RetainedValue) -> Result<OwnedValue> {
        self.context.retained_to_owned(handle)
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

    /// Registers an engine-owned host operation under an embedder-selected
    /// global function name.
    ///
    /// # Errors
    /// Fails when the name is empty, exceeds string limits, duplicates an
    /// existing binding, or would exceed the binding limit.
    pub fn register_host_operation(
        &mut self,
        name: impl Into<String>,
        operation: crate::HostOperation,
    ) -> Result<()> {
        self.context.register_host_operation(name, operation)
    }

    #[must_use]
    pub fn resource_usage(&self) -> VmResourceUsage {
        let optimization = self.optimization_snapshot();
        VmResourceUsage {
            runtime_steps: self.context.runtime_steps(),
            bytecode_linear_segment_runs: optimization.bytecode_linear_segment_runs(),
            bytecode_linear_direct_runs: optimization.bytecode_linear_direct_runs(),
            output_entries: self.context.output().len(),
            global_bindings: self.context.global_binding_count(),
            atom_count: self.context.atom_count(),
            string_count: self.context.string_count(),
            string_bytes: self.context.string_bytes(),
            shape_count: self.context.shape_count(),
            native_function_count: self.context.native_function_count(),
            prototype_lookup_version: self.context.prototype_lookup_version(),
            upvalue_cell_count: self.context.upvalue_cell_count(),
            native_call_cache_hits: optimization.native_call_cache_hits(),
            native_call_cache_misses: optimization.native_call_cache_misses(),
            native_call_cache_slow_paths: optimization.native_call_cache_slow_paths(),
            call_value_cache_hits: optimization.call_value_cache_hits(),
            call_value_cache_misses: optimization.call_value_cache_misses(),
            call_value_cache_slow_paths: optimization.call_value_cache_slow_paths(),
        }
    }

    /// Returns the VM's optimization policy and stable profiling counters.
    #[must_use]
    pub const fn optimization_snapshot(&self) -> VmOptimizationSnapshot {
        self.context.optimization_snapshot()
    }

    /// Counts the VM's currently stored direct root references.
    ///
    /// # Errors
    /// Fails if a root-reference counter exceeds the supported range.
    pub fn root_snapshot(&self) -> Result<VmRootSnapshot> {
        self.context.root_snapshot()
    }

    /// Counts physical strong-reference slots in callable stores.
    ///
    /// # Errors
    /// Fails if an edge counter exceeds the supported range.
    pub fn callable_edge_snapshot(&self) -> Result<VmCallableEdgeSnapshot> {
        self.context.callable_edge_snapshot()
    }

    /// Counts physical strong-reference slots in the ordinary object arena.
    ///
    /// # Errors
    /// Fails if an edge counter exceeds the supported range.
    pub fn object_edge_snapshot(&self) -> Result<VmObjectEdgeSnapshot> {
        self.context.object_edge_snapshot()
    }

    /// Counts Promise, collection, iterator, weak-key, and ephemeron trace
    /// records stored in asynchronous arenas.
    ///
    /// # Errors
    /// Fails if an edge counter exceeds the supported range or a category uses
    /// an incompatible trace strength.
    pub fn async_edge_snapshot(&self) -> Result<VmAsyncEdgeSnapshot> {
        self.context.async_edge_snapshot()
    }

    /// Marks VM records reachable from the explicit root and edge contracts
    /// without mutating the heap.
    ///
    /// # Errors
    /// Fails if a root or edge points outside its live arena or a count
    /// exceeds the supported range.
    pub fn heap_reachability_snapshot(&self) -> Result<VmHeapReachabilitySnapshot> {
        self.context.heap_reachability_snapshot()
    }

    /// Reclaims VM records that are not reachable from explicit roots.
    ///
    /// # Errors
    /// Fails if tracing, arena reclamation, or storage reconciliation detects
    /// an invalid VM invariant.
    pub fn collect_garbage(&mut self) -> Result<VmGarbageCollectionReport> {
        self.context.collect_garbage()
    }

    /// Counts logical records and variable-size payload bytes retained by
    /// every current VM storage owner.
    ///
    /// # Errors
    /// Fails if a category or total count or payload byte sum exceeds the
    /// supported range.
    pub fn storage_snapshot(&self) -> Result<VmStorageSnapshot> {
        self.context.storage_snapshot()
    }

    /// Previews the resources and complete storage owner set that consuming
    /// this VM would release.
    ///
    /// # Errors
    /// Fails if a storage category or total count or payload byte sum exceeds
    /// the supported range.
    pub fn teardown_report(&self) -> Result<VmTeardownReport> {
        Ok(VmTeardownReport {
            resources: self.resource_usage(),
            storage: self.storage_snapshot()?,
        })
    }

    /// Consumes the VM and reports the complete storage owner set released by
    /// deterministic Rust teardown.
    ///
    /// # Errors
    /// Fails if a storage category or total count or payload byte sum exceeds
    /// the supported range.
    pub fn finish(self) -> Result<VmTeardownReport> {
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VmTeardownReport {
    pub resources: VmResourceUsage,
    pub storage: VmStorageSnapshot,
}
