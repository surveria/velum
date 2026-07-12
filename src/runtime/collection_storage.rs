use crate::{runtime::disposable_stack::DisposableStackData, value::Value};

/// Which collection-backed internal-slot flavor an object owns.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum CollectionKind {
    Map,
    Set,
    WeakMap,
    WeakSet,
    DisposableStack,
}

/// VM-local backing data shared by keyed collections and disposable stacks.
#[derive(Debug, Clone)]
pub(in crate::runtime) struct CollectionData {
    pub(in crate::runtime) kind: CollectionKind,
    pub(in crate::runtime) entries: Vec<Option<(Value, Value)>>,
    pub(in crate::runtime) disposable_stack: Option<DisposableStackData>,
}

impl CollectionData {
    pub(in crate::runtime) const fn new(kind: CollectionKind) -> Self {
        let disposable_stack = if matches!(kind, CollectionKind::DisposableStack) {
            Some(DisposableStackData::new())
        } else {
            None
        };
        Self {
            kind,
            entries: Vec::new(),
            disposable_stack,
        }
    }

    pub(in crate::runtime) fn logical_entry_count(&self) -> usize {
        self.disposable_stack.as_ref().map_or_else(
            || self.entries.iter().flatten().count(),
            DisposableStackData::resource_count,
        )
    }
}
