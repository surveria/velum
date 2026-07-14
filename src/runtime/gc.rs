use std::collections::{BTreeSet, VecDeque};

use crate::{
    error::{Error, Result},
    runtime::{
        async_trace::VmAsyncEdgeKind,
        collections::{CollectionId, CollectionIteratorId},
        generator::GeneratorId,
        object::PropertyKey,
        promise::PromiseId,
        roots::{DirectRootVisitor, VmRootKind},
        trace::{
            StrongEdgeReference, StrongEdgeVisitor, VmCallableEdgeKind, VmObjectEdgeKind,
            WeakEdgeReference, WeakEdgeVisitor,
        },
    },
    storage::{string_heap::StringId, symbol::SymbolId},
    value::{BoundFunctionId, FunctionId, HostFunctionId, NativeFunctionId, ObjectId, Value},
};

use super::Context;

const GC_KIND_COUNT: usize = 11;
const AUTOMATIC_GC_OBJECT_HEADROOM_DIVISOR: usize = 16;

#[derive(Debug)]
pub(super) struct AutomaticGcState {
    object_limit: usize,
    next_object_count: usize,
}

impl AutomaticGcState {
    pub(super) const fn new(object_limit: usize) -> Self {
        Self {
            object_limit,
            next_object_count: automatic_gc_initial_object_count(object_limit),
        }
    }

    const fn should_collect(&self, object_count: usize) -> bool {
        object_count >= self.next_object_count
    }

    fn record_collection(&mut self, object_count: usize) {
        let initial = automatic_gc_initial_object_count(self.object_limit);
        if object_count < initial {
            self.next_object_count = initial;
            return;
        }

        let retry = object_count.saturating_add(automatic_gc_object_headroom(self.object_limit));
        let final_retry = self.object_limit.saturating_sub(1);
        let next = retry.min(final_retry);
        self.next_object_count = if next > object_count {
            next
        } else {
            usize::MAX
        };
    }
}

const fn automatic_gc_initial_object_count(object_limit: usize) -> usize {
    if object_limit == 0 {
        return usize::MAX;
    }
    object_limit.saturating_sub(automatic_gc_object_headroom(object_limit))
}

const fn automatic_gc_object_headroom(object_limit: usize) -> usize {
    let divided = match object_limit.checked_div(AUTOMATIC_GC_OBJECT_HEADROOM_DIVISOR) {
        Some(value) => value,
        None => 0,
    };
    if divided == 0 { 1 } else { divided }
}

/// Stable categories included in VM reachability and collection reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmGcKind {
    Object,
    JavaScriptFunction,
    NativeFunction,
    HostFunction,
    BoundFunction,
    Promise,
    Collection,
    CollectionIterator,
    Symbol,
    HeapString,
    Generator,
}

impl VmGcKind {
    const ALL: [Self; GC_KIND_COUNT] = [
        Self::Object,
        Self::JavaScriptFunction,
        Self::NativeFunction,
        Self::HostFunction,
        Self::BoundFunction,
        Self::Promise,
        Self::Collection,
        Self::CollectionIterator,
        Self::Symbol,
        Self::HeapString,
        Self::Generator,
    ];

    /// Returns all reachability categories in stable reporting order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &Self::ALL
    }

    const fn index(self) -> usize {
        match self {
            Self::Object => 0,
            Self::JavaScriptFunction => 1,
            Self::NativeFunction => 2,
            Self::HostFunction => 3,
            Self::BoundFunction => 4,
            Self::Promise => 5,
            Self::Collection => 6,
            Self::CollectionIterator => 7,
            Self::Symbol => 8,
            Self::HeapString => 9,
            Self::Generator => 10,
        }
    }
}

/// Deterministic mark result before any VM records are reclaimed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmHeapReachabilitySnapshot {
    reachable: [usize; GC_KIND_COUNT],
    unreachable: [usize; GC_KIND_COUNT],
}

/// Records reclaimed by one stop-the-world VM collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmGarbageCollectionReport {
    reclaimed: [usize; GC_KIND_COUNT],
    total_reclaimed: usize,
    weak_entries_removed: usize,
}

