use crate::value::ObjectId;

use super::runtime_object_shape::ShapeTable;

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

#[derive(Debug, Clone, Default)]
pub struct ObjectHeap {
    pub(super) objects: Vec<super::Object>,
    pub(super) shapes: ShapeTable,
    pub(super) object_prototype: Option<ObjectId>,
    pub(super) array_prototype: Option<ObjectId>,
}

impl ObjectHeap {
    pub const fn new() -> Self {
        Self {
            objects: Vec::new(),
            shapes: ShapeTable::new(),
            object_prototype: None,
            array_prototype: None,
        }
    }
}
