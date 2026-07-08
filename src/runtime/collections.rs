use crate::{
    error::{Error, Result},
    runtime::Context,
    value::{ObjectId, Value},
};

const COLLECTION_TARGET_ERROR: &str = "method requires a Map or Set receiver";

/// VM-local index of one Map or Set backing store.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct CollectionId(usize);

impl CollectionId {
    const fn index(self) -> usize {
        self.0
    }
}

/// Which collection flavor an object slot belongs to.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CollectionKind {
    Map,
    Set,
}

/// Insertion-ordered entry storage shared by Map (key/value pairs) and Set
/// (the key doubles as the value). Keys compare with SameValueZero.
#[derive(Debug, Default, Clone)]
pub(crate) struct CollectionData {
    entries: Vec<(Value, Value)>,
}

impl Context {
    pub(crate) fn create_collection(&mut self) -> Result<CollectionId> {
        if self.collections.len() >= self.limits.max_objects {
            return Err(Error::limit(format!(
                "collection count exceeded {}",
                self.limits.max_objects
            )));
        }
        let id = CollectionId(self.collections.len());
        self.collections.push(CollectionData::default());
        Ok(id)
    }

    pub(crate) fn bind_collection_object(
        &mut self,
        object: ObjectId,
        kind: CollectionKind,
        collection: CollectionId,
    ) -> Result<()> {
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
    pub(crate) fn collection_from_this(
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

    pub(crate) fn collection_len(&self, id: CollectionId) -> Result<usize> {
        Ok(self.collection(id)?.entries.len())
    }

    pub(crate) fn collection_get(&self, id: CollectionId, key: &Value) -> Result<Option<Value>> {
        Ok(self
            .collection(id)?
            .entries
            .iter()
            .find(|(entry_key, _)| same_value_zero(entry_key, key))
            .map(|(_, value)| value.clone()))
    }

    pub(crate) fn collection_has(&self, id: CollectionId, key: &Value) -> Result<bool> {
        Ok(self
            .collection(id)?
            .entries
            .iter()
            .any(|(entry_key, _)| same_value_zero(entry_key, key)))
    }

    /// Inserts or updates an entry, normalizing -0 keys to +0 per spec.
    pub(crate) fn collection_set(
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

    pub(crate) fn collection_delete(&mut self, id: CollectionId, key: &Value) -> Result<bool> {
        let data = self.collection_mut(id)?;
        let before = data.entries.len();
        data.entries
            .retain(|(entry_key, _)| !same_value_zero(entry_key, key));
        Ok(data.entries.len() != before)
    }

    pub(crate) fn collection_clear(&mut self, id: CollectionId) -> Result<()> {
        self.collection_mut(id)?.entries.clear();
        Ok(())
    }

    /// Snapshots the current entries for iteration-style consumers.
    pub(crate) fn collection_entries(&self, id: CollectionId) -> Result<Vec<(Value, Value)>> {
        Ok(self.collection(id)?.entries.clone())
    }
}

/// VM-local index of one live collection iterator snapshot.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct CollectionIteratorId(usize);

impl CollectionIteratorId {
    const fn index(self) -> usize {
        self.0
    }
}

/// Snapshot cursor backing one materialized collection iterator object.
#[derive(Debug, Default, Clone)]
pub(crate) struct CollectionIteratorState {
    items: Vec<Value>,
    cursor: usize,
}

impl Context {
    pub(crate) fn create_collection_iterator(
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
    pub(crate) fn collection_iterator_step(
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

/// SameValueZero: strict equality except NaN equals NaN and +0 equals -0.
fn same_value_zero(left: &Value, right: &Value) -> bool {
    if let (Value::Number(left), Value::Number(right)) = (left, right) {
        if left.is_nan() && right.is_nan() {
            return true;
        }
        return left == right;
    }
    left == right
}

/// Map and Set normalize a -0 key to +0 on insertion.
fn normalize_zero_key(key: Value) -> Value {
    if let Value::Number(number) = &key {
        if *number == 0.0 {
            return Value::Number(0.0);
        }
    }
    key
}
