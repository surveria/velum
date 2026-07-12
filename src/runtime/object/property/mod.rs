mod descriptor;
mod key;
mod keys;
mod lookup;
mod slot;

use super::array::ArrayIndex;
use super::shape::{PropertySlot, ShapeId, ShapePropertyAttributes, ShapeTable};
use super::{ARRAY_LENGTH_PROPERTY, Object, ObjectHeap};

pub(in crate::runtime) use descriptor::AccessorPropertyDescriptor;
pub use descriptor::{
    AccessorPropertyUpdate, DataPropertyDescriptor, DataPropertyUpdate, ObjectProperty,
    OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable, PropertyUpdate,
    PropertyWritable,
};
pub(in crate::runtime) use descriptor::{
    is_compatible_own_property_descriptor, is_compatible_property_update,
};
pub use key::{ObjectPropertyInit, PropertyKey, PropertyLookup};
pub use lookup::{
    CacheableNativePropertyValue, CacheablePropertyDelete, CacheablePropertyLookup,
    CacheablePropertyPresence, CacheablePropertyValue, CacheablePropertyWrite,
};

pub(in crate::runtime::object) use keys::push_unique_key;
pub(in crate::runtime::object) use lookup::PrototypeTraversalBudget;
pub(in crate::runtime::object) use slot::NamedProperty;