impl VmGarbageCollectionReport {
    /// Returns reclaimed records in one stable heap category.
    #[must_use]
    pub fn reclaimed(&self, kind: VmGcKind) -> usize {
        self.reclaimed.get(kind.index()).copied().unwrap_or(0)
    }

    /// Returns the total number of reclaimed indexed-arena records.
    #[must_use]
    pub const fn total_reclaimed(&self) -> usize {
        self.total_reclaimed
    }

    /// Returns weak collection entries removed because their keys were dead.
    #[must_use]
    pub const fn weak_entries_removed(&self) -> usize {
        self.weak_entries_removed
    }
}

impl VmHeapReachabilitySnapshot {
    fn capture(context: &Context) -> Result<Self> {
        Reachability::capture(context)?.snapshot(context)
    }

    /// Returns the number of records reachable in one category.
    #[must_use]
    pub fn reachable(&self, kind: VmGcKind) -> usize {
        self.reachable.get(kind.index()).copied().unwrap_or(0)
    }

    /// Returns the number of records not reachable from the explicit root set.
    #[must_use]
    pub fn unreachable(&self, kind: VmGcKind) -> usize {
        self.unreachable.get(kind.index()).copied().unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug)]
enum MarkTarget {
    Object(ObjectId),
    Function(FunctionId),
    NativeFunction(NativeFunctionId),
    HostFunction(HostFunctionId),
    BoundFunction(BoundFunctionId),
    Promise(PromiseId),
    Collection(CollectionId),
    CollectionIterator(CollectionIteratorId),
    Symbol(SymbolId),
    HeapString(StringId),
    Generator(GeneratorId),
}

pub(in crate::runtime) struct Reachability {
    objects: Vec<bool>,
    functions: Vec<bool>,
    native_functions: Vec<bool>,
    host_functions: Vec<bool>,
    bound_functions: Vec<bool>,
    promises: Vec<bool>,
    collections: Vec<bool>,
    collection_iterators: Vec<bool>,
    generators: Vec<bool>,
    symbols: BTreeSet<SymbolId>,
    strings: BTreeSet<StringId>,
    queue: VecDeque<MarkTarget>,
    ephemerons: Vec<(Value, Value)>,
}

impl Reachability {
    fn capture(context: &Context) -> Result<Self> {
        let mut marker = Self {
            objects: vec![false; context.objects.object_slot_count()],
            functions: vec![false; context.functions.slot_len()],
            native_functions: vec![false; context.native_functions.slot_len()],
            host_functions: vec![false; context.host_functions.slot_len()],
            bound_functions: vec![false; context.bound_functions.slot_len()],
            promises: vec![false; context.promises.slot_len()],
            collections: vec![false; context.collections.slot_len()],
            collection_iterators: vec![false; context.collection_iterators.slot_len()],
            generators: vec![false; context.generators.slot_len()],
            symbols: BTreeSet::new(),
            strings: BTreeSet::new(),
            queue: VecDeque::new(),
            ephemerons: Vec::new(),
        };
        context.visit_direct_roots(&mut marker)?;
        loop {
            marker.drain_queue(context)?;
            let mut added = false;
            for (key, value) in marker.ephemerons.clone() {
                if marker.weak_key_is_reachable(&key) {
                    added |= marker.mark_value(&value)?;
                }
            }
            if !added {
                break;
            }
        }
        Ok(marker)
    }

