use crate::error::{Error, Result};

use super::Context;

pub(super) const STORAGE_KIND_COUNT: usize = 26;

/// Stable logical owner categories for VM-local retained storage.
///
/// Counts describe logical records, not allocator blocks or unique reachable
/// JavaScript values. A function and each binding reference it owns therefore
/// contribute to their respective owner categories independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmStorageKind {
    Atom,
    HeapString,
    Symbol,
    Binding,
    JavaScriptFunction,
    NativeFunction,
    BoundFunction,
    HostCallback,
    Object,
    ObjectProperty,
    ByteBuffer,
    Collection,
    CollectionEntry,
    CollectionIterator,
    IteratorItem,
    Promise,
    PromiseReaction,
    PromiseJob,
    RetainedHandle,
    TransientRoot,
    ExecutionFrame,
    OutputEntry,
    CacheEntry,
    Association,
    Module,
    SourceRecord,
}

impl VmStorageKind {
    const ALL: [Self; STORAGE_KIND_COUNT] = [
        Self::Atom,
        Self::HeapString,
        Self::Symbol,
        Self::Binding,
        Self::JavaScriptFunction,
        Self::NativeFunction,
        Self::BoundFunction,
        Self::HostCallback,
        Self::Object,
        Self::ObjectProperty,
        Self::ByteBuffer,
        Self::Collection,
        Self::CollectionEntry,
        Self::CollectionIterator,
        Self::IteratorItem,
        Self::Promise,
        Self::PromiseReaction,
        Self::PromiseJob,
        Self::RetainedHandle,
        Self::TransientRoot,
        Self::ExecutionFrame,
        Self::OutputEntry,
        Self::CacheEntry,
        Self::Association,
        Self::Module,
        Self::SourceRecord,
    ];

    /// Returns every storage category in stable reporting order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &Self::ALL
    }

    pub(super) const fn index(self) -> usize {
        match self {
            Self::Atom => 0,
            Self::HeapString => 1,
            Self::Symbol => 2,
            Self::Binding => 3,
            Self::JavaScriptFunction => 4,
            Self::NativeFunction => 5,
            Self::BoundFunction => 6,
            Self::HostCallback => 7,
            Self::Object => 8,
            Self::ObjectProperty => 9,
            Self::ByteBuffer => 10,
            Self::Collection => 11,
            Self::CollectionEntry => 12,
            Self::CollectionIterator => 13,
            Self::IteratorItem => 14,
            Self::Promise => 15,
            Self::PromiseReaction => 16,
            Self::PromiseJob => 17,
            Self::RetainedHandle => 18,
            Self::TransientRoot => 19,
            Self::ExecutionFrame => 20,
            Self::OutputEntry => 21,
            Self::CacheEntry => 22,
            Self::Association => 23,
            Self::Module => 24,
            Self::SourceRecord => 25,
        }
    }
}

/// Checked count and logical payload-byte snapshot across every current VM
/// storage owner.
///
/// Payload bytes cover variable-size UTF-8 text and raw byte buffers retained
/// directly by the VM. Fixed-size record layouts remain represented by
/// counts, so this contract stays independent of pointer width, allocator
/// headers, and spare capacity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmStorageSnapshot {
    counts: [usize; STORAGE_KIND_COUNT],
    payload_bytes: [usize; STORAGE_KIND_COUNT],
    total: usize,
    total_payload_bytes: usize,
}

impl VmStorageSnapshot {
    fn capture(context: &Context) -> Result<Self> {
        let mut counter = StorageCounter::new();
        context.record_storage_counts(&mut counter)?;
        context.record_storage_payload_bytes(&mut counter)?;
        let snapshot = Self {
            counts: counter.counts,
            payload_bytes: counter.payload_bytes,
            total: counter.total,
            total_payload_bytes: counter.total_payload_bytes,
        };
        context.ensure_durable_storage_ledger_matches(&snapshot)?;
        context.ensure_storage_snapshot_within_limits(&snapshot)?;
        Ok(snapshot)
    }

    /// Returns the logical record count for one owner category.
    #[must_use]
    pub fn count(&self, kind: VmStorageKind) -> usize {
        self.counts.get(kind.index()).copied().unwrap_or(0)
    }

    /// Returns variable-size logical payload bytes for one owner category.
    #[must_use]
    pub fn payload_bytes(&self, kind: VmStorageKind) -> usize {
        self.payload_bytes.get(kind.index()).copied().unwrap_or(0)
    }

