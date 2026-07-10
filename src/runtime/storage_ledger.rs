use std::{cell::Cell, rc::Rc};

use crate::error::{Error, Result};

use super::{VmStorageKind, accounting::STORAGE_KIND_COUNT, limits::VmStorageLimits};

/// VM-local O(1) usage ledger shared only by storage components of one VM.
#[derive(Clone, Debug)]
pub(in crate::runtime) struct VmStorageLedger {
    state: Rc<VmStorageLedgerState>,
}

#[derive(Debug)]
struct VmStorageLedgerState {
    limits: VmStorageLimits,
    counts: [Cell<usize>; STORAGE_KIND_COUNT],
    payload_bytes: [Cell<usize>; STORAGE_KIND_COUNT],
}

#[must_use]
pub(in crate::runtime) struct VmStorageReservation {
    ledger: VmStorageLedger,
    kind: VmStorageKind,
    previous_count: usize,
    projected_count: usize,
    previous_payload_bytes: usize,
    projected_payload_bytes: usize,
}

impl VmStorageLedger {
    pub(in crate::runtime) fn new(limits: VmStorageLimits) -> Self {
        Self {
            state: Rc::new(VmStorageLedgerState {
                limits,
                counts: std::array::from_fn(|_| Cell::new(0)),
                payload_bytes: std::array::from_fn(|_| Cell::new(0)),
            }),
        }
    }

    pub(in crate::runtime) fn reserve_count(
        &self,
        kind: VmStorageKind,
        additional_count: usize,
    ) -> Result<VmStorageReservation> {
        self.reserve(kind, additional_count, 0)
    }

    pub(in crate::runtime) fn grow_count(
        &self,
        kind: VmStorageKind,
        additional_count: usize,
    ) -> Result<()> {
        let count = self.count_cell(kind)?;
        let projected_count = count
            .get()
            .checked_add(additional_count)
            .ok_or_else(|| Error::limit("storage category count overflowed"))?;
        let max_count = self.state.limits.max_count(kind);
        if projected_count > max_count {
            return Err(Error::limit(format!(
                "{kind:?} record count exceeded {max_count}"
            )));
        }
        count.set(projected_count);
        Ok(())
    }

    pub(in crate::runtime) fn reserve(
        &self,
        kind: VmStorageKind,
        additional_count: usize,
        additional_payload_bytes: usize,
    ) -> Result<VmStorageReservation> {
        let count = self.count_cell(kind)?;
        let payload_bytes = self.payload_cell(kind)?;
        let previous_count = count.get();
        let previous_payload_bytes = payload_bytes.get();
        let projected_count = previous_count
            .checked_add(additional_count)
            .ok_or_else(|| Error::limit("storage category count overflowed"))?;
        let projected_payload_bytes = previous_payload_bytes
            .checked_add(additional_payload_bytes)
            .ok_or_else(|| Error::limit("storage category payload bytes overflowed"))?;
        let max_count = self.state.limits.max_count(kind);
        if projected_count > max_count {
            return Err(Error::limit(format!(
                "{kind:?} record count exceeded {max_count}"
            )));
        }
        let max_payload_bytes = self.state.limits.max_payload_bytes(kind);
        if projected_payload_bytes > max_payload_bytes {
            return Err(Error::limit(format!(
                "{kind:?} payload bytes exceeded {max_payload_bytes}"
            )));
        }
        Ok(VmStorageReservation {
            ledger: self.clone(),
            kind,
            previous_count,
            projected_count,
            previous_payload_bytes,
            projected_payload_bytes,
        })
    }

    pub(in crate::runtime) fn release_count(
        &self,
        kind: VmStorageKind,
        released_count: usize,
    ) -> Result<()> {
        self.release(kind, released_count, 0)
    }

    /// Releases a count from a destructor that cannot report errors.
    ///
    /// Snapshot reconciliation still detects any violated accounting
    /// invariant after the destructor completes.
    pub(in crate::runtime) fn release_count_on_drop(
        &self,
        kind: VmStorageKind,
        released_count: usize,
    ) {
        let Ok(count) = self.count_cell(kind) else {
            return;
        };
        count.set(count.get().saturating_sub(released_count));
    }

    pub(in crate::runtime) fn release(
        &self,
        kind: VmStorageKind,
        released_count: usize,
        released_payload_bytes: usize,
    ) -> Result<()> {
        let count = self.count_cell(kind)?;
        let payload_bytes = self.payload_cell(kind)?;
        let updated_count = count
            .get()
            .checked_sub(released_count)
            .ok_or_else(|| Error::runtime("storage ledger count underflowed"))?;
        let updated_payload_bytes = payload_bytes
            .get()
            .checked_sub(released_payload_bytes)
            .ok_or_else(|| Error::runtime("storage ledger payload bytes underflowed"))?;
        count.set(updated_count);
        payload_bytes.set(updated_payload_bytes);
        Ok(())
    }

    pub(in crate::runtime) fn count(&self, kind: VmStorageKind) -> Result<usize> {
        self.count_cell(kind).map(Cell::get)
    }

    fn count_cell(&self, kind: VmStorageKind) -> Result<&Cell<usize>> {
        self.state
            .counts
            .get(kind.index())
            .ok_or_else(|| Error::runtime("storage kind count is not defined"))
    }

    fn payload_cell(&self, kind: VmStorageKind) -> Result<&Cell<usize>> {
        self.state
            .payload_bytes
            .get(kind.index())
            .ok_or_else(|| Error::runtime("storage kind payload bytes are not defined"))
    }
}

impl VmStorageReservation {
    pub(in crate::runtime) fn commit(self) -> Result<()> {
        let count = self.ledger.count_cell(self.kind)?;
        let payload_bytes = self.ledger.payload_cell(self.kind)?;
        if count.get() != self.previous_count || payload_bytes.get() != self.previous_payload_bytes
        {
            return Err(Error::runtime(format!(
                "{:?} storage reservation became stale: count {} != {}, payload {} != {}",
                self.kind,
                count.get(),
                self.previous_count,
                payload_bytes.get(),
                self.previous_payload_bytes
            )));
        }
        count.set(self.projected_count);
        payload_bytes.set(self.projected_payload_bytes);
        Ok(())
    }
}