    fn drain_queue(&mut self, context: &Context) -> Result<()> {
        while let Some(target) = self.queue.pop_front() {
            match target {
                MarkTarget::Object(id) => {
                    context.objects.visit_object_strong_edges(id, self)?;
                    if let Some(promise) = context
                        .promise_object_slots
                        .get(id.index())
                        .copied()
                        .flatten()
                    {
                        self.mark_promise(promise)?;
                    }
                    if let Some((_kind, collection)) = context
                        .collection_object_slots
                        .get(id.index())
                        .copied()
                        .flatten()
                    {
                        self.mark_collection(collection)?;
                    }
                    if let Some(generator) = context
                        .generator_object_slots
                        .get(id.index())
                        .copied()
                        .flatten()
                    {
                        self.mark_generator(generator)?;
                    }
                }
                MarkTarget::Function(id) => context
                    .functions
                    .get(id.index())
                    .ok_or_else(|| Error::runtime("reachable function record disappeared"))?
                    .visit_strong_edges(self)?,
                MarkTarget::NativeFunction(id) => context
                    .native_functions
                    .get(id.index())
                    .ok_or_else(|| Error::runtime("reachable native function disappeared"))?
                    .visit_strong_edges(self)?,
                MarkTarget::HostFunction(id) => context
                    .host_functions
                    .get(id.index())
                    .ok_or_else(|| Error::runtime("reachable host function disappeared"))?
                    .visit_strong_edges(self)?,
                MarkTarget::BoundFunction(id) => context
                    .bound_functions
                    .get(id.index())
                    .ok_or_else(|| Error::runtime("reachable bound function disappeared"))?
                    .visit_strong_edges(self)?,
                MarkTarget::Promise(id) => context
                    .promises
                    .get(id.index())
                    .ok_or_else(|| Error::runtime("reachable Promise record disappeared"))?
                    .visit_strong_edges(self)?,
                MarkTarget::Collection(id) => context
                    .collections
                    .get(id.index())
                    .ok_or_else(|| Error::runtime("reachable collection record disappeared"))?
                    .visit_edges(self)?,
                MarkTarget::CollectionIterator(id) => context
                    .collection_iterators
                    .get(id.index())
                    .ok_or_else(|| Error::runtime("reachable collection iterator disappeared"))?
                    .visit_strong_edges(self)?,
                MarkTarget::Symbol(id) => {
                    if let Some(description) = context.symbols.get(id)?.description_string() {
                        self.mark_string(description);
                    }
                }
                MarkTarget::HeapString(id) => {
                    context.strings.get(id)?;
                }
                MarkTarget::Generator(id) => context
                    .generators
                    .get(id.index())
                    .ok_or_else(|| Error::runtime("reachable generator record disappeared"))?
                    .visit_strong_edges(self)?,
            }
        }
        Ok(())
    }

    fn snapshot(&self, context: &Context) -> Result<VmHeapReachabilitySnapshot> {
        let reachable = [
            true_count(&self.objects),
            true_count(&self.functions),
            true_count(&self.native_functions),
            true_count(&self.host_functions),
            true_count(&self.bound_functions),
            true_count(&self.promises),
            true_count(&self.collections),
            true_count(&self.collection_iterators),
            self.symbols.len(),
            self.strings.len(),
            true_count(&self.generators),
        ];
        let totals = [
            context.objects.object_count(),
            context.functions.len(),
            context.native_functions.len(),
            context.host_functions.len(),
            context.bound_functions.len(),
            context.promises.len(),
            context.collections.len(),
            context.collection_iterators.len(),
            context.symbols.len(),
            context.strings.len(),
            context.generators.len(),
        ];
        let mut unreachable = [0_usize; GC_KIND_COUNT];
        for (index, (total, live)) in totals.into_iter().zip(reachable).enumerate() {
            let Some(slot) = unreachable.get_mut(index) else {
                return Err(Error::runtime("reachability category index is not defined"));
            };
            *slot = total
                .checked_sub(live)
                .ok_or_else(|| Error::runtime("reachable record count exceeded live records"))?;
        }
        Ok(VmHeapReachabilitySnapshot {
            reachable,
            unreachable,
        })
    }

