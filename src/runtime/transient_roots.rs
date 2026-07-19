#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;

use crate::sync::Mutex;

use crate::{error::Result, value::Value};

use super::{
    Context, VmStorageKind,
    arena::SlotArena,
    roots::{DirectRootVisitor, VmRootKind},
    storage_ledger::VmStorageLedger,
};

#[derive(Debug)]
pub(in crate::runtime) struct TransientRootRegistry {
    state: Rc<Mutex<TransientRootState>>,
}

impl TransientRootRegistry {
    pub(in crate::runtime) fn new(storage_ledger: VmStorageLedger) -> Self {
        Self {
            state: Rc::new(Mutex::new(TransientRootState::new(storage_ledger))),
        }
    }

    pub(in crate::runtime) fn scope<'value, I>(
        &self,
        kind: VmRootKind,
        values: I,
    ) -> Result<TransientRootScope>
    where
        I: IntoIterator<Item = &'value Value>,
    {
        if !kind.is_transient() {
            return Err(crate::Error::runtime(
                "transient root scope requires a transient root category",
            ));
        }
        let mut values = values
            .into_iter()
            .filter(|value| is_traceable(value))
            .peekable();
        if values.peek().is_none() {
            return Ok(TransientRootScope::inactive());
        }
        let scope = self.active_scope(kind)?;
        scope.add_values(values)?;
        Ok(scope)
    }

    pub(in crate::runtime) fn active_scope(&self, kind: VmRootKind) -> Result<TransientRootScope> {
        if !kind.is_transient() {
            return Err(crate::Error::runtime(
                "transient root scope requires a transient root category",
            ));
        }
        let mut state = self.state.lock();
        let removal_capacity = state
            .scopes
            .len()
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("transient root scope count overflowed"))?;
        state.scopes.reserve_removals(removal_capacity)?;
        let scope = state.scopes.insert(TransientRootBucket::new(kind))?;
        drop(state);
        Ok(TransientRootScope {
            state: Some(Rc::clone(&self.state)),
            scope,
        })
    }

    pub(in crate::runtime) fn visit<V: DirectRootVisitor>(&self, visitor: &mut V) -> Result<()> {
        let state = self.state.lock();
        for bucket in &state.scopes {
            for value in &bucket.values {
                visitor.visit_value(bucket.kind, value)?;
            }
        }
        drop(state);
        Ok(())
    }

    pub(in crate::runtime) fn active_count(&self) -> usize {
        self.state.lock().root_count
    }
}

impl Context {
    pub(crate) fn transient_root_scope<'value, I>(
        &self,
        kind: VmRootKind,
        values: I,
    ) -> Result<TransientRootScope>
    where
        I: IntoIterator<Item = &'value Value>,
    {
        self.transient_roots.scope(kind, values)
    }

    pub(crate) fn active_transient_root_scope(
        &self,
        kind: VmRootKind,
    ) -> Result<TransientRootScope> {
        self.transient_roots.active_scope(kind)
    }
}

#[derive(Debug)]
struct TransientRootState {
    scopes: SlotArena<TransientRootBucket>,
    root_count: usize,
    storage_ledger: VmStorageLedger,
}

impl TransientRootState {
    const fn new(storage_ledger: VmStorageLedger) -> Self {
        Self {
            scopes: SlotArena::new(),
            root_count: 0,
            storage_ledger,
        }
    }
}

#[derive(Debug)]
struct TransientRootBucket {
    kind: VmRootKind,
    values: Vec<Value>,
}

impl TransientRootBucket {
    const fn new(kind: VmRootKind) -> Self {
        Self {
            kind,
            values: Vec::new(),
        }
    }
}

/// Scoped owner for traceable values held outside durable VM arenas.
///
/// Dropping the scope removes its roots even when a host callback unwinds.
#[derive(Debug)]
#[must_use = "transient roots must stay alive across the allocation point"]
pub struct TransientRootScope {
    state: Option<Rc<Mutex<TransientRootState>>>,
    scope: usize,
}

impl TransientRootScope {
    const fn inactive() -> Self {
        Self {
            state: None,
            scope: 0,
        }
    }

    pub(crate) fn add_values<'value, I>(&self, values: I) -> Result<()>
    where
        I: IntoIterator<Item = &'value Value>,
    {
        let Some(state) = &self.state else {
            return Ok(());
        };
        let mut additions = Vec::new();
        for value in values.into_iter().filter(|value| is_traceable(value)) {
            additions.try_reserve(1).map_err(|error| {
                crate::Error::limit(format!("transient root storage exhausted: {error}"))
            })?;
            additions.push(value.clone());
        }
        if additions.is_empty() {
            return Ok(());
        }

        let mut state = state.lock();
        let updated_count = state
            .root_count
            .checked_add(additions.len())
            .ok_or_else(|| crate::Error::limit("transient root count overflowed"))?;
        let reservation = state
            .storage_ledger
            .reserve_count(VmStorageKind::TransientRoot, additions.len())?;
        let Some(bucket) = state.scopes.get_mut(self.scope) else {
            return Err(crate::Error::runtime(
                "transient root scope is not available",
            ));
        };
        bucket
            .values
            .try_reserve(additions.len())
            .map_err(|error| {
                crate::Error::limit(format!("transient root storage exhausted: {error}"))
            })?;
        reservation.commit()?;
        bucket.values.extend(additions);
        state.root_count = updated_count;
        drop(state);
        Ok(())
    }
}

impl Drop for TransientRootScope {
    fn drop(&mut self) {
        if let Some(state) = &self.state {
            let mut state = state.lock();
            let Ok(Some(bucket)) = state.scopes.remove_reserved(self.scope) else {
                return;
            };
            let released = bucket.values.len();
            state.root_count = state.root_count.saturating_sub(released);
            state
                .storage_ledger
                .release_count_on_drop(VmStorageKind::TransientRoot, released);
        }
    }
}

pub(in crate::runtime) const fn is_traceable(value: &Value) -> bool {
    matches!(
        value,
        Value::String(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_)
    )
}
