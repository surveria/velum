use std::sync::Arc;

use super::{VmStorageKind, accounting::STORAGE_KIND_COUNT};

const DEFAULT_MAX_SOURCE_LEN: usize = 65_536;
const DEFAULT_MAX_STATEMENTS: usize = 4_096;
const DEFAULT_MAX_EXPRESSION_DEPTH: usize = 256;
const DEFAULT_MAX_RUNTIME_STEPS: usize = 100_000;
const DEFAULT_MAX_STRING_LEN: usize = 65_536;
const DEFAULT_MAX_BINDINGS: usize = 4_096;
const DEFAULT_MAX_OBJECTS: usize = 4_096;
const DEFAULT_MAX_OBJECT_PROPERTIES: usize = 4_096;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RuntimeLimits {
    pub max_source_len: usize,
    pub max_statements: usize,
    pub max_expression_depth: usize,
    pub max_runtime_steps: usize,
    pub max_string_len: usize,
    pub max_bindings: usize,
    pub max_objects: usize,
    pub max_object_properties: usize,
    pub storage: VmStorageLimits,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            max_source_len: DEFAULT_MAX_SOURCE_LEN,
            max_statements: DEFAULT_MAX_STATEMENTS,
            max_expression_depth: DEFAULT_MAX_EXPRESSION_DEPTH,
            max_runtime_steps: DEFAULT_MAX_RUNTIME_STEPS,
            max_string_len: DEFAULT_MAX_STRING_LEN,
            max_bindings: DEFAULT_MAX_BINDINGS,
            max_objects: DEFAULT_MAX_OBJECTS,
            max_object_properties: DEFAULT_MAX_OBJECT_PROPERTIES,
            storage: VmStorageLimits::unlimited(),
        }
    }
}

/// Per-owner hard limits for VM-retained records and logical payload bytes.
///
/// Defaults are unlimited so the existing focused limits remain source
/// compatible. Embedders can tighten individual categories with the builder
/// methods while leaving unrelated owners unconstrained.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VmStorageLimits {
    policy: VmStorageLimitPolicy,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum VmStorageLimitPolicy {
    Unlimited,
    Custom(Arc<VmStorageLimitTable>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct VmStorageLimitTable {
    max_counts: [usize; STORAGE_KIND_COUNT],
    max_payload_bytes: [usize; STORAGE_KIND_COUNT],
}

impl VmStorageLimits {
    /// Returns a policy that does not add owner-level limits.
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            policy: VmStorageLimitPolicy::Unlimited,
        }
    }

    /// Sets the retained logical-record limit for one owner category.
    #[must_use]
    pub fn with_max_count(self, kind: VmStorageKind, limit: usize) -> Self {
        let mut table = self.into_table();
        if let Some(slot) = table.max_counts.get_mut(kind.index()) {
            *slot = limit;
        }
        Self {
            policy: VmStorageLimitPolicy::Custom(Arc::new(table)),
        }
    }

    /// Sets the retained logical payload-byte limit for one owner category.
    #[must_use]
    pub fn with_max_payload_bytes(self, kind: VmStorageKind, limit: usize) -> Self {
        let mut table = self.into_table();
        if let Some(slot) = table.max_payload_bytes.get_mut(kind.index()) {
            *slot = limit;
        }
        Self {
            policy: VmStorageLimitPolicy::Custom(Arc::new(table)),
        }
    }

    /// Returns the configured logical-record limit for one category.
    #[must_use]
    pub fn max_count(&self, kind: VmStorageKind) -> usize {
        match &self.policy {
            VmStorageLimitPolicy::Unlimited => usize::MAX,
            VmStorageLimitPolicy::Custom(table) => {
                table.max_counts.get(kind.index()).copied().unwrap_or(0)
            }
        }
    }

    /// Returns the configured logical payload-byte limit for one category.
    #[must_use]
    pub fn max_payload_bytes(&self, kind: VmStorageKind) -> usize {
        match &self.policy {
            VmStorageLimitPolicy::Unlimited => usize::MAX,
            VmStorageLimitPolicy::Custom(table) => table
                .max_payload_bytes
                .get(kind.index())
                .copied()
                .unwrap_or(0),
        }
    }

    fn into_table(self) -> VmStorageLimitTable {
        match self.policy {
            VmStorageLimitPolicy::Unlimited => VmStorageLimitTable::unlimited(),
            VmStorageLimitPolicy::Custom(table) => (*table).clone(),
        }
    }
}

impl VmStorageLimitTable {
    const fn unlimited() -> Self {
        Self {
            max_counts: [usize::MAX; STORAGE_KIND_COUNT],
            max_payload_bytes: [usize::MAX; STORAGE_KIND_COUNT],
        }
    }
}

impl Default for VmStorageLimits {
    fn default() -> Self {
        Self::unlimited()
    }
}
