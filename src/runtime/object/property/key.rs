use crate::{
    storage::{atom::AtomId, symbol::SymbolId},
    syntax::AccessorKind,
    value::Value,
};

use super::PropertyEnumerable;

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum PropertyKey {
    Atom(AtomId),
    Symbol(SymbolId),
}

impl PropertyKey {
    pub const fn new(atom: AtomId) -> Self {
        Self::Atom(atom)
    }

    pub const fn symbol(symbol: SymbolId) -> Self {
        Self::Symbol(symbol)
    }

    pub const fn atom(self) -> Option<AtomId> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::Symbol(_) => None,
        }
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
    pub(in crate::runtime::object) key: PropertyKey,
    pub(in crate::runtime::object) name: &'a str,
    pub(in crate::runtime::object) value: Value,
    pub(in crate::runtime::object) enumerable: PropertyEnumerable,
    kind: ObjectPropertyInitKind,
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
            kind: ObjectPropertyInitKind::Literal,
        }
    }

    pub const fn new_data(
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
            kind: ObjectPropertyInitKind::Data,
        }
    }

    /// A getter or setter half of an accessor property; `value` carries the
    /// accessor function.
    pub const fn new_accessor(
        key: PropertyKey,
        name: &'a str,
        function: Value,
        accessor: AccessorKind,
    ) -> Self {
        Self {
            key,
            name,
            value: function,
            enumerable: PropertyEnumerable::Yes,
            kind: ObjectPropertyInitKind::Accessor(accessor),
        }
    }

    pub(in crate::runtime::object) const fn uses_literal_prototype(&self) -> bool {
        matches!(self.kind, ObjectPropertyInitKind::Literal)
    }

    pub(in crate::runtime::object) const fn accessor_kind(&self) -> Option<AccessorKind> {
        match self.kind {
            ObjectPropertyInitKind::Accessor(kind) => Some(kind),
            ObjectPropertyInitKind::Literal | ObjectPropertyInitKind::Data => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ObjectPropertyInitKind {
    Literal,
    Data,
    Accessor(AccessorKind),
}
