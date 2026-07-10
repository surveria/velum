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
    entries: Vec<AtomEntry>,
    names: Vec<String>,
    bytes: usize,
    max_count: usize,
    max_bytes: usize,
}

impl AtomTable {
    pub const fn new(max_count: usize, max_bytes: usize) -> Self {
        Self {
            entries: Vec::new(),
            names: Vec::new(),
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

    pub(crate) const fn index_entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn intern(&mut self, name: &str) -> Result<AtomId> {
        let position = self.atom_position(name);
        let position = match position {
            Ok(position) => {
                return self
                    .entries
                    .get(position)
                    .map(AtomEntry::id)
                    .ok_or_else(|| Error::runtime("atom index entry is not available"));
            }
            Err(position) => position,
        };

        let id = AtomId::from_index(self.names.len())?;
        if self.names.len() >= self.max_count {
            return Err(Error::limit(format!(
                "Atom record count exceeded {}",
                self.max_count
            )));
        }
        if position > self.entries.len() {
            return Err(Error::runtime("atom index insert position is out of range"));
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
        let name = name.to_owned();
        self.names.push(name.clone());
        self.entries.insert(position, AtomEntry::new(name, id));
        self.bytes = updated_bytes;
        Ok(id)
    }

    pub fn get(&self, name: &str) -> Option<AtomId> {
        let position = self.atom_position(name).ok()?;
        self.entries.get(position).map(AtomEntry::id)
    }

    pub fn name(&self, id: AtomId) -> Result<&str> {
        self.names
            .get(id.index()?)
            .map(String::as_str)
            .ok_or_else(|| Error::runtime("atom id is not defined"))
    }

    fn atom_position(&self, name: &str) -> std::result::Result<usize, usize> {
        self.entries
            .binary_search_by(|entry| entry.name().cmp(name))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct AtomEntry {
    name: String,
    id: AtomId,
}

impl AtomEntry {
    const fn new(name: String, id: AtomId) -> Self {
        Self { name, id }
    }

    const fn name(&self) -> &str {
        self.name.as_str()
    }

    const fn id(&self) -> AtomId {
        self.id
    }
}