    /// Returns the checked sum of all category counts.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.total
    }

    /// Returns the checked sum of logical payload bytes across all categories.
    #[must_use]
    pub const fn total_payload_bytes(&self) -> usize {
        self.total_payload_bytes
    }

    /// Returns whether every current variable-size VM owner is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total == 0
    }
}

struct StorageCounter {
    counts: [usize; STORAGE_KIND_COUNT],
    payload_bytes: [usize; STORAGE_KIND_COUNT],
    total: usize,
    total_payload_bytes: usize,
}

impl StorageCounter {
    const fn new() -> Self {
        Self {
            counts: [0; STORAGE_KIND_COUNT],
            payload_bytes: [0; STORAGE_KIND_COUNT],
            total: 0,
            total_payload_bytes: 0,
        }
    }

    fn record(&mut self, kind: VmStorageKind, count: usize) -> Result<()> {
        let current = self
            .counts
            .get_mut(kind.index())
            .ok_or_else(|| Error::runtime("storage kind index is not defined"))?;
        *current = current
            .checked_add(count)
            .ok_or_else(|| Error::limit("storage category count overflowed"))?;
        self.total = self
            .total
            .checked_add(count)
            .ok_or_else(|| Error::limit("storage record count overflowed"))?;
        Ok(())
    }

    fn record_payload_bytes(&mut self, kind: VmStorageKind, bytes: usize) -> Result<()> {
        let current = self
            .payload_bytes
            .get_mut(kind.index())
            .ok_or_else(|| Error::runtime("storage kind index is not defined"))?;
        *current = current
            .checked_add(bytes)
            .ok_or_else(|| Error::limit("storage category payload bytes overflowed"))?;
        self.total_payload_bytes = self
            .total_payload_bytes
            .checked_add(bytes)
            .ok_or_else(|| Error::limit("storage payload bytes overflowed"))?;
        Ok(())
    }
}

impl Context {
    /// Counts logical records and variable-size payload bytes retained by
    /// every current VM storage owner.
    ///
    /// # Errors
    /// Fails if a category or total count or payload byte sum exceeds the
    /// supported range.
    pub fn storage_snapshot(&self) -> Result<VmStorageSnapshot> {
        VmStorageSnapshot::capture(self)
    }

    fn ensure_durable_storage_ledger_matches(&self, snapshot: &VmStorageSnapshot) -> Result<()> {
        const ENFORCED_KINDS: [VmStorageKind; 18] = [
            VmStorageKind::Binding,
            VmStorageKind::JavaScriptFunction,
            VmStorageKind::NativeFunction,
            VmStorageKind::BoundFunction,
            VmStorageKind::ObjectProperty,
            VmStorageKind::CacheEntry,
            VmStorageKind::Collection,
            VmStorageKind::CollectionEntry,
            VmStorageKind::CollectionIterator,
            VmStorageKind::IteratorItem,
            VmStorageKind::Promise,
            VmStorageKind::PromiseReaction,
            VmStorageKind::PromiseJob,
            VmStorageKind::RetainedHandle,
            VmStorageKind::TransientRoot,
            VmStorageKind::ExecutionFrame,
            VmStorageKind::Association,
            VmStorageKind::Module,
        ];
        for kind in ENFORCED_KINDS {
            let observed = snapshot.count(kind);
            let tracked = self.storage_ledger.count(kind)?;
            if tracked != observed {
                return Err(Error::runtime(format!(
                    "{kind:?} storage ledger mismatch: tracked {tracked}, observed {observed}"
                )));
            }
        }
        Ok(())
    }

    fn ensure_storage_snapshot_within_limits(&self, snapshot: &VmStorageSnapshot) -> Result<()> {
        for kind in VmStorageKind::all() {
            self.ensure_storage_totals(
                *kind,
                snapshot.count(*kind),
                snapshot.payload_bytes(*kind),
            )?;
        }
        Ok(())
    }

    fn record_storage_counts(&self, counter: &mut StorageCounter) -> Result<()> {
        counter.record(VmStorageKind::Atom, self.atoms.len())?;
        counter.record(VmStorageKind::HeapString, self.strings.len())?;
        counter.record(VmStorageKind::Symbol, self.symbols.len())?;

        self.record_binding_storage(counter)?;
        self.record_callable_storage(counter)?;
        self.record_object_storage(counter)?;
        self.record_async_storage(counter)?;
        self.record_active_storage(counter)?;
        self.record_cache_storage(counter)?;
        self.record_association_storage(counter)?;
        counter.record(VmStorageKind::Module, 0)
    }

