use crate::{storage::atom::AtomId, value::Value};

use super::descriptor::PropertyEnumerable;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct PropertyKey(AtomId);

impl PropertyKey {
    pub const fn new(atom: AtomId) -> Self {
        Self(atom)
    }

    pub const fn atom(self) -> AtomId {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PropertyLookup<'a> {
    name: &'a str,
    key: Option<PropertyKey>,
}

impl<'a> PropertyLookup<'a> {
    pub const fn new(name: &'a str, key: Option<PropertyKey>) -> Self {
        Self { name, key }
    }

    pub const fn from_key(name: &'a str, key: PropertyKey) -> Self {
        Self {
            name,
            key: Some(key),
        }
    }

    pub const fn name(self) -> &'a str {
        self.name
    }

    pub const fn key(self) -> Option<PropertyKey> {
        self.key
    }
}

#[derive(Debug, Clone)]
pub struct ObjectPropertyInit<'a> {
    pub(super) key: PropertyKey,
    pub(super) name: &'a str,
    pub(super) value: Value,
    pub(super) enumerable: PropertyEnumerable,
}

impl<'a> ObjectPropertyInit<'a> {
    pub const fn new(
        key: PropertyKey,
        name: &'a str,
        value: Value,
        enumerable: PropertyEnumerable,
    ) -> Self {
        Self {
            key,
            name,
            value,
            enumerable,
        }
    }
}
