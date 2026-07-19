#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::{Rc, Weak};
use core::fmt;

use crate::sync::Mutex;

use crate::{
    api::owned_value::OwnedValue,
    compiled_script::CompiledScript,
    error::{Error, Result},
    ownership::VmIdentity,
    value::Value,
};

use super::{Context, VmStorageKind, roots::DirectRootVisitor, storage_ledger::VmStorageLedger};

const INITIAL_RETAINED_SLOT_GENERATION: u64 = 1;
const FOREIGN_RETAINED_VALUE_ERROR: &str = "retained value belongs to another VM";
const STALE_RETAINED_VALUE_ERROR: &str = "retained value handle is stale";
const TORN_DOWN_RETAINED_VALUE_ERROR: &str = "retained value owner has been torn down";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RetainedSlot(usize);

impl RetainedSlot {
    const fn new(index: usize) -> Self {
        Self(index)
    }

    const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RetainedSlotGeneration(u64);

impl RetainedSlotGeneration {
    const fn initial() -> Self {
        Self(INITIAL_RETAINED_SLOT_GENERATION)
    }

    const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(generation) => Some(Self(generation)),
            None => None,
        }
    }
}

#[derive(Debug)]
struct RetainedSlotEntry {
    generation: RetainedSlotGeneration,
    value: Option<Value>,
}

#[derive(Debug)]
struct RetainedValueState {
    slots: Vec<RetainedSlotEntry>,
    storage_ledger: VmStorageLedger,
}

impl RetainedValueState {
    fn retain(&mut self, value: Value) -> Result<(RetainedSlot, RetainedSlotGeneration)> {
        for (index, entry) in self.slots.iter_mut().enumerate() {
            if entry.value.is_some() {
                continue;
            }
            let Some(generation) = entry.generation.next() else {
                continue;
            };
            self.storage_ledger
                .grow_count(VmStorageKind::RetainedHandle, 1)?;
            entry.generation = generation;
            entry.value = Some(value);
            return Ok((RetainedSlot::new(index), generation));
        }

        self.slots
            .try_reserve(1)
            .map_err(|_| Error::limit("retained value registry capacity exceeded"))?;
        self.storage_ledger
            .grow_count(VmStorageKind::RetainedHandle, 1)?;
        let slot = RetainedSlot::new(self.slots.len());
        let generation = RetainedSlotGeneration::initial();
        self.slots.push(RetainedSlotEntry {
            generation,
            value: Some(value),
        });
        Ok((slot, generation))
    }

    fn value(&self, slot: RetainedSlot, generation: RetainedSlotGeneration) -> Result<Value> {
        let Some(entry) = self.slots.get(slot.index()) else {
            return Err(Error::runtime(STALE_RETAINED_VALUE_ERROR));
        };
        if entry.generation != generation {
            return Err(Error::runtime(STALE_RETAINED_VALUE_ERROR));
        }
        entry
            .value
            .clone()
            .ok_or_else(|| Error::runtime(STALE_RETAINED_VALUE_ERROR))
    }

    fn release(&mut self, slot: RetainedSlot, generation: RetainedSlotGeneration) -> Result<()> {
        let Some(entry) = self.slots.get(slot.index()) else {
            return Err(Error::runtime(STALE_RETAINED_VALUE_ERROR));
        };
        if entry.generation != generation || entry.value.is_none() {
            return Err(Error::runtime(STALE_RETAINED_VALUE_ERROR));
        }
        self.storage_ledger
            .release_count(VmStorageKind::RetainedHandle, 1)?;
        let Some(entry) = self.slots.get_mut(slot.index()) else {
            return Err(Error::runtime(STALE_RETAINED_VALUE_ERROR));
        };
        entry.value = None;
        Ok(())
    }

    fn discard(&mut self, slot: RetainedSlot, generation: RetainedSlotGeneration) {
        let Some(entry) = self.slots.get_mut(slot.index()) else {
            return;
        };
        if entry.generation == generation && entry.value.is_some() {
            self.storage_ledger
                .release_count_on_drop(VmStorageKind::RetainedHandle, 1);
            entry.value = None;
        }
    }
}

/// A non-cloneable root for one value owned by a specific VM generation.
///
/// The handle does not expose arena ids. Dropping it releases the root as a
/// safety net; [`Self::release`] provides deterministic release with error
/// reporting.
#[must_use = "a retained value keeps its JavaScript value rooted until release or drop"]
pub struct RetainedValue {
    identity: VmIdentity,
    registry: Weak<Mutex<RetainedValueState>>,
    slot: RetainedSlot,
    slot_generation: RetainedSlotGeneration,
    active: bool,
}

impl RetainedValue {
    /// Returns the VM storage identity that owns this retained value.
    #[must_use]
    pub const fn identity(&self) -> &VmIdentity {
        &self.identity
    }

