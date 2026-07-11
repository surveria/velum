use crate::{error::Result, value::ObjectId};

use super::{
    Context,
    trace::{
        StrongEdgeReference, StrongEdgeVisitor, WeakEdgeReference, WeakEdgeVisitor,
        consume_reference, consume_weak_reference,
    },
};

const ASYNC_EDGE_KIND_COUNT: usize = 10;
const ASYNC_EDGE_STRENGTH_COUNT: usize = 3;

/// Trace strength assigned to one asynchronous-store edge category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum VmAsyncEdgeStrength {
    Strong,
    Weak,
    Ephemeron,
}

impl VmAsyncEdgeStrength {
    const ALL: [Self; ASYNC_EDGE_STRENGTH_COUNT] = [Self::Strong, Self::Weak, Self::Ephemeron];

    /// Returns every trace strength in stable reporting order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &Self::ALL
    }

    const fn index(self) -> usize {
        match self {
            Self::Strong => 0,
            Self::Weak => 1,
            Self::Ephemeron => 2,
        }
    }
}

/// Edge categories owned by Promise, collection, iterator, and generator side stores.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum VmAsyncEdgeKind {
    PromiseState,
    PromiseReaction,
    PromiseObjectAssociation,
    CollectionObjectAssociation,
    CollectionEntry,
    IteratorItem,
    WeakCollectionKey,
    WeakCollectionEphemeron,
    GeneratorObjectAssociation,
    GeneratorState,
}

impl VmAsyncEdgeKind {
    const ALL: [Self; ASYNC_EDGE_KIND_COUNT] = [
        Self::PromiseState,
        Self::PromiseReaction,
        Self::PromiseObjectAssociation,
        Self::CollectionObjectAssociation,
        Self::CollectionEntry,
        Self::IteratorItem,
        Self::WeakCollectionKey,
        Self::WeakCollectionEphemeron,
        Self::GeneratorObjectAssociation,
        Self::GeneratorState,
    ];

    /// Returns every asynchronous edge category in stable reporting order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &Self::ALL
    }

    /// Returns the trace strength required by this category.
    #[must_use]
    pub const fn strength(self) -> VmAsyncEdgeStrength {
        match self {
            Self::PromiseState
            | Self::PromiseReaction
            | Self::PromiseObjectAssociation
            | Self::CollectionObjectAssociation
            | Self::CollectionEntry
            | Self::IteratorItem
            | Self::GeneratorObjectAssociation
            | Self::GeneratorState => VmAsyncEdgeStrength::Strong,
            Self::WeakCollectionKey => VmAsyncEdgeStrength::Weak,
            Self::WeakCollectionEphemeron => VmAsyncEdgeStrength::Ephemeron,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::PromiseState => 0,
            Self::PromiseReaction => 1,
            Self::PromiseObjectAssociation => 2,
            Self::CollectionObjectAssociation => 3,
            Self::CollectionEntry => 4,
            Self::IteratorItem => 5,
            Self::WeakCollectionKey => 6,
            Self::WeakCollectionEphemeron => 7,
            Self::GeneratorObjectAssociation => 8,
            Self::GeneratorState => 9,
        }
    }
}

/// Counted view of trace records stored in resumable and asynchronous VM arenas.
///
/// Strong counts describe physical reference slots. `WeakSet` keys and `WeakMap`
/// ephemeron pairs are logical trace records because their duplicated backing
/// values must not become ordinary strong edges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmAsyncEdgeSnapshot {
    counts: [usize; ASYNC_EDGE_KIND_COUNT],
    strength_counts: [usize; ASYNC_EDGE_STRENGTH_COUNT],
    total: usize,
}

impl VmAsyncEdgeSnapshot {
    fn capture(context: &Context) -> Result<Self> {
        let mut counter = AsyncEdgeCounter::new();
        context.visit_async_edges(&mut counter)?;
        Ok(Self {
            counts: counter.counts,
            strength_counts: counter.strength_counts,
            total: counter.total,
        })
    }

    /// Returns the number of trace records in one category.
    #[must_use]
    pub fn count(self, kind: VmAsyncEdgeKind) -> usize {
        self.counts.get(kind.index()).copied().unwrap_or(0)
    }

    /// Returns the number of trace records with one strength classification.
    #[must_use]
    pub fn count_by_strength(self, strength: VmAsyncEdgeStrength) -> usize {
        self.strength_counts
            .get(strength.index())
            .copied()
            .unwrap_or(0)
    }

    /// Returns the total number of asynchronous-store trace records.
    #[must_use]
    pub const fn total(self) -> usize {
        self.total
    }

