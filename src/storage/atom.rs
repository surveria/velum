#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::rc::Rc;
use hashbrown::HashMap;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd)]
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
    names: Vec<Rc<str>>,
    index: HashMap<Rc<str>, AtomId>,
    bytes: usize,
    max_count: usize,
    max_bytes: usize,
}

impl AtomTable {
    pub fn new(max_count: usize, max_bytes: usize) -> Self {
        Self {
            names: Vec::new(),
            index: HashMap::new(),
            bytes: 0,
            max_count,
            max_bytes,
        }
    }

    pub const fn len(&self) -> usize {
        self.names.len()
    }

    pub(crate) const fn bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn index_entry_count(&self) -> usize {
        self.index.len()
    }

    pub fn intern(&mut self, name: &str) -> Result<AtomId> {
        if let Some(id) = self.index.get(name) {
            return Ok(*id);
        }

        let id = AtomId::from_index(self.names.len())?;
        if self.names.len() >= self.max_count {
            return Err(Error::limit(format!(
                "Atom record count exceeded {}",
                self.max_count
            )));
        }
        let updated_bytes = self
            .bytes
            .checked_add(name.len())
            .ok_or_else(|| Error::limit("atom table byte count overflowed"))?;
        if updated_bytes > self.max_bytes {
            return Err(Error::limit(format!(
                "Atom payload bytes exceeded {}",
                self.max_bytes
            )));
        }
        self.names
            .try_reserve(1)
            .map_err(|error| Error::limit(format!("atom name allocation failed: {error:?}")))?;
        self.index
            .try_reserve(1)
            .map_err(|error| Error::limit(format!("atom index allocation failed: {error:?}")))?;
        let name: Rc<str> = Rc::from(name);
        self.names.push(Rc::clone(&name));
        self.index.insert(name, id);
        self.bytes = updated_bytes;
        Ok(id)
    }

    pub fn get(&self, name: &str) -> Option<AtomId> {
        self.index.get(name).copied()
    }

    pub fn name(&self, id: AtomId) -> Result<&str> {
        self.names
            .get(id.index()?)
            .map(Rc::as_ref)
            .ok_or_else(|| Error::runtime("atom id is not defined"))
    }
}
