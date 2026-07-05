use std::collections::BTreeMap;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct AtomId(u32);

impl AtomId {
    fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("atom table exceeded supported range"))?;
        Ok(Self(id))
    }

    fn index(self) -> Result<usize> {
        usize::try_from(self.0).map_err(|_| Error::limit("atom id exceeded supported range"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct AtomTable {
    ids: BTreeMap<String, AtomId>,
    names: Vec<String>,
}

impl AtomTable {
    pub const fn new() -> Self {
        Self {
            ids: BTreeMap::new(),
            names: Vec::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.names.len()
    }

    pub fn intern(&mut self, name: &str) -> Result<AtomId> {
        if let Some(id) = self.ids.get(name) {
            return Ok(*id);
        }

        let id = AtomId::from_index(self.names.len())?;
        let name = name.to_owned();
        self.names.push(name.clone());
        let previous = self.ids.insert(name, id);
        if previous.is_some() {
            return Err(Error::runtime("atom table insert raced with existing atom"));
        }
        Ok(id)
    }

    pub fn get(&self, name: &str) -> Option<AtomId> {
        self.ids.get(name).copied()
    }

    pub fn name(&self, id: AtomId) -> Result<&str> {
        self.names
            .get(id.index()?)
            .map(String::as_str)
            .ok_or_else(|| Error::runtime("atom id is not defined"))
    }
}
