use crate::{
    error::{Error, Result},
    runtime::{arena::SlotArena, limits::VmStorageLimits, storage_ledger::VmStorageLedger},
    value::ObjectId,
};

use super::shape::ShapeTable;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum LiteralPrototype {
    Object(ObjectId),
    Null,
}

impl LiteralPrototype {
    pub(super) const fn into_object_id(self) -> Option<ObjectId> {
        match self {
            Self::Object(id) => Some(id),
            Self::Null => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct PrototypeLookupVersion(u64);

impl PrototypeLookupVersion {
    const fn initial() -> Self {
        Self(1)
    }

    pub(super) const fn value(self) -> u64 {
        self.0
    }

    fn next(self) -> Result<Self> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or_else(|| Error::limit("prototype lookup version overflowed"))
    }
}

#[derive(Debug, Clone)]
pub struct ObjectHeap {
    pub(super) objects: SlotArena<super::Object>,
    pub(super) private_slots: Vec<Vec<crate::runtime::private::PrivateSlot>>,
    pub(super) shapes: ShapeTable,
    pub(super) object_prototype: Option<ObjectId>,
    pub(super) array_prototype: Option<ObjectId>,
    pub(super) storage_limits: VmStorageLimits,
    pub(super) storage_ledger: VmStorageLedger,
    pub(super) object_payload_bytes: usize,
    pub(super) byte_buffer_count: usize,
    pub(super) byte_buffer_payload_bytes: usize,
    prototype_lookup_version: PrototypeLookupVersion,
}

impl ObjectHeap {
    pub(in crate::runtime) fn new(
        storage_limits: VmStorageLimits,
        storage_ledger: VmStorageLedger,
    ) -> Self {
        Self {
            objects: SlotArena::new(),
            private_slots: Vec::new(),
            shapes: ShapeTable::new(storage_ledger.clone()),
            object_prototype: None,
            array_prototype: None,
            storage_limits,
            storage_ledger,
            object_payload_bytes: 0,
            byte_buffer_count: 0,
            byte_buffer_payload_bytes: 0,
            prototype_lookup_version: PrototypeLookupVersion::initial(),
        }
    }

    pub(crate) const fn prototype_lookup_version(&self) -> u64 {
        self.prototype_lookup_version.value()
    }

    pub(in crate::runtime) const fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub(in crate::runtime) const fn object_slot_count(&self) -> usize {
        self.objects.slot_len()
    }

    pub(in crate::runtime) fn sweep_unmarked_objects(&mut self, marks: &[bool]) -> Result<usize> {
        for (index, slots) in self.private_slots.iter_mut().enumerate() {
            if !marks.get(index).copied().unwrap_or(false) {
                slots.clear();
            }
        }
        let removed = self.objects.sweep_unmarked(marks)?;
        if removed == 0 {
            return Ok(0);
        }
        let mut object_payload_bytes = 0_usize;
        let mut byte_buffer_count = 0_usize;
        let mut byte_buffer_payload_bytes = 0_usize;
        for object in &self.objects {
            let (object_bytes, buffer_count, buffer_bytes) = object.storage_payload_bytes()?;
            object_payload_bytes = object_payload_bytes
                .checked_add(object_bytes)
                .ok_or_else(|| Error::limit("object payload bytes overflowed"))?;
            byte_buffer_count = byte_buffer_count
                .checked_add(buffer_count)
                .ok_or_else(|| Error::limit("byte buffer count overflowed"))?;
            byte_buffer_payload_bytes = byte_buffer_payload_bytes
                .checked_add(buffer_bytes)
                .ok_or_else(|| Error::limit("byte buffer payload bytes overflowed"))?;
        }
        self.object_payload_bytes = object_payload_bytes;
        self.byte_buffer_count = byte_buffer_count;
        self.byte_buffer_payload_bytes = byte_buffer_payload_bytes;
        self.bump_prototype_lookup_version()?;
        Ok(removed)
    }

    pub(super) fn bump_prototype_lookup_version(&mut self) -> Result<()> {
        self.prototype_lookup_version = self.prototype_lookup_version.next()?;
        Ok(())
    }
}

impl Default for PrototypeLookupVersion {
    fn default() -> Self {
        Self::initial()
    }
}
