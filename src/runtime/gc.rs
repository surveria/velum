use std::collections::{BTreeSet, VecDeque};

use crate::{
    error::{Error, Result},
    runtime::{
        async_trace::VmAsyncEdgeKind,
        collections::{CollectionId, CollectionIteratorId},
        object::PropertyKey,
        promise::PromiseId,
        roots::{DirectRootVisitor, VmRootKind},
        trace::{
            StrongEdgeReference, StrongEdgeVisitor, VmCallableEdgeKind, VmObjectEdgeKind,
            WeakEdgeReference, WeakEdgeVisitor,
        },
    },
    storage::symbol::SymbolId,
    value::{BoundFunctionId, FunctionId, HostFunctionId, NativeFunctionId, ObjectId, Value},
};

use super::Context;

const GC_KIND_COUNT: usize = 9;

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
        }
    }
}

/// Deterministic mark result before any VM records are reclaimed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmHeapReachabilitySnapshot {
    reachable: [usize; GC_KIND_COUNT],
    unreachable: [usize; GC_KIND_COUNT],
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
    symbols: BTreeSet<SymbolId>,
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
            symbols: BTreeSet::new(),
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
                MarkTarget::HostFunction(id) => {
                    context
                        .host_functions
                        .get(id.index())
                        .ok_or_else(|| Error::runtime("reachable host function disappeared"))?;
                }
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
            Value::Symbol(symbol) => Ok(self.symbols.insert(symbol.id())),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_) => Ok(false),
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

    fn mark_property_key(&mut self, key: PropertyKey) -> bool {
        key.symbol_id()
            .is_some_and(|symbol| self.symbols.insert(symbol))
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
                self.symbols.insert(symbol.id());
            }
            StrongEdgeReference::String(_string) => {}
            StrongEdgeReference::PromiseAssociation { object, promise } => {
                self.mark_value(&Value::Object(object))?;
                self.mark_promise(promise)?;
            }
            StrongEdgeReference::CollectionAssociation { object, collection } => {
                self.mark_value(&Value::Object(object))?;
                self.mark_collection(collection)?;
            }
        }
        Ok(())
    }

    fn weak_key_is_reachable(&self, value: &Value) -> bool {
        match value {
            Value::Object(id) => self.objects.get(id.index()).copied().unwrap_or(false),
            Value::Symbol(symbol) => self.symbols.contains(&symbol.id()),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => false,
        }
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
    /// Marks every VM-owned record reachable from explicit roots without
    /// mutating the heap.
    ///
    /// # Errors
    /// Fails if a root or edge points outside its live arena or a count
    /// exceeds the supported range.
    pub fn heap_reachability_snapshot(&self) -> Result<VmHeapReachabilitySnapshot> {
        VmHeapReachabilitySnapshot::capture(self)
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
