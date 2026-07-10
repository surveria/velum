use crate::{
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        abstract_operations::same_value_zero,
        async_trace::VmAsyncEdgeKind,
        trace::{StrongEdgeReference, StrongEdgeVisitor, WeakEdgeReference, WeakEdgeVisitor},
    },
    value::{ObjectId, Value},
};

const COLLECTION_TARGET_ERROR: &str = "method requires a compatible collection receiver";

/// VM-local index of one Map or Set backing store.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) struct CollectionId(usize);

impl CollectionId {
    const fn index(self) -> usize {
        self.0
    }
}

/// Which collection flavor an object slot belongs to.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum CollectionKind {
    Map,
    Set,
    WeakMap,
    WeakSet,
}

/// Insertion-ordered entry storage shared by Map (key/value pairs) and Set
/// (the key doubles as the value). Keys compare with `SameValueZero`.
#[derive(Debug, Clone)]
pub(in crate::runtime) struct CollectionData {
    kind: CollectionKind,
    entries: Vec<(Value, Value)>,
}

impl CollectionData {
    const fn new(kind: CollectionKind) -> Self {
        Self {
            kind,
            entries: Vec::new(),
        }
    }

    pub(in crate::runtime) fn visit_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind> + WeakEdgeVisitor<VmAsyncEdgeKind>,
    {
        for (key, value) in &self.entries {
            match self.kind {
                CollectionKind::Map | CollectionKind::Set => {
                    visitor.visit(
                        VmAsyncEdgeKind::CollectionEntry,
                        StrongEdgeReference::Value(key),
                    )?;
                    visitor.visit(
                        VmAsyncEdgeKind::CollectionEntry,
                        StrongEdgeReference::Value(value),
                    )?;
                }
                CollectionKind::WeakMap => visitor.visit_ephemeron(
                    VmAsyncEdgeKind::WeakCollectionEphemeron,
                    WeakEdgeReference::Value(key),
                    WeakEdgeReference::Value(value),
                )?,
                CollectionKind::WeakSet => visitor.visit_weak(
                    VmAsyncEdgeKind::WeakCollectionKey,
                    WeakEdgeReference::Value(key),
                )?,
            }
        }
        Ok(())
    }
}

impl Context {
    pub(in crate::runtime) fn collection_storage_entry_count(&self) -> Result<usize> {
        self.collections.iter().try_fold(0_usize, |count, data| {
            count
                .checked_add(data.entries.len())
                .ok_or_else(|| Error::limit("collection entry count overflowed"))
        })
    }

    pub(in crate::runtime) fn collection_iterator_item_count(&self) -> Result<usize> {
        self.collection_iterators
            .iter()
            .try_fold(0_usize, |count, iterator| {
                count
                    .checked_add(iterator.items.len())
                    .ok_or_else(|| Error::limit("collection iterator item count overflowed"))
            })
    }

    pub(in crate::runtime) fn create_collection(
        &mut self,
        kind: CollectionKind,
    ) -> Result<CollectionId> {
        self.storage_ledger
            .grow_count(VmStorageKind::Collection, 1)?;
        let id = CollectionId(self.collections.len());
        self.collections.push(CollectionData::new(kind));
        Ok(id)
    }

    pub(in crate::runtime) fn bind_collection_object(
        &mut self,
        object: ObjectId,
        kind: CollectionKind,
        collection: CollectionId,
    ) -> Result<()> {
        if self.collection(collection)?.kind != kind {
            return Err(Error::runtime(
                "collection object kind does not match its backing store",
            ));
        }
        let index = object.index();
        let required = index
            .checked_add(1)
            .ok_or_else(|| Error::limit("collection slot index overflowed"))?;
        let adds_association = self
            .collection_object_slots
            .get(index)
            .and_then(Option::as_ref)
            .is_none();
        if adds_association {
            self.storage_ledger
                .grow_count(VmStorageKind::Association, 1)?;
        }
        if self.collection_object_slots.len() < required {
            self.collection_object_slots.resize(required, None);
        }
        let Some(slot) = self.collection_object_slots.get_mut(index) else {
            if adds_association {
                self.storage_ledger
                    .release_count(VmStorageKind::Association, 1)?;
            }
            return Err(Error::runtime("collection slot disappeared"));
        };
        *slot = Some((kind, collection));
        Ok(())
    }

    /// Resolves a method receiver to its backing collection, checking the
    /// collection flavor so Map methods reject Set receivers and vice versa.
    pub(in crate::runtime) fn collection_from_this(
        &self,
        this_value: &Value,
        kind: CollectionKind,
    ) -> Result<CollectionId> {
        let Value::Object(object) = this_value else {
            return Err(Error::type_error(COLLECTION_TARGET_ERROR));
        };
        let slot = self
            .collection_object_slots
            .get(object.index())
            .copied()
            .flatten();
        match slot {
            Some((slot_kind, collection)) if slot_kind == kind => Ok(collection),
            _ => Err(Error::type_error(COLLECTION_TARGET_ERROR)),
        }
    }

    fn collection(&self, id: CollectionId) -> Result<&CollectionData> {
        self.collections
            .get(id.index())
            .ok_or_else(|| Error::runtime("collection storage disappeared"))
    }