    /// Releases this root deterministically.
    ///
    /// # Errors
    /// Fails if the owning VM has already been torn down or the private slot
    /// generation is no longer current.
    pub fn release(mut self) -> Result<()> {
        let Some(registry) = self.registry.upgrade() else {
            return Err(Error::runtime(TORN_DOWN_RETAINED_VALUE_ERROR));
        };
        registry.lock().release(self.slot, self.slot_generation)?;
        self.active = false;
        Ok(())
    }
}

impl fmt::Debug for RetainedValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedValue")
            .field("identity", &self.identity)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl Drop for RetainedValue {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        registry.lock().discard(self.slot, self.slot_generation);
        self.active = false;
    }
}

#[derive(Debug)]
pub struct RetainedValueRegistry {
    identity: VmIdentity,
    state: Rc<Mutex<RetainedValueState>>,
}

impl RetainedValueRegistry {
    pub(in crate::runtime) fn new(identity: VmIdentity, storage_ledger: VmStorageLedger) -> Self {
        Self {
            identity,
            state: Rc::new(Mutex::new(RetainedValueState {
                slots: Vec::new(),
                storage_ledger,
            })),
        }
    }

    pub fn retain(&self, identity: &VmIdentity, value: Value) -> Result<RetainedValue> {
        if identity != &self.identity {
            return Err(Error::runtime(FOREIGN_RETAINED_VALUE_ERROR));
        }
        let (slot, slot_generation) = self.state.lock().retain(value)?;
        Ok(RetainedValue {
            identity: self.identity.clone(),
            registry: Rc::downgrade(&self.state),
            slot,
            slot_generation,
            active: true,
        })
    }

    pub(in crate::runtime) fn active_count(&self) -> usize {
        self.state
            .lock()
            .slots
            .iter()
            .filter(|entry| entry.value.is_some())
            .count()
    }

    pub(crate) fn value(&self, identity: &VmIdentity, handle: &RetainedValue) -> Result<Value> {
        if identity != &self.identity
            || &handle.identity != identity
            || !handle.registry.ptr_eq(&Rc::downgrade(&self.state))
        {
            return Err(Error::runtime(FOREIGN_RETAINED_VALUE_ERROR));
        }
        self.state.lock().value(handle.slot, handle.slot_generation)
    }

    pub(in crate::runtime) fn visit<V: DirectRootVisitor>(&self, visitor: &mut V) -> Result<()> {
        let state = self.state.lock();
        for entry in &state.slots {
            if let Some(value) = &entry.value {
                visitor.visit_value(super::VmRootKind::RetainedHandle, value)?;
            }
        }
        drop(state);
        Ok(())
    }
}

impl Context {
    pub(crate) fn retain_embedder_value(&self, value: Value) -> Result<RetainedValue> {
        self.retained_values.retain(self.identity(), value)
    }

    pub(crate) fn resolve_retained_value(&self, handle: &RetainedValue) -> Result<Value> {
        self.retained_values.value(self.identity(), handle)
    }

    pub(crate) const fn retained_value_registry(&self) -> &RetainedValueRegistry {
        &self.retained_values
    }

    /// Evaluates source and retains its result as a VM-bound root.
    ///
    /// # Errors
    /// Fails when evaluation or retained-slot allocation fails.
    pub fn eval_retained(&mut self, source: &str) -> Result<RetainedValue> {
        let value = self.eval(source)?;
        self.retain_embedder_value(value)
    }

    /// Evaluates compiled source and retains its result as a VM-bound root.
    ///
    /// # Errors
    /// Fails when evaluation or retained-slot allocation fails.
    pub fn eval_compiled_retained(&mut self, script: &CompiledScript) -> Result<RetainedValue> {
        let value = self.eval_compiled(script)?;
        self.retain_embedder_value(value)
    }

    /// Retains the current value of a global binding when it exists.
    ///
    /// # Errors
    /// Fails when retained-slot allocation fails.
    pub fn get_global_retained(&self, name: &str) -> Result<Option<RetainedValue>> {
        self.get_global(name)
            .map(|value| self.retain_embedder_value(value))
            .transpose()
    }

    /// Returns the ECMAScript type name of a retained value.
    ///
    /// # Errors
    /// Fails for a foreign or stale handle.
    pub fn retained_type_name(&self, handle: &RetainedValue) -> Result<&'static str> {
        self.retained_values
            .value(self.identity(), handle)
            .map(|value| value.type_name())
    }

    /// Copies a retained primitive into a VM-independent value.
    ///
    /// # Errors
    /// Fails for a foreign or stale handle, or when the retained value is a
    /// Symbol, object, or function.
    pub fn retained_to_owned(&self, handle: &RetainedValue) -> Result<OwnedValue> {
        self.retained_values
            .value(self.identity(), handle)
            .and_then(OwnedValue::try_from)
    }
}