    fn mark_value(&mut self, value: &Value) -> Result<bool> {
        match value {
            Value::Function(id) => mark_slot(
                &mut self.functions,
                id.index(),
                MarkTarget::Function(*id),
                &mut self.queue,
            ),
            Value::NativeFunction(id) => mark_slot(
                &mut self.native_functions,
                id.index(),
                MarkTarget::NativeFunction(*id),
                &mut self.queue,
            ),
            Value::HostFunction(id) => mark_slot(
                &mut self.host_functions,
                id.index(),
                MarkTarget::HostFunction(*id),
                &mut self.queue,
            ),
            Value::Object(id) => mark_slot(
                &mut self.objects,
                id.index(),
                MarkTarget::Object(*id),
                &mut self.queue,
            ),
            Value::Symbol(symbol) => Ok(self.mark_symbol(symbol.id())),
            Value::String(string) => Ok(self.mark_string(string)),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_) => Ok(false),
        }
    }

    fn mark_promise(&mut self, id: PromiseId) -> Result<bool> {
        mark_slot(
            &mut self.promises,
            id.index(),
            MarkTarget::Promise(id),
            &mut self.queue,
        )
    }

    fn mark_collection(&mut self, id: CollectionId) -> Result<bool> {
        mark_slot(
            &mut self.collections,
            id.index(),
            MarkTarget::Collection(id),
            &mut self.queue,
        )
    }

    fn mark_bound_function(&mut self, id: BoundFunctionId) -> Result<bool> {
        mark_slot(
            &mut self.bound_functions,
            id.index(),
            MarkTarget::BoundFunction(id),
            &mut self.queue,
        )
    }

    fn mark_collection_iterator(&mut self, id: CollectionIteratorId) -> Result<bool> {
        mark_slot(
            &mut self.collection_iterators,
            id.index(),
            MarkTarget::CollectionIterator(id),
            &mut self.queue,
        )
    }

    fn mark_generator(&mut self, id: GeneratorId) -> Result<bool> {
        mark_slot(
            &mut self.generators,
            id.index(),
            MarkTarget::Generator(id),
            &mut self.queue,
        )
    }

    fn mark_symbol(&mut self, id: SymbolId) -> bool {
        if !self.symbols.insert(id) {
            return false;
        }
        self.queue.push_back(MarkTarget::Symbol(id));
        true
    }

    fn mark_string(&mut self, string: &crate::storage::string_heap::JsString) -> bool {
        let Some(id) = string.id() else {
            return false;
        };
        if !self.strings.insert(id) {
            return false;
        }
        self.queue.push_back(MarkTarget::HeapString(id));
        true
    }

    fn mark_property_key(&mut self, key: PropertyKey) -> bool {
        let Some(symbol) = key.symbol_id() else {
            return false;
        };
        self.mark_symbol(symbol)
    }

    fn visit_reference(&mut self, reference: StrongEdgeReference<'_>) -> Result<()> {
        match reference {
            StrongEdgeReference::Value(value) => {
                self.mark_value(value)?;
            }
            StrongEdgeReference::Function(id) => {
                mark_slot(
                    &mut self.functions,
                    id.index(),
                    MarkTarget::Function(id),
                    &mut self.queue,
                )?;
            }
            StrongEdgeReference::Object(id) => {
                self.mark_value(&Value::Object(id))?;
            }
            StrongEdgeReference::Promise(id) => {
                self.mark_promise(id)?;
            }
            StrongEdgeReference::BoundFunction(id) => {
                self.mark_bound_function(id)?;
            }
            StrongEdgeReference::CollectionIterator(id) => {
                self.mark_collection_iterator(id)?;
            }
            StrongEdgeReference::PropertyKey(key) => {
                self.mark_property_key(key);
            }
            StrongEdgeReference::Symbol(symbol) => {
                self.mark_symbol(symbol.id());
            }
            StrongEdgeReference::String(string) => {
                self.mark_string(string);
            }
            StrongEdgeReference::PromiseAssociation { object, promise } => {
                self.mark_value(&Value::Object(object))?;
                self.mark_promise(promise)?;
            }
            StrongEdgeReference::CollectionAssociation { object, collection } => {
                self.mark_value(&Value::Object(object))?;
                self.mark_collection(collection)?;
            }
            StrongEdgeReference::GeneratorAssociation { object, generator } => {
                self.mark_value(&Value::Object(object))?;
                self.mark_generator(generator)?;
            }
        }
        Ok(())
    }

    fn weak_key_is_reachable(&self, value: &Value) -> bool {
        match value {
            Value::Object(id) => self.objects.get(id.index()).copied().unwrap_or(false),
            Value::Function(id) => self.functions.get(id.index()).copied().unwrap_or(false),
            Value::NativeFunction(id) => self
                .native_functions
                .get(id.index())
                .copied()
                .unwrap_or(false),
            Value::HostFunction(id) => self
                .host_functions
                .get(id.index())
                .copied()
                .unwrap_or(false),
            Value::Symbol(symbol) => self.symbols.contains(&symbol.id()),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_) => false,
        }
    }

    fn object_is_reachable(&self, index: usize) -> bool {
        self.objects.get(index).copied().unwrap_or(false)
    }

    fn promise_is_reachable(&self, id: PromiseId) -> bool {
        self.promises.get(id.index()).copied().unwrap_or(false)
    }

    fn collection_is_reachable(&self, id: CollectionId) -> bool {
        self.collections.get(id.index()).copied().unwrap_or(false)
    }

    fn generator_is_reachable(&self, id: GeneratorId) -> bool {
        self.generators.get(id.index()).copied().unwrap_or(false)
    }
}