    fn collection_mut(&mut self, id: CollectionId) -> Result<&mut CollectionData> {
        self.collections
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("collection storage disappeared"))
    }

    pub(in crate::runtime) fn collection_len(&self, id: CollectionId) -> Result<usize> {
        Ok(self.collection(id)?.entries.len())
    }

    pub(in crate::runtime) fn collection_get(
        &self,
        id: CollectionId,
        key: &Value,
    ) -> Result<Option<Value>> {
        Ok(self
            .collection(id)?
            .entries
            .iter()
            .find(|(entry_key, _)| same_value_zero(entry_key, key))
            .map(|(_, value)| value.clone()))
    }

    pub(in crate::runtime) fn collection_has(&self, id: CollectionId, key: &Value) -> Result<bool> {
        Ok(self
            .collection(id)?
            .entries
            .iter()
            .any(|(entry_key, _)| same_value_zero(entry_key, key)))
    }

    /// Inserts or updates an entry, normalizing -0 keys to +0 per spec.
    pub(in crate::runtime) fn collection_set(
        &mut self,
        id: CollectionId,
        key: Value,
        value: Value,
    ) -> Result<()> {
        let key = normalize_zero_key(key);
        if let Some(entry) = self
            .collection_mut(id)?
            .entries
            .iter_mut()
            .find(|(entry_key, _)| same_value_zero(entry_key, &key))
        {
            entry.1 = value;
            return Ok(());
        }
        self.storage_ledger
            .grow_count(VmStorageKind::CollectionEntry, 1)?;
        self.collection_mut(id)?.entries.push((key, value));
        Ok(())
    }

    pub(in crate::runtime) fn collection_delete(
        &mut self,
        id: CollectionId,
        key: &Value,
    ) -> Result<bool> {
        let position = self
            .collection(id)?
            .entries
            .iter()
            .position(|(entry_key, _)| same_value_zero(entry_key, key));
        let Some(position) = position else {
            return Ok(false);
        };
        self.storage_ledger
            .release_count(VmStorageKind::CollectionEntry, 1)?;
        self.collection_mut(id)?.entries.remove(position);
        Ok(true)
    }

    pub(in crate::runtime) fn collection_clear(&mut self, id: CollectionId) -> Result<()> {
        let released = self.collection(id)?.entries.len();
        self.storage_ledger
            .release_count(VmStorageKind::CollectionEntry, released)?;
        self.collection_mut(id)?.entries.clear();
        Ok(())
    }

    /// Snapshots the current entries for iteration-style consumers.
    pub(in crate::runtime) fn collection_entries(
        &self,
        id: CollectionId,
    ) -> Result<Vec<(Value, Value)>> {
        Ok(self.collection(id)?.entries.clone())
    }

    pub(in crate::runtime) const fn can_be_held_weakly(value: &Value) -> bool {
        matches!(value, Value::Object(_) | Value::Symbol(_))
    }
}

/// VM-local index of one live collection iterator snapshot.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) struct CollectionIteratorId(usize);

impl CollectionIteratorId {
    const fn index(self) -> usize {
        self.0
    }
}

/// Snapshot cursor backing one materialized collection iterator object.
#[derive(Debug, Default, Clone)]
pub(in crate::runtime) struct CollectionIteratorState {
    items: Vec<Value>,
    cursor: usize,
}

impl CollectionIteratorState {
    pub(in crate::runtime) fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        for item in &self.items {
            visitor.visit(
                VmAsyncEdgeKind::IteratorItem,
                StrongEdgeReference::Value(item),
            )?;
        }
        Ok(())
    }
}

impl Context {
    pub(in crate::runtime) fn create_collection_iterator(
        &mut self,
        items: Vec<Value>,
    ) -> Result<CollectionIteratorId> {
        self.collection_iterators.try_reserve(1).map_err(|error| {
            Error::limit(format!("collection iterator storage exhausted: {error}"))
        })?;
        self.storage_ledger
            .grow_count(VmStorageKind::CollectionIterator, 1)?;
        if let Err(error) = self
            .storage_ledger
            .grow_count(VmStorageKind::IteratorItem, items.len())
        {
            self.storage_ledger
                .release_count(VmStorageKind::CollectionIterator, 1)?;
            return Err(error);
        }
        let id = CollectionIteratorId(self.collection_iterators.len());
        self.collection_iterators
            .push(CollectionIteratorState { items, cursor: 0 });
        Ok(id)
    }

    /// Advances the iterator, returning the next item or None when finished.
    pub(in crate::runtime) fn collection_iterator_step(
        &mut self,
        id: CollectionIteratorId,
    ) -> Result<Option<Value>> {
        let state = self
            .collection_iterators
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("collection iterator disappeared"))?;
        let Some(item) = state.items.get(state.cursor).cloned() else {
            return Ok(None);
        };
        state.cursor = state
            .cursor
            .checked_add(1)
            .ok_or_else(|| Error::limit("collection iterator cursor overflowed"))?;
        Ok(Some(item))
    }
}

/// Map and Set normalize a -0 key to +0 on insertion.
fn normalize_zero_key(key: Value) -> Value {
    if matches!(&key, Value::Number(number) if *number == 0.0) {
        return Value::Number(0.0);
    }
    key
}