    /// Returns whether the asynchronous stores contain no trace records.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.total == 0
    }
}

struct AsyncEdgeCounter {
    counts: [usize; ASYNC_EDGE_KIND_COUNT],
    strength_counts: [usize; ASYNC_EDGE_STRENGTH_COUNT],
    total: usize,
}

impl AsyncEdgeCounter {
    const fn new() -> Self {
        Self {
            counts: [0; ASYNC_EDGE_KIND_COUNT],
            strength_counts: [0; ASYNC_EDGE_STRENGTH_COUNT],
            total: 0,
        }
    }

    fn record(&mut self, kind: VmAsyncEdgeKind, strength: VmAsyncEdgeStrength) -> Result<()> {
        if kind.strength() != strength {
            return Err(crate::Error::runtime(
                "asynchronous edge used an incompatible trace strength",
            ));
        }
        increment(
            self.counts.get_mut(kind.index()),
            "asynchronous edge category count overflowed",
        )?;
        increment(
            self.strength_counts.get_mut(strength.index()),
            "asynchronous edge strength count overflowed",
        )?;
        self.total = self
            .total
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("asynchronous edge count overflowed"))?;
        Ok(())
    }
}

impl StrongEdgeVisitor<VmAsyncEdgeKind> for AsyncEdgeCounter {
    fn visit(&mut self, kind: VmAsyncEdgeKind, reference: StrongEdgeReference<'_>) -> Result<()> {
        consume_reference(&reference);
        self.record(kind, VmAsyncEdgeStrength::Strong)
    }
}

impl WeakEdgeVisitor<VmAsyncEdgeKind> for AsyncEdgeCounter {
    fn visit_weak(
        &mut self,
        kind: VmAsyncEdgeKind,
        reference: WeakEdgeReference<'_>,
    ) -> Result<()> {
        consume_weak_reference(reference);
        self.record(kind, VmAsyncEdgeStrength::Weak)
    }

    fn visit_ephemeron(
        &mut self,
        kind: VmAsyncEdgeKind,
        key: WeakEdgeReference<'_>,
        value: WeakEdgeReference<'_>,
    ) -> Result<()> {
        consume_weak_reference(key);
        consume_weak_reference(value);
        self.record(kind, VmAsyncEdgeStrength::Ephemeron)
    }
}

impl Context {
    /// Counts Promise, collection, iterator, generator, weak-key, and
    /// ephemeron trace records without exposing VM-local arena ids.
    ///
    /// # Errors
    /// Fails if an edge counter exceeds the supported range or a category is
    /// emitted through an incompatible strength visitor.
    pub fn async_edge_snapshot(&self) -> Result<VmAsyncEdgeSnapshot> {
        VmAsyncEdgeSnapshot::capture(self)
    }

    fn visit_async_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind> + WeakEdgeVisitor<VmAsyncEdgeKind>,
    {
        for (index, promise) in self.promise_object_slots.iter().enumerate() {
            if let Some(promise) = promise {
                visitor.visit(
                    VmAsyncEdgeKind::PromiseObjectAssociation,
                    StrongEdgeReference::PromiseAssociation {
                        object: ObjectId::new(index),
                        promise: *promise,
                    },
                )?;
            }
        }
        for promise in &self.promises {
            promise.visit_strong_edges(visitor)?;
        }
        for (index, slot) in self.collection_object_slots.iter().enumerate() {
            if let Some((_kind, collection)) = slot {
                visitor.visit(
                    VmAsyncEdgeKind::CollectionObjectAssociation,
                    StrongEdgeReference::CollectionAssociation {
                        object: ObjectId::new(index),
                        collection: *collection,
                    },
                )?;
            }
        }
        for collection in &self.collections {
            collection.visit_edges(visitor)?;
        }
        for iterator in &self.collection_iterators {
            iterator.visit_strong_edges(visitor)?;
        }
        for (index, generator) in self.generator_object_slots.iter().enumerate() {
            if let Some(generator) = generator {
                visitor.visit(
                    VmAsyncEdgeKind::GeneratorObjectAssociation,
                    StrongEdgeReference::GeneratorAssociation {
                        object: ObjectId::new(index),
                        generator: *generator,
                    },
                )?;
            }
        }
        for generator in &self.generators {
            generator.visit_strong_edges(visitor)?;
        }
        Ok(())
    }
}

fn increment(slot: Option<&mut usize>, message: &'static str) -> Result<()> {
    let Some(count) = slot else {
        return Err(crate::Error::runtime(
            "asynchronous edge counter index is not defined",
        ));
    };
    *count = count
        .checked_add(1)
        .ok_or_else(|| crate::Error::limit(message))?;
    Ok(())
}
