use std::rc::Rc;

use super::{StaticBindingId, StaticNameId, StaticStringId};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticName {
    id: StaticNameId,
    text: Rc<str>,
}

impl StaticName {
    pub fn new(id: StaticNameId, name: String) -> Self {
        Self {
            id,
            text: Rc::from(name.into_boxed_str()),
        }
    }

    pub fn borrowed(id: StaticNameId, name: &str) -> Self {
        Self {
            id,
            text: Rc::from(name),
        }
    }

    pub const fn id(&self) -> StaticNameId {
        self.id
    }

    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }
}

impl std::fmt::Display for StaticName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::ops::Deref for StaticName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticString {
    id: StaticStringId,
    units: Rc<[u16]>,
    text: Rc<str>,
}

impl StaticString {
    pub fn new(id: StaticStringId, value: Vec<u16>) -> Self {
        let text = String::from_utf16_lossy(&value);
        Self {
            id,
            units: Rc::from(value.into_boxed_slice()),
            text: Rc::from(text.into_boxed_str()),
        }
    }

    pub const fn id(&self) -> StaticStringId {
        self.id
    }

    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }

    pub fn as_utf16(&self) -> &[u16] {
        self.units.as_ref()
    }
}

impl std::fmt::Display for StaticString {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::ops::Deref for StaticString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticBinding {
    id: StaticBindingId,
    name: StaticName,
}

impl StaticBinding {
    pub const fn new(id: StaticBindingId, name: StaticName) -> Self {
        Self { id, name }
    }

    pub const fn id(&self) -> StaticBindingId {
        self.id
    }

    pub const fn name(&self) -> &StaticName {
        &self.name
    }

    pub fn as_str(&self) -> &str {
        self.name.as_str()
    }
}

impl std::fmt::Display for StaticBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::ops::Deref for StaticBinding {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
