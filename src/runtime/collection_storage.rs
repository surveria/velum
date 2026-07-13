use std::{collections::HashMap, rc::Rc};

use crate::{
    error::{Error, Result},
    ownership::VmIdentity,
    runtime::{
        async_disposable_stack::AsyncDisposableStackData,
        async_trace::VmAsyncEdgeKind,
        disposable_stack::DisposableStackData,
        trace::{StrongEdgeReference, StrongEdgeVisitor, WeakEdgeReference, WeakEdgeVisitor},
    },
    storage::symbol::SymbolId,
    value::{FunctionId, HostFunctionId, JsBigInt, NativeFunctionId, ObjectId, Value},
};

const COLLECTION_COMPACTION_MIN_TOMBSTONES: usize = 64;

/// Which collection-backed internal-slot flavor an object owns.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum CollectionKind {
    Map,
    Set,
    WeakMap,
    WeakSet,
    AsyncDisposableStack,
    DisposableStack,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
enum CollectionKey {
    Undefined,
    Null,
    Bool(bool),
    Number(u64),
    BigInt(JsBigInt),
    String(Rc<[u16]>),
    Symbol { identity: VmIdentity, id: SymbolId },
    Function(FunctionId),
    NativeFunction(NativeFunctionId),
    HostFunction(HostFunctionId),
    Object(ObjectId),
}

impl CollectionKey {
    fn from_value(value: &Value) -> Self {
        match value {
            Value::Undefined => Self::Undefined,
            Value::Null => Self::Null,
            Value::Bool(value) => Self::Bool(*value),
            Value::Number(value) => Self::Number(canonical_number_bits(*value)),
            Value::BigInt(value) => Self::BigInt(value.clone()),
            Value::String(value) => Self::String(value.shared_utf16()),
            Value::Symbol(value) => Self::Symbol {
                identity: value.identity().clone(),
                id: value.id(),
            },
            Value::Function(value) => Self::Function(*value),
            Value::NativeFunction(value) => Self::NativeFunction(*value),
            Value::HostFunction(value) => Self::HostFunction(*value),
            Value::Object(value) => Self::Object(*value),
        }
    }
}

fn canonical_number_bits(value: f64) -> u64 {
    if value.is_nan() {
        return f64::NAN.to_bits();
    }
    if value == 0.0 {
        return 0.0_f64.to_bits();
    }
    value.to_bits()
}

type CollectionEntry = Option<(Value, Value)>;
type CollectionIndex = HashMap<CollectionKey, usize>;

/// VM-local backing data shared by keyed collections and disposable stacks.
#[derive(Debug, Clone)]
pub(in crate::runtime) struct CollectionData {
    pub(in crate::runtime) kind: CollectionKind,
    pub(in crate::runtime) entries: Vec<CollectionEntry>,
    key_index: CollectionIndex,
    tombstones: usize,
    cursor_pins: usize,
    pub(in crate::runtime) async_disposable_stack: Option<AsyncDisposableStackData>,
    pub(in crate::runtime) disposable_stack: Option<DisposableStackData>,
}

impl CollectionData {
    pub(in crate::runtime) fn new(kind: CollectionKind) -> Self {
        let async_disposable_stack = if matches!(kind, CollectionKind::AsyncDisposableStack) {
            Some(AsyncDisposableStackData::new())
        } else {
            None
        };
        let disposable_stack = if matches!(kind, CollectionKind::DisposableStack) {
            Some(DisposableStackData::new())
        } else {
            None
        };
        Self {
            kind,
            entries: Vec::new(),
            key_index: HashMap::new(),
            tombstones: 0,
            cursor_pins: 0,
            async_disposable_stack,
            disposable_stack,
        }
    }

    pub(in crate::runtime) fn logical_entry_count(&self) -> usize {
        if let Some(stack) = &self.async_disposable_stack {
            return stack.resource_count();
        }
        self.disposable_stack
            .as_ref()
            .map_or_else(|| self.key_index.len(), DisposableStackData::resource_count)
    }

    pub(in crate::runtime) fn entry_index(&self, key: &Value) -> Option<usize> {
        self.key_index.get(&CollectionKey::from_value(key)).copied()
    }

