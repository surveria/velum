use std::{collections::HashMap, rc::Rc};

use crate::{
    error::{Error, Result},
    syntax::{
        StaticBinding, StaticBindingId, StaticFunctionId, StaticName, StaticNameId, StaticString,
        StaticStringId,
    },
};

#[derive(Debug, Clone, Default)]
pub(super) struct StaticStringTable {
    strings: Vec<StaticString>,
    index: HashMap<Rc<[u16]>, StaticStringId>,
}

impl StaticStringTable {
    pub(super) fn new() -> Self {
        Self {
            strings: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.strings.len()
    }

    pub(super) fn intern_owned(&mut self, value: Vec<u16>, offset: usize) -> Result<StaticString> {
        if let Some(id) = self.index.get(value.as_slice()).copied() {
            return self.static_string(id, offset);
        }
        let id = StaticStringId::from_index(self.strings.len())?;
        let value = StaticString::new(value);
        self.strings.push(value.clone());
        self.index.insert(value.shared_units(), id);
        Ok(value)
    }

    fn static_string(&self, id: StaticStringId, offset: usize) -> Result<StaticString> {
        self.strings
            .get(id.index()?)
            .cloned()
            .ok_or_else(|| Error::parse("static string id is not defined", offset))
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct StaticNameTable {
    names: Vec<StaticName>,
    index: HashMap<Rc<str>, StaticNameId>,
}

impl StaticNameTable {
    pub(super) fn new() -> Self {
        Self {
            names: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.names.len()
    }

    pub(super) fn intern_owned(&mut self, name: String, offset: usize) -> Result<StaticName> {
        if let Some(id) = self.index.get(name.as_str()).copied() {
            return self.static_name(id, offset);
        }
        self.insert_owned(name)
    }

    pub(super) fn intern_borrowed(&mut self, name: &str, offset: usize) -> Result<StaticName> {
        if let Some(id) = self.index.get(name).copied() {
            return self.static_name(id, offset);
        }
        let id = StaticNameId::from_index(self.names.len())?;
        let name = StaticName::borrowed(id, name);
        self.remember_name(name.clone());
        Ok(name)
    }

    fn insert_owned(&mut self, name: String) -> Result<StaticName> {
        let id = StaticNameId::from_index(self.names.len())?;
        let name = StaticName::new(id, name);
        self.remember_name(name.clone());
        Ok(name)
    }

    fn remember_name(&mut self, name: StaticName) {
        self.index.insert(name.shared_text(), name.id());
        self.names.push(name);
    }

    fn static_name(&self, id: StaticNameId, offset: usize) -> Result<StaticName> {
        self.names
            .get(id.index()?)
            .cloned()
            .ok_or_else(|| Error::parse("static name id is not defined", offset))
    }

    pub(super) fn rollback_to(&mut self, len: usize, offset: usize) -> Result<()> {
        if len > self.names.len() {
            return Err(Error::parse("static name rollback is out of range", offset));
        }
        while self.names.len() > len {
            let Some(name) = self.names.pop() else {
                return Err(Error::parse("static name rollback failed", offset));
            };
            if self.index.remove(name.as_str()).is_none() {
                return Err(Error::parse(
                    "static name index entry is not defined",
                    offset,
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct StaticBindingTable {
    count: usize,
}

impl StaticBindingTable {
    pub(super) const fn new() -> Self {
        Self { count: 0 }
    }

    pub(super) const fn len(&self) -> usize {
        self.count
    }

    pub(super) fn intern(&mut self, name: StaticName) -> Result<StaticBinding> {
        let id = StaticBindingId::from_index(self.count)?;
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| Error::limit("static binding count overflowed"))?;
        Ok(StaticBinding::new(id, name))
    }

    pub(super) fn rollback_to(&mut self, count: usize, offset: usize) -> Result<()> {
        if count > self.count {
            return Err(Error::parse(
                "static binding rollback is out of range",
                offset,
            ));
        }
        self.count = count;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct StaticFunctionTable {
    count: usize,
}

impl StaticFunctionTable {
    pub(super) const fn new() -> Self {
        Self { count: 0 }
    }

    pub(super) const fn len(&self) -> usize {
        self.count
    }

    pub(super) fn intern(&mut self) -> Result<StaticFunctionId> {
        let id = StaticFunctionId::from_index(self.count)?;
        self.count = self
            .count
            .checked_add(1)
            .ok_or_else(|| Error::limit("static function count overflowed"))?;
        Ok(id)
    }

    pub(super) fn rollback_to(&mut self, count: usize, offset: usize) -> Result<()> {
        if count > self.count {
            return Err(Error::parse(
                "static function rollback is out of range",
                offset,
            ));
        }
        self.count = count;
        Ok(())
    }
}