    fn record_storage_payload_bytes(&self, counter: &mut StorageCounter) -> Result<()> {
        counter.record_payload_bytes(VmStorageKind::Atom, self.atoms.bytes())?;
        counter.record_payload_bytes(VmStorageKind::HeapString, self.strings.bytes())?;
        counter.record_payload_bytes(
            VmStorageKind::HostCallback,
            self.host_callback_name_bytes()?,
        )?;

        let object_counts = self.objects.storage_counts()?;
        counter
            .record_payload_bytes(VmStorageKind::Object, object_counts.object_payload_bytes())?;
        counter.record_payload_bytes(
            VmStorageKind::ByteBuffer,
            object_counts.byte_buffer_payload_bytes(),
        )?;
        counter.record_payload_bytes(VmStorageKind::OutputEntry, self.output_payload_bytes())?;
        counter.record_payload_bytes(VmStorageKind::SourceRecord, self.source_record_bytes()?)
    }

    pub(crate) fn host_callback_name_bytes(&self) -> Result<usize> {
        self.host_functions
            .iter()
            .try_fold(0_usize, |total, function| {
                total
                    .checked_add(function.storage_name_bytes())
                    .ok_or_else(|| Error::limit("host callback name bytes overflowed"))
            })
    }

    const fn output_payload_bytes(&self) -> usize {
        self.output_payload_bytes
    }

    pub(crate) fn source_record_bytes(&self) -> Result<usize> {
        self.functions.iter().try_fold(0_usize, |total, function| {
            total
                .checked_add(function.source.as_deref().map_or(0, str::len))
                .ok_or_else(|| Error::limit("source record bytes overflowed"))
        })
    }

    pub(crate) fn source_record_count(&self) -> usize {
        self.functions
            .iter()
            .filter(|function| function.source.is_some())
            .count()
    }

    pub(crate) fn ensure_storage_totals(
        &self,
        kind: VmStorageKind,
        projected_count: usize,
        projected_payload_bytes: usize,
    ) -> Result<()> {
        let max_count = self.limits.storage.max_count(kind);
        if projected_count > max_count {
            return Err(Error::limit(format!(
                "{kind:?} record count exceeded {max_count}"
            )));
        }
        let max_payload_bytes = self.limits.storage.max_payload_bytes(kind);
        if projected_payload_bytes > max_payload_bytes {
            return Err(Error::limit(format!(
                "{kind:?} payload bytes exceeded {max_payload_bytes}"
            )));
        }
        Ok(())
    }

    fn record_binding_storage(&self, counter: &mut StorageCounter) -> Result<()> {
        counter.record(VmStorageKind::Binding, self.globals.len())?;
        counter.record(VmStorageKind::Binding, self.builtin_globals.len())?;
        for scope in &self.locals {
            counter.record(VmStorageKind::Binding, scope.len())?;
        }
        for frame in &self.activation_frames {
            if let Some(upvalues) = frame.upvalues() {
                counter.record(VmStorageKind::Binding, upvalues.len())?;
            }
        }
        Ok(())
    }

    fn record_callable_storage(&self, counter: &mut StorageCounter) -> Result<()> {
        for function in &self.functions {
            counter.record(VmStorageKind::Binding, function.upvalues.len())?;
            counter.record(
                VmStorageKind::ObjectProperty,
                function.properties.storage_property_count()?,
            )?;
            counter.record(
                VmStorageKind::SourceRecord,
                usize::from(function.source.is_some()),
            )?;
            counter.record(VmStorageKind::CacheEntry, function.param_binding_ids.len())?;
            counter.record(VmStorageKind::CacheEntry, function.param_atoms.len())?;
            counter.record(VmStorageKind::CacheEntry, function.param_frames.len())?;
            counter.record(
                VmStorageKind::CacheEntry,
                function
                    .class_fields
                    .as_ref()
                    .map_or(0, |fields| fields.len()),
            )?;
            counter.record(
                VmStorageKind::CacheEntry,
                usize::from(function.fast_path.is_some()),
            )?;
            counter.record(
                VmStorageKind::CacheEntry,
                function.properties.storage_cache_entry_count(),
            )?;
            if let Some(template) = &function.scope_template {
                counter.record(VmStorageKind::CacheEntry, template.storage_entry_count()?)?;
            }
        }
        for function in &self.native_functions {
            counter.record(
                VmStorageKind::ObjectProperty,
                function.properties().storage_property_count()?,
            )?;
            counter.record(
                VmStorageKind::CacheEntry,
                function.properties().storage_cache_entry_count(),
            )?;
        }

        counter.record(VmStorageKind::JavaScriptFunction, self.functions.len())?;
        counter.record(VmStorageKind::NativeFunction, self.native_functions.len())?;
        counter.record(VmStorageKind::BoundFunction, self.bound_functions.len())?;
        counter.record(VmStorageKind::HostCallback, self.host_functions.len())
    }

