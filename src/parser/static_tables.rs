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
    index: Vec<StaticStringIndexEntry>,
}

impl StaticStringTable {
    pub(super) const fn new() -> Self {
        Self {
            strings: Vec::new(),
            index: Vec::new(),
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.strings.len()
    }

    pub(super) fn intern_owned(&mut self, value: String, offset: usize) -> Result<StaticString> {
        let position = match self.static_string_position(&value) {
            Ok(position) => return self.static_string_at_index_position(position, offset),
            Err(position) => position,
        };
        if position > self.index.len() {
            return Err(Error::parse(
                "static string insert position is out of range",
                offset,
            ));
        }
        let id = StaticStringId::from_index(self.strings.len())?;
        let value = StaticString::new(id, value);
        self.strings.push(value.clone());
        self.index
            .insert(position, StaticStringIndexEntry::new(value.clone()));
        Ok(value)
    }

    fn static_string_at_index_position(
        &self,
        position: usize,
        offset: usize,
    ) -> Result<StaticString> {
        let entry = self
            .index
            .get(position)
            .ok_or_else(|| Error::parse("static string index entry is not available", offset))?;
        self.strings
            .get(entry.id().index()?)
            .cloned()
            .ok_or_else(|| Error::parse("static string id is not defined", offset))
    }

    fn static_string_position(&self, value: &str) -> std::result::Result<usize, usize> {
        self.index
            .binary_search_by(|entry| entry.as_str().cmp(value))
    }
}

#[derive(Debug, Clone)]
struct StaticStringIndexEntry {
    value: StaticString,
}

impl StaticStringIndexEntry {
    const fn new(value: StaticString) -> Self {
        Self { value }
    }

    fn as_str(&self) -> &str {
        self.value.as_str()
    }

    const fn id(&self) -> StaticStringId {
        self.value.id()
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct StaticNameTable {
    names: Vec<StaticName>,
    index: Vec<StaticNameIndexEntry>,
}

impl StaticNameTable {
    pub(super) const fn new() -> Self {
        Self {
            names: Vec::new(),
            index: Vec::new(),
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.names.len()
    }

    pub(super) fn intern_owned(&mut self, name: String, offset: usize) -> Result<StaticName> {
        let position = match self.static_name_position(&name) {
            Ok(position) => return self.static_name_at_index_position(position, offset),
            Err(position) => position,
        };
        self.insert(name, position, offset)
    }

    pub(super) fn intern_borrowed(&mut self, name: &str, offset: usize) -> Result<StaticName> {
        let position = match self.static_name_position(name) {
            Ok(position) => return self.static_name_at_index_position(position, offset),
            Err(position) => position,
        };
        if position > self.index.len() {
            return Err(Error::parse(
                "static name insert position is out of range",
                offset,
            ));
        }
        let id = StaticNameId::from_index(self.names.len())?;
        let name = StaticName::borrowed(id, name);
        self.remember_name(name, position);
        self.names
            .last()
            .cloned()
            .ok_or_else(|| Error::parse("static name insert failed", offset))
    }

    fn insert(&mut self, name: String, position: usize, offset: usize) -> Result<StaticName> {
        if position > self.index.len() {
            return Err(Error::parse(
                "static name insert position is out of range",
                offset,
            ));
        }
        let id = StaticNameId::from_index(self.names.len())?;
        let name = StaticName::new(id, name);
        self.remember_name(name.clone(), position);
        Ok(name)
    }

    fn remember_name(&mut self, name: StaticName, position: usize) {
        self.names.push(name.clone());
        self.index.insert(position, StaticNameIndexEntry::new(name));
    }

    fn static_name_at_index_position(&self, position: usize, offset: usize) -> Result<StaticName> {
        let entry = self
            .index
            .get(position)
            .ok_or_else(|| Error::parse("static name index entry is not available", offset))?;
        self.names
            .get(entry.id().index()?)
            .cloned()
            .ok_or_else(|| Error::parse("static name id is not defined", offset))
    }

    fn static_name_position(&self, name: &str) -> std::result::Result<usize, usize> {
        self.index
            .binary_search_by(|entry| entry.as_str().cmp(name))
    }
}

#[derive(Debug, Clone)]
struct StaticNameIndexEntry {
    name: StaticName,
}

impl StaticNameIndexEntry {
    const fn new(name: StaticName) -> Self {
        Self { name }
    }

    fn as_str(&self) -> &str {
        self.name.as_str()
    }

    const fn id(&self) -> StaticNameId {
        self.name.id()
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
}
