#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;

use crate::sync::Mutex;

use crate::{error::Result, value::Value};

use super::{
    Context, VmStorageKind,
    roots::{DirectRootVisitor, VmRootKind},
    storage_ledger::VmStorageLedger,
};

const MAX_RETAINED_ROOT_VALUES_PER_SCOPE: usize = 32;

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

    // Keep the per-instruction empty-root scan inline while sharing the heavier
    // active-scope path across every concrete iterator type.
    #[inline]
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
        let mut values = values.into_iter();
        let Some(first_traceable_value) = values.find(|value| is_traceable(value)) else {
            return Ok(TransientRootScope::inactive());
        };

        self.scope_with_values(kind, first_traceable_value, &mut values)
    }

    #[inline(never)]
    fn scope_with_values<'value>(
        &self,
        kind: VmRootKind,
        first_traceable_value: &'value Value,
        values: &mut (dyn Iterator<Item = &'value Value> + '_),
    ) -> Result<TransientRootScope> {
        let mut state = self.state.lock();
        let scope = state.activate_scope(kind)?;
        if let Err(error) = state.add_scope_values(scope, Some(first_traceable_value), values) {
            state.release_scope(scope);
            return Err(error);
        }
        drop(state);
        Ok(TransientRootScope {
            state: Some(Rc::clone(&self.state)),
            scope,
        })
    }

    fn scope_with_slice_and_value(
        &self,
        kind: VmRootKind,
        values: &[Value],
        last: &Value,
    ) -> Result<TransientRootScope> {
        if !kind.is_transient() {
            return Err(crate::Error::runtime(
                "transient root scope requires a transient root category",
            ));
        }
        let traceable_count = values
            .iter()
            .chain(core::iter::once(last))
            .filter(|value| is_traceable(value))
            .count();
        if traceable_count == 0 {
            return Ok(TransientRootScope::inactive());
        }

        let mut state = self.state.lock();
        let scope = state.activate_scope(kind)?;
        if let Err(error) = state.add_scope_slice_and_value(scope, values, last, traceable_count) {
            state.release_scope(scope);
            return Err(error);
        }
        drop(state);
        Ok(TransientRootScope {
            state: Some(Rc::clone(&self.state)),
            scope,
        })
    }

    pub(in crate::runtime) fn active_scope(&self, kind: VmRootKind) -> Result<TransientRootScope> {
        if !kind.is_transient() {
            return Err(crate::Error::runtime(
                "transient root scope requires a transient root category",
            ));
        }
        let mut state = self.state.lock();
        let scope = state.activate_scope(kind)?;
        drop(state);
        Ok(TransientRootScope {
            state: Some(Rc::clone(&self.state)),
            scope,
        })
    }

    pub(in crate::runtime) fn visit<V: DirectRootVisitor>(&self, visitor: &mut V) -> Result<()> {
        let state = self.state.lock();
        for bucket in &state.scopes {
            if !bucket.active {
                continue;
            }
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

    pub(in crate::runtime) fn transient_bytecode_root_scope(
        &self,
        values: &[Value],
        last: &Value,
    ) -> Result<TransientRootScope> {
        self.transient_roots
            .scope_with_slice_and_value(VmRootKind::TransientOperand, values, last)
    }
}

#[derive(Debug)]
struct TransientRootState {
    scopes: Vec<TransientRootBucket>,
    free_scopes: Vec<usize>,
    root_count: usize,
    storage_ledger: VmStorageLedger,
}

impl TransientRootState {
    const fn new(storage_ledger: VmStorageLedger) -> Self {
        Self {
            scopes: Vec::new(),
            free_scopes: Vec::new(),
            root_count: 0,
            storage_ledger,
        }
    }

    fn activate_scope(&mut self, kind: VmRootKind) -> Result<usize> {
        if let Some(scope) = self.free_scopes.pop() {
            let Some(bucket) = self.scopes.get_mut(scope) else {
                self.free_scopes.push(scope);
                return Err(crate::Error::runtime(
                    "transient root free scope is not available",
                ));
            };
            if bucket.active || !bucket.values.is_empty() {
                self.free_scopes.push(scope);
                return Err(crate::Error::runtime(
                    "transient root free scope is still active",
                ));
            }
            bucket.kind = kind;
            bucket.active = true;
            return Ok(scope);
        }

        let scope_count = self
            .scopes
            .len()
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("transient root scope count overflowed"))?;
        self.scopes.try_reserve(1).map_err(|error| {
            crate::Error::limit(format!("transient root scope storage exhausted: {error}"))
        })?;
        let free_capacity = scope_count.saturating_sub(self.free_scopes.len());
        self.free_scopes
            .try_reserve(free_capacity)
            .map_err(|error| {
                crate::Error::limit(format!(
                    "transient root free scope storage exhausted: {error}"
                ))
            })?;
        let scope = self.scopes.len();
        self.scopes.push(TransientRootBucket::new(kind));
        Ok(scope)
    }

    fn truncate_scope(&mut self, scope: usize, length: usize) {
        if let Some(bucket) = self.scopes.get_mut(scope) {
            bucket.values.truncate(length);
        }
    }

    fn add_scope_values<'value>(
        &mut self,
        scope: usize,
        first_traceable_value: Option<&'value Value>,
        values: &mut (dyn Iterator<Item = &'value Value> + '_),
    ) -> Result<()> {
        let Some(bucket) = self.scopes.get_mut(scope) else {
            return Err(crate::Error::runtime(
                "transient root scope is not available",
            ));
        };
        if !bucket.active {
            return Err(crate::Error::runtime("transient root scope is not active"));
        }
        let initial_length = bucket.values.len();
        if let Some(value) = first_traceable_value {
            if let Err(error) = bucket.values.try_reserve(1) {
                return Err(crate::Error::limit(format!(
                    "transient root storage exhausted: {error}"
                )));
            }
            bucket.values.push(value.clone());
        }
        for value in values.filter(|value| is_traceable(value)) {
            if let Err(error) = bucket.values.try_reserve(1) {
                bucket.values.truncate(initial_length);
                return Err(crate::Error::limit(format!(
                    "transient root storage exhausted: {error}"
                )));
            }
            bucket.values.push(value.clone());
        }
        let Some(additions) = bucket.values.len().checked_sub(initial_length) else {
            bucket.values.truncate(initial_length);
            return Err(crate::Error::runtime(
                "transient root count decreased while adding",
            ));
        };
        if additions == 0 {
            return Ok(());
        }

        let Some(updated_count) = self.root_count.checked_add(additions) else {
            self.truncate_scope(scope, initial_length);
            return Err(crate::Error::limit("transient root count overflowed"));
        };
        if let Err(error) = self
            .storage_ledger
            .grow_count(VmStorageKind::TransientRoot, additions)
        {
            self.truncate_scope(scope, initial_length);
            return Err(error);
        }
        self.root_count = updated_count;
        Ok(())
    }

    fn add_scope_slice_and_value(
        &mut self,
        scope: usize,
        values: &[Value],
        last: &Value,
        traceable_count: usize,
    ) -> Result<()> {
        let Some(bucket) = self.scopes.get_mut(scope) else {
            return Err(crate::Error::runtime(
                "transient root scope is not available",
            ));
        };
        if !bucket.active {
            return Err(crate::Error::runtime("transient root scope is not active"));
        }
        if let Err(error) = bucket.values.try_reserve(traceable_count) {
            return Err(crate::Error::limit(format!(
                "transient root storage exhausted: {error}"
            )));
        }
        for value in values
            .iter()
            .chain(core::iter::once(last))
            .filter(|value| is_traceable(value))
        {
            bucket.values.push(value.clone());
        }
        let Some(updated_count) = self.root_count.checked_add(traceable_count) else {
            bucket.values.clear();
            return Err(crate::Error::limit("transient root count overflowed"));
        };
        if let Err(error) = self
            .storage_ledger
            .grow_count(VmStorageKind::TransientRoot, traceable_count)
        {
            bucket.values.clear();
            return Err(error);
        }
        self.root_count = updated_count;
        Ok(())
    }

    fn release_scope(&mut self, scope: usize) {
        let Some(bucket) = self.scopes.get_mut(scope) else {
            return;
        };
        let Some(released) = bucket.deactivate() else {
            return;
        };
        self.free_scopes.push(scope);
        self.root_count = self.root_count.saturating_sub(released);
        self.storage_ledger
            .release_count_on_drop(VmStorageKind::TransientRoot, released);
    }
}

#[derive(Debug)]
struct TransientRootBucket {
    active: bool,
    kind: VmRootKind,
    values: Vec<Value>,
}

impl TransientRootBucket {
    const fn new(kind: VmRootKind) -> Self {
        Self {
            active: true,
            kind,
            values: Vec::new(),
        }
    }

    fn deactivate(&mut self) -> Option<usize> {
        if !self.active {
            return None;
        }
        let released = self.values.len();
        self.values.clear();
        if self.values.capacity() > MAX_RETAINED_ROOT_VALUES_PER_SCOPE {
            self.values = Vec::new();
        }
        self.active = false;
        Some(released)
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
        let mut state = state.lock();
        let mut values = values.into_iter();
        state.add_scope_values(self.scope, None, &mut values)?;
        drop(state);
        Ok(())
    }
}

impl Drop for TransientRootScope {
    fn drop(&mut self) {
        if let Some(state) = &self.state {
            let mut state = state.lock();
            state.release_scope(self.scope);
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
