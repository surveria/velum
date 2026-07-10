use crate::{
    error::{Error, Result},
    runtime::{
        Context,
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
    pub(in crate::runtime) fn create_collection(
        &mut self,
        kind: CollectionKind,
    ) -> Result<CollectionId> {
        if self.collections.len() >= self.limits.max_objects {
            return Err(Error::limit(format!(
                "collection count exceeded {}",
                self.limits.max_objects
            )));
        }
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
        if self.collection_object_slots.len() < required {
            self.collection_object_slots.resize(required, None);
        }
        let Some(slot) = self.collection_object_slots.get_mut(index) else {
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
        let max_entries = self.limits.max_object_properties;
        let data = self.collection_mut(id)?;
        if let Some(entry) = data
            .entries
            .iter_mut()
            .find(|(entry_key, _)| same_value_zero(entry_key, &key))
        {
            entry.1 = value;
            return Ok(());
        }
        if data.entries.len() >= max_entries {
            return Err(Error::limit(format!(
                "collection entry count exceeded {max_entries}"
            )));
        }
        data.entries.push((key, value));
        Ok(())
    }

    pub(in crate::runtime) fn collection_delete(
        &mut self,
        id: CollectionId,
        key: &Value,
    ) -> Result<bool> {
        let data = self.collection_mut(id)?;
        let before = data.entries.len();
        data.entries
            .retain(|(entry_key, _)| !same_value_zero(entry_key, key));
        Ok(data.entries.len() != before)
    }

    pub(in crate::runtime) fn collection_clear(&mut self, id: CollectionId) -> Result<()> {
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
        if self.collection_iterators.len() >= self.limits.max_objects {
            return Err(Error::limit(format!(
                "collection iterator count exceeded {}",
                self.limits.max_objects
            )));
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