impl DirectRootVisitor for Reachability {
    fn visit_value(&mut self, _kind: VmRootKind, value: &Value) -> Result<()> {
        self.mark_value(value).map(|_added| ())
    }

    fn visit_promise(&mut self, _kind: VmRootKind, promise: PromiseId) -> Result<()> {
        self.mark_promise(promise).map(|_added| ())
    }

    fn visit_property_key(&mut self, _kind: VmRootKind, key: PropertyKey) -> Result<()> {
        self.mark_property_key(key);
        Ok(())
    }
}

macro_rules! strong_visitor {
    ($kind:ty) => {
        impl StrongEdgeVisitor<$kind> for Reachability {
            fn visit(&mut self, _kind: $kind, reference: StrongEdgeReference<'_>) -> Result<()> {
                self.visit_reference(reference)
            }
        }
    };
}

strong_visitor!(VmCallableEdgeKind);
strong_visitor!(VmObjectEdgeKind);
strong_visitor!(VmAsyncEdgeKind);

impl WeakEdgeVisitor<VmAsyncEdgeKind> for Reachability {
    fn visit_weak(
        &mut self,
        _kind: VmAsyncEdgeKind,
        _reference: WeakEdgeReference<'_>,
    ) -> Result<()> {
        Ok(())
    }

    fn visit_ephemeron(
        &mut self,
        _kind: VmAsyncEdgeKind,
        key: WeakEdgeReference<'_>,
        value: WeakEdgeReference<'_>,
    ) -> Result<()> {
        let WeakEdgeReference::Value(key) = key;
        let WeakEdgeReference::Value(value) = value;
        self.ephemerons.push((key.clone(), value.clone()));
        Ok(())
    }
}

impl Context {
    pub(in crate::runtime) fn collect_garbage_at_bytecode_safe_point(&mut self) -> Result<()> {
        let object_count = self.objects.object_count();
        if !self.automatic_gc.should_collect(object_count) {
            return Ok(());
        }
        self.collect_garbage().map(|_report| ())
    }

    /// Marks every VM-owned record reachable from explicit roots without
    /// mutating the heap.
    ///
    /// # Errors
    /// Fails if a root or edge points outside its live arena or a count
    /// exceeds the supported range.
    pub fn heap_reachability_snapshot(&self) -> Result<VmHeapReachabilitySnapshot> {
        VmHeapReachabilitySnapshot::capture(self)
    }

