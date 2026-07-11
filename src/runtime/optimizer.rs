/// Selects whether a VM may execute optional optimized paths.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OptimizationMode {
    /// Use verified quickening, caches, and guarded specialized paths.
    #[default]
    Enabled,
    /// Execute the generic semantic paths and leave optional caches cold.
    Disabled,
}

/// Stable VM-local diagnostics owned by the optimizer boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmOptimizationSnapshot {
    mode: OptimizationMode,
    bytecode_linear_segment_runs: usize,
    bytecode_linear_direct_runs: usize,
    native_call_cache_hits: usize,
    native_call_cache_misses: usize,
    native_call_cache_slow_paths: usize,
    call_value_cache_hits: usize,
    call_value_cache_misses: usize,
    call_value_cache_slow_paths: usize,
}

impl VmOptimizationSnapshot {
    /// Returns the configured optimization mode.
    #[must_use]
    pub const fn mode(self) -> OptimizationMode {
        self.mode
    }

    /// Returns the number of segmented linear-plan executions.
    #[must_use]
    pub const fn bytecode_linear_segment_runs(self) -> usize {
        self.bytecode_linear_segment_runs
    }

    /// Returns the number of direct specialized loop iterations.
    #[must_use]
    pub const fn bytecode_linear_direct_runs(self) -> usize {
        self.bytecode_linear_direct_runs
    }

    /// Returns native-call cache hits.
    #[must_use]
    pub const fn native_call_cache_hits(self) -> usize {
        self.native_call_cache_hits
    }

    /// Returns native-call cache misses.
    #[must_use]
    pub const fn native_call_cache_misses(self) -> usize {
        self.native_call_cache_misses
    }

    /// Returns native-call cache slow paths after a failed guard.
    #[must_use]
    pub const fn native_call_cache_slow_paths(self) -> usize {
        self.native_call_cache_slow_paths
    }

    /// Returns generic value-call cache hits.
    #[must_use]
    pub const fn call_value_cache_hits(self) -> usize {
        self.call_value_cache_hits
    }

    /// Returns generic value-call cache misses.
    #[must_use]
    pub const fn call_value_cache_misses(self) -> usize {
        self.call_value_cache_misses
    }

    /// Returns generic value-call cache slow paths after a failed guard.
    #[must_use]
    pub const fn call_value_cache_slow_paths(self) -> usize {
        self.call_value_cache_slow_paths
    }
}

#[derive(Debug)]
pub(in crate::runtime) struct Optimizer {
    mode: OptimizationMode,
    bytecode_linear_segment_runs: usize,
    bytecode_linear_direct_runs: usize,
    native_call_cache_hits: usize,
    native_call_cache_misses: usize,
    native_call_cache_slow_paths: usize,
    call_value_cache_hits: usize,
    call_value_cache_misses: usize,
    call_value_cache_slow_paths: usize,
}

impl Optimizer {
    pub(in crate::runtime) const fn new(mode: OptimizationMode) -> Self {
        Self {
            mode,
            bytecode_linear_segment_runs: 0,
            bytecode_linear_direct_runs: 0,
            native_call_cache_hits: 0,
            native_call_cache_misses: 0,
            native_call_cache_slow_paths: 0,
            call_value_cache_hits: 0,
            call_value_cache_misses: 0,
            call_value_cache_slow_paths: 0,
        }
    }

    pub(in crate::runtime) const fn optional_paths_enabled(&self) -> bool {
        matches!(self.mode, OptimizationMode::Enabled)
    }

    pub(in crate::runtime) const fn snapshot(&self) -> VmOptimizationSnapshot {
        VmOptimizationSnapshot {
            mode: self.mode,
            bytecode_linear_segment_runs: self.bytecode_linear_segment_runs,
            bytecode_linear_direct_runs: self.bytecode_linear_direct_runs,
            native_call_cache_hits: self.native_call_cache_hits,
            native_call_cache_misses: self.native_call_cache_misses,
            native_call_cache_slow_paths: self.native_call_cache_slow_paths,
            call_value_cache_hits: self.call_value_cache_hits,
            call_value_cache_misses: self.call_value_cache_misses,
            call_value_cache_slow_paths: self.call_value_cache_slow_paths,
        }
    }

    pub(in crate::runtime) fn record_linear_segment_runs(
        &mut self,
        runs: usize,
    ) -> crate::Result<()> {
        self.bytecode_linear_segment_runs = self
            .bytecode_linear_segment_runs
            .checked_add(runs)
            .ok_or_else(|| crate::Error::limit("bytecode linear segment counter overflowed"))?;
        Ok(())
    }

    pub(in crate::runtime) fn record_linear_direct_runs(
        &mut self,
        runs: usize,
    ) -> crate::Result<()> {
        self.bytecode_linear_direct_runs = self
            .bytecode_linear_direct_runs
            .checked_add(runs)
            .ok_or_else(|| crate::Error::limit("bytecode linear direct counter overflowed"))?;
        Ok(())
    }

    pub(in crate::runtime) const fn record_native_call_cache_hit(&mut self) {
        self.native_call_cache_hits = self.native_call_cache_hits.saturating_add(1);
    }

    pub(in crate::runtime) const fn record_native_call_cache_miss(&mut self) {
        self.native_call_cache_misses = self.native_call_cache_misses.saturating_add(1);
    }

    pub(in crate::runtime) const fn record_native_call_cache_slow_path(&mut self) {
        self.native_call_cache_slow_paths = self.native_call_cache_slow_paths.saturating_add(1);
    }

    pub(in crate::runtime) const fn record_call_value_cache_hit(&mut self) {
        self.call_value_cache_hits = self.call_value_cache_hits.saturating_add(1);
    }

    pub(in crate::runtime) const fn record_call_value_cache_miss(&mut self) {
        self.call_value_cache_misses = self.call_value_cache_misses.saturating_add(1);
    }

    pub(in crate::runtime) const fn record_call_value_cache_slow_path(&mut self) {
        self.call_value_cache_slow_paths = self.call_value_cache_slow_paths.saturating_add(1);
    }
}

impl super::Context {
    pub(super) fn record_bytecode_linear_segment_run(&mut self) -> crate::Result<()> {
        self.optimizer.record_linear_segment_runs(1)
    }

    pub(super) fn record_bytecode_linear_direct_run(&mut self) -> crate::Result<()> {
        self.optimizer.record_linear_direct_runs(1)
    }

    pub(super) const fn record_native_call_cache_hit(&mut self) {
        self.optimizer.record_native_call_cache_hit();
    }

    pub(super) const fn record_native_call_cache_miss(&mut self) {
        self.optimizer.record_native_call_cache_miss();
    }

    pub(super) const fn record_native_call_cache_slow_path(&mut self) {
        self.optimizer.record_native_call_cache_slow_path();
    }

    pub(super) const fn record_call_value_cache_hit(&mut self) {
        self.optimizer.record_call_value_cache_hit();
    }

    pub(super) const fn record_call_value_cache_miss(&mut self) {
        self.optimizer.record_call_value_cache_miss();
    }

    pub(super) const fn record_call_value_cache_slow_path(&mut self) {
        self.optimizer.record_call_value_cache_slow_path();
    }
}
