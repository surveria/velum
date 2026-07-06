use crate::{
    error::{Error, Result},
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

#[derive(Debug, Clone, Default)]
pub struct ObjectHeap {
    pub(super) objects: Vec<super::Object>,
    pub(super) shapes: ShapeTable,
    pub(super) object_prototype: Option<ObjectId>,
    pub(super) array_prototype: Option<ObjectId>,
    prototype_lookup_version: PrototypeLookupVersion,
}

impl ObjectHeap {
    pub const fn new() -> Self {
        Self {
            objects: Vec::new(),
            shapes: ShapeTable::new(),
            object_prototype: None,
            array_prototype: None,
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
