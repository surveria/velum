use crate::{
    error::{Error, Result},
    runtime::{limits::VmStorageLimits, storage_ledger::VmStorageLedger},
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
    pub(super) objects: Vec<super::Object>,
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
            objects: Vec::new(),
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
