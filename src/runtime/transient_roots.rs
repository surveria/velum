use std::rc::Rc;

use parking_lot::Mutex;

use crate::{error::Result, value::Value};

use super::{
    Context,
    roots::{DirectRootVisitor, VmRootKind},
};

#[derive(Debug)]
pub(in crate::runtime) struct TransientRootRegistry {
    state: Rc<Mutex<TransientRootState>>,
}

impl TransientRootRegistry {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            state: Rc::new(Mutex::new(TransientRootState::new())),
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
        let scope = state.next_scope_id;
        state.next_scope_id = state
            .next_scope_id
            .checked_add(1)
            .ok_or_else(|| crate::Error::limit("transient root scope id overflowed"))?;
        drop(state);
        Ok(TransientRootScope {
            state: Some(Rc::clone(&self.state)),
            scope,
            kind,
        })
    }

    pub(in crate::runtime) fn visit<V: DirectRootVisitor>(&self, visitor: &mut V) -> Result<()> {
        let state = self.state.lock();
        for root in &state.roots {
            visitor.visit_value(root.kind, &root.value)?;
        }
        drop(state);
        Ok(())
    }

    pub(in crate::runtime) fn active_count(&self) -> usize {
        self.state.lock().roots.len()
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
    next_scope_id: usize,
    roots: Vec<TransientRoot>,
}

impl TransientRootState {
    const fn new() -> Self {
        Self {
            next_scope_id: 0,
            roots: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct TransientRoot {
    scope: usize,
    kind: VmRootKind,
    value: Value,
}

/// Scoped owner for traceable values held outside durable VM arenas.
///
/// Dropping the scope removes its roots even when a host callback unwinds.
#[derive(Debug)]
#[must_use = "transient roots must stay alive across the allocation point"]
pub struct TransientRootScope {
    state: Option<Rc<Mutex<TransientRootState>>>,
    scope: usize,
    kind: VmRootKind,
}

impl TransientRootScope {
    const fn inactive() -> Self {
        Self {
            state: None,
            scope: 0,
            kind: VmRootKind::TransientTemporary,
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
        for value in values.into_iter().filter(|value| is_traceable(value)) {
            state.roots.try_reserve(1).map_err(|error| {
                crate::Error::limit(format!("transient root storage exhausted: {error}"))
            })?;
            state.roots.push(TransientRoot {
                scope: self.scope,
                kind: self.kind,
                value: value.clone(),
            });
        }
        drop(state);
        Ok(())
    }
}

impl Drop for TransientRootScope {
    fn drop(&mut self) {
        if let Some(state) = &self.state {
            state.lock().roots.retain(|root| root.scope != self.scope);
        }
    }
}

const fn is_traceable(value: &Value) -> bool {
    matches!(
        value,
        Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_)
    )
}