    pub(in crate::runtime) fn entry(&self, index: usize) -> Result<&(Value, Value)> {
        self.entries
            .get(index)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::runtime("collection index points to a missing entry"))
    }

    pub(in crate::runtime) fn replace_value(&mut self, index: usize, value: Value) -> Result<()> {
        let Some((_key, entry_value)) = self.entries.get_mut(index).and_then(Option::as_mut) else {
            return Err(Error::runtime("collection index points to a missing entry"));
        };
        *entry_value = value;
        Ok(())
    }

    pub(in crate::runtime) fn prepare_new_entry(&mut self, allow_compaction: bool) -> Result<()> {
        if allow_compaction && self.should_compact() {
            self.compact()?;
        }
        self.entries
            .try_reserve(1)
            .map_err(|_| Error::limit("collection entry storage capacity exceeded"))?;
        self.key_index
            .try_reserve(1)
            .map_err(|_| Error::limit("collection key index capacity exceeded"))
    }

    pub(in crate::runtime) fn pin_cursor(&mut self) -> Result<()> {
        self.cursor_pins = self
            .cursor_pins
            .checked_add(1)
            .ok_or_else(|| Error::limit("collection cursor pin count overflowed"))?;
        Ok(())
    }

    pub(in crate::runtime) const fn unpin_cursor(&mut self) {
        self.cursor_pins = self.cursor_pins.saturating_sub(1);
    }

    pub(in crate::runtime) const fn cursor_is_pinned(&self) -> bool {
        self.cursor_pins != 0
    }

    pub(in crate::runtime) fn insert_reserved(&mut self, key: Value, value: Value) -> Result<()> {
        let lookup = CollectionKey::from_value(&key);
        if self.key_index.contains_key(&lookup) {
            return Err(Error::runtime(
                "collection key index already contains entry",
            ));
        }
        let position = self.entries.len();
        if self.key_index.insert(lookup, position).is_some() {
            return Err(Error::runtime("collection key index insertion collided"));
        }
        self.entries.push(Some((key, value)));
        Ok(())
    }

    pub(in crate::runtime) fn delete_indexed(&mut self, key: &Value, index: usize) -> Result<()> {
        let lookup = CollectionKey::from_value(key);
        if self.key_index.get(&lookup).copied() != Some(index) {
            return Err(Error::runtime("collection key index is inconsistent"));
        }
        let entry = self.entry(index)?;
        if CollectionKey::from_value(&entry.0) != lookup {
            return Err(Error::runtime("collection entry key is inconsistent"));
        }
        let tombstones = self
            .tombstones
            .checked_add(1)
            .ok_or_else(|| Error::limit("collection tombstone count overflowed"))?;
        let Some(removed_index) = self.key_index.remove(&lookup) else {
            return Err(Error::runtime("collection key index disappeared"));
        };
        if removed_index != index {
            return Err(Error::runtime(
                "collection key index changed during deletion",
            ));
        }
        let Some(entry) = self.entries.get_mut(index) else {
            return Err(Error::runtime(
                "collection entry disappeared during deletion",
            ));
        };
        *entry = None;
        self.tombstones = tombstones;
        Ok(())
    }

    pub(in crate::runtime) fn clear(&mut self, preserve_iterator_history: bool) {
        self.key_index = HashMap::new();
        if preserve_iterator_history {
            for entry in &mut self.entries {
                *entry = None;
            }
            self.tombstones = self.entries.len();
        } else {
            self.entries = Vec::new();
            self.tombstones = 0;
        }
    }

    pub(in crate::runtime) fn retain_keyed_entries(
        &mut self,
        mut keep: impl FnMut(&Value) -> bool,
    ) -> Result<usize> {
        let before = self.key_index.len();
        let (entries, key_index) = self.rebuilt_entries(|key| keep(key))?;
        self.entries = entries;
        self.key_index = key_index;
        self.tombstones = 0;
        Ok(before.saturating_sub(self.key_index.len()))
    }

    const fn should_compact(&self) -> bool {
        self.tombstones >= COLLECTION_COMPACTION_MIN_TOMBSTONES
            && self.tombstones >= self.entries.len().saturating_sub(self.tombstones)
    }

    fn compact(&mut self) -> Result<()> {
        let (entries, key_index) = self.rebuilt_entries(|_key| true)?;
        self.entries = entries;
        self.key_index = key_index;
        self.tombstones = 0;
        Ok(())
    }

    fn rebuilt_entries(
        &self,
        mut keep: impl FnMut(&Value) -> bool,
    ) -> Result<(Vec<CollectionEntry>, CollectionIndex)> {
        let live = self.key_index.len();
        let mut entries = Vec::new();
        entries
            .try_reserve(live)
            .map_err(|_| Error::limit("collection compaction capacity exceeded"))?;
        let mut key_index = HashMap::new();
        key_index
            .try_reserve(live)
            .map_err(|_| Error::limit("collection index compaction capacity exceeded"))?;

        for (key, value) in self.entries.iter().flatten() {
            if !keep(key) {
                continue;
            }
            let position = entries.len();
            if key_index
                .insert(CollectionKey::from_value(key), position)
                .is_some()
            {
                return Err(Error::runtime("collection contains duplicate indexed keys"));
            }
            entries.push(Some((key.clone(), value.clone())));
        }
        Ok((entries, key_index))
    }

    pub(in crate::runtime) fn visit_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind> + WeakEdgeVisitor<VmAsyncEdgeKind>,
    {
        if let Some(stack) = &self.async_disposable_stack {
            return stack.visit_edges(visitor);
        }
        if let Some(stack) = &self.disposable_stack {
            return stack.visit_edges(visitor);
        }
        for (key, value) in self.entries.iter().flatten() {
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
                CollectionKind::AsyncDisposableStack | CollectionKind::DisposableStack => {}
            }
        }
        Ok(())
    }

    pub(in crate::runtime) fn sweep_dead_weak_entries(
        &mut self,
        mut key_is_reachable: impl FnMut(&Value) -> bool,
    ) -> Result<usize> {
        if !matches!(self.kind, CollectionKind::WeakMap | CollectionKind::WeakSet) {
            return Ok(0);
        }
        self.retain_keyed_entries(|key| key_is_reachable(key))
    }
}
