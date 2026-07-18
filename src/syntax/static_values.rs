use alloc::rc::Rc;

use super::{StaticBindingId, StaticNameId};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticName {
    id: StaticNameId,
    text: Rc<str>,
}

impl StaticName {
    pub fn borrowed(id: StaticNameId, name: &str) -> Self {
        Self::from_shared(id, Rc::from(name))
    }

    pub(crate) const fn from_shared(id: StaticNameId, text: Rc<str>) -> Self {
        Self { id, text }
    }

    pub const fn id(&self) -> StaticNameId {
        self.id
    }

    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }

    pub(crate) fn shared_text(&self) -> Rc<str> {
        Rc::clone(&self.text)
    }
}

impl core::fmt::Display for StaticName {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl core::ops::Deref for StaticName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticString {
    units: Rc<[u16]>,
    text: Rc<str>,
}

impl StaticString {
    pub(crate) fn from_shared(units: Rc<[u16]>) -> Self {
        let text = String::from_utf16_lossy(&units);
        Self {
            units,
            text: Rc::from(text.into_boxed_str()),
        }
    }

    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }

    pub fn as_utf16(&self) -> &[u16] {
        self.units.as_ref()
    }

    pub(crate) fn shared_units(&self) -> Rc<[u16]> {
        Rc::clone(&self.units)
    }
}

impl core::fmt::Display for StaticString {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl core::ops::Deref for StaticString {
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

impl core::fmt::Display for StaticBinding {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl core::ops::Deref for StaticBinding {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}