    fn record_object_storage(&self, counter: &mut StorageCounter) -> Result<()> {
        let object_counts = self.objects.storage_counts()?;
        counter.record(VmStorageKind::Object, object_counts.objects())?;
        counter.record(VmStorageKind::ObjectProperty, object_counts.properties())?;
        counter.record(VmStorageKind::ByteBuffer, object_counts.byte_buffers())?;
        counter.record(VmStorageKind::CacheEntry, object_counts.cache_entries())?;
        counter.record(VmStorageKind::Association, object_counts.associations())
    }

    fn record_async_storage(&self, counter: &mut StorageCounter) -> Result<()> {
        counter.record(VmStorageKind::Collection, self.collections.len())?;
        counter.record(
            VmStorageKind::CollectionEntry,
            self.collection_storage_entry_count()?,
        )?;
        counter.record(
            VmStorageKind::CollectionIterator,
            self.collection_iterators.len(),
        )?;
        counter.record(
            VmStorageKind::IteratorItem,
            self.collection_iterator_item_count()?,
        )?;

        counter.record(VmStorageKind::Promise, self.promises.len())?;
        counter.record(
            VmStorageKind::PromiseReaction,
            self.promise_reaction_count()?,
        )?;
        counter.record(VmStorageKind::PromiseJob, self.promise_jobs.len())?;
        counter.record(
            VmStorageKind::RetainedHandle,
            self.retained_values.active_count(),
        )?;
        counter.record(
            VmStorageKind::TransientRoot,
            self.transient_roots.active_count(),
        )
    }

    fn record_active_storage(&self, counter: &mut StorageCounter) -> Result<()> {
        counter.record(VmStorageKind::ExecutionFrame, self.locals.len())?;
        counter.record(VmStorageKind::ExecutionFrame, self.activation_frames.len())?;
        counter.record(VmStorageKind::OutputEntry, self.output.len())
    }

    fn record_cache_storage(&self, counter: &mut StorageCounter) -> Result<()> {
        counter.record(
            VmStorageKind::CacheEntry,
            self.well_known_properties.entry_count(),
        )?;
        counter.record(VmStorageKind::CacheEntry, self.atoms.index_entry_count())?;
        counter.record(VmStorageKind::CacheEntry, self.strings.index_entry_count())?;
        counter.record(VmStorageKind::CacheEntry, self.globals.index_entry_count()?)?;
        counter.record(
            VmStorageKind::CacheEntry,
            self.builtin_globals.index_entry_count()?,
        )?;
        for scope in &self.locals {
            counter.record(VmStorageKind::CacheEntry, scope.index_entry_count()?)?;
        }
        if let Some(keys) = self.descriptor_property_keys {
            counter.record(VmStorageKind::CacheEntry, keys.keys().count())?;
        }
        for cache in &self.static_name_atom_caches {
            counter.record(VmStorageKind::CacheEntry, cache.storage_entry_count()?)?;
        }
        for cache in &self.static_binding_caches {
            counter.record(VmStorageKind::CacheEntry, cache.storage_entry_count()?)?;
        }
        for layout in &self.static_binding_layouts {
            counter.record(VmStorageKind::CacheEntry, layout.storage_entry_count()?)?;
        }
        counter.record(
            VmStorageKind::CacheEntry,
            self.native_function_registry.ids().count(),
        )
    }

    fn record_association_storage(&self, counter: &mut StorageCounter) -> Result<()> {
        counter.record(
            VmStorageKind::Association,
            self.collection_object_slots.iter().flatten().count(),
        )?;
        counter.record(
            VmStorageKind::Association,
            self.symbols.registry_entry_count(),
        )?;
        counter.record(
            VmStorageKind::Association,
            self.promise_object_slots.iter().flatten().count(),
        )?;
        counter.record(
            VmStorageKind::Association,
            usize::from(self.global_object.is_some()),
        )?;
        counter.record(
            VmStorageKind::Association,
            usize::from(self.promise_prototype.is_some()),
        )?;
        counter.record(
            VmStorageKind::Association,
            usize::from(self.iterator_symbol.is_some()),
        )
    }
}