    /// Reclaims records not reachable from the VM's explicit root contract.
    /// Promise jobs, suspended async and generator activations, retained
    /// handles, runtime anchors, and registered Symbols participate in the
    /// mark phase.
    ///
    /// Raw VM-local [`Value`] ids are not durable across this call. Embedders
    /// must use retained handles for values that survive Context operations.
    ///
    /// # Errors
    /// Fails if a root or edge is invalid, arena reclamation cannot reserve its
    /// free-list storage, or post-collection accounting does not reconcile.
    pub fn collect_garbage(&mut self) -> Result<VmGarbageCollectionReport> {
        let reachability = Reachability::capture(self)?;
        let before = self.owner_storage_snapshot()?;
        self.invalidate_identity_caches();
        let weak_entries_removed = self.sweep_dead_weak_entries(&reachability)?;
        self.sweep_dead_associations(&reachability);

        let reclaimed = [
            self.objects.sweep_unmarked_objects(&reachability.objects)?,
            self.functions.sweep_unmarked(&reachability.functions)?,
            self.native_functions
                .sweep_unmarked(&reachability.native_functions)?,
            self.host_functions
                .sweep_unmarked(&reachability.host_functions)?,
            self.bound_functions
                .sweep_unmarked(&reachability.bound_functions)?,
            self.promises.sweep_unmarked(&reachability.promises)?,
            self.collections.sweep_unmarked(&reachability.collections)?,
            self.collection_iterators
                .sweep_unmarked(&reachability.collection_iterators)?,
            self.symbols.sweep_unmarked(&reachability.symbols)?,
            self.strings.sweep_unmarked(&reachability.strings)?,
            self.generators.sweep_unmarked(&reachability.generators)?,
        ];
        let after = self.owner_storage_snapshot()?;
        self.release_collected_storage(&before, &after)?;
        self.automatic_gc
            .record_collection(self.objects.object_count());
        let total_reclaimed = reclaimed.iter().try_fold(0_usize, |total, count| {
            total
                .checked_add(*count)
                .ok_or_else(|| Error::limit("reclaimed record count overflowed"))
        })?;
        Ok(VmGarbageCollectionReport {
            reclaimed,
            total_reclaimed,
            weak_entries_removed,
        })
    }

    fn invalidate_identity_caches(&mut self) {
        for cache in &self.static_name_atom_caches {
            cache.invalidate_identity_caches();
        }
        for cache in &self.static_binding_caches {
            cache.invalidate_identity_caches();
        }
        for function in self.functions.iter_mut() {
            if let Some(cache) = &function.static_name_atom_cache {
                cache.invalidate_identity_caches();
            }
            if let Some(cache) = &function.static_binding_cache {
                cache.invalidate_identity_caches();
            }
        }
    }

    fn sweep_dead_weak_entries(&mut self, reachability: &Reachability) -> Result<usize> {
        let mut removed = 0_usize;
        for (index, collection) in self.collections.indexed_mut() {
            if !reachability
                .collections
                .get(index)
                .copied()
                .unwrap_or(false)
            {
                continue;
            }
            removed = removed
                .checked_add(
                    collection
                        .sweep_dead_weak_entries(|key| reachability.weak_key_is_reachable(key))?,
                )
                .ok_or_else(|| Error::limit("weak entry removal count overflowed"))?;
        }
        Ok(removed)
    }

    fn sweep_dead_associations(&mut self, reachability: &Reachability) {
        for (index, slot) in self.promise_object_slots.iter_mut().enumerate() {
            let keep = slot.is_some_and(|promise| {
                reachability.object_is_reachable(index)
                    && reachability.promise_is_reachable(promise)
            });
            if !keep {
                *slot = None;
            }
        }
        for (index, slot) in self.collection_object_slots.iter_mut().enumerate() {
            let keep = slot.is_some_and(|(_kind, collection)| {
                reachability.object_is_reachable(index)
                    && reachability.collection_is_reachable(collection)
            });
            if !keep {
                *slot = None;
            }
        }
        for (index, slot) in self.generator_object_slots.iter_mut().enumerate() {
            let keep = slot.is_some_and(|generator| {
                reachability.object_is_reachable(index)
                    && reachability.generator_is_reachable(generator)
            });
            if !keep {
                *slot = None;
            }
        }
    }
}

fn mark_slot(
    marks: &mut [bool],
    index: usize,
    target: MarkTarget,
    queue: &mut VecDeque<MarkTarget>,
) -> Result<bool> {
    let Some(marked) = marks.get_mut(index) else {
        return Err(Error::runtime("trace edge points outside its arena"));
    };
    if *marked {
        return Ok(false);
    }
    *marked = true;
    queue.push_back(target);
    Ok(true)
}

fn true_count(marks: &[bool]) -> usize {
    marks.iter().filter(|marked| **marked).count()
}
