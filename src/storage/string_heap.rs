use std::{
    borrow::Borrow,
    collections::HashMap,
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
};

use crate::{
    error::{Error, Result},
    ownership::VmIdentity,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct StringId(u32);

impl StringId {
    fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("string heap exceeded supported range"))?;
        Ok(Self(id))
    }

    fn index(self) -> Result<usize> {
        usize::try_from(self.0).map_err(|_| Error::limit("string id exceeded supported range"))
    }
}

#[derive(Clone, Debug, Eq)]
pub struct JsString {
    id: StringId,
    data: StringDataRef,
}

impl JsString {
    const fn new(id: StringId, data: StringDataRef) -> Self {
        Self { id, data }
    }

    /// Returns the VM owner and storage generation of this heap string.
    #[must_use]
    pub fn identity(&self) -> &VmIdentity {
        &self.data.0.identity
    }

    #[must_use]
    pub const fn id(&self) -> StringId {
        self.id
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.data.as_str()
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.data.as_str().to_owned()
    }
}

impl PartialEq for JsString {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

#[derive(Debug)]
struct StringData {
    identity: VmIdentity,
    text: String,
}

#[derive(Clone, Debug)]
struct StringDataRef(Rc<StringData>);

impl StringDataRef {
    fn new(identity: VmIdentity, text: String) -> Self {
        Self(Rc::new(StringData { identity, text }))
    }

    fn as_str(&self) -> &str {
        self.0.text.as_str()
    }
}

impl Borrow<str> for StringDataRef {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Hash for StringDataRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialEq for StringDataRef {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for StringDataRef {}

impl fmt::Display for JsString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct StringHeap {
    identity: VmIdentity,
    entries: HashMap<StringDataRef, StringId>,
    strings: Vec<StringDataRef>,
    bytes: usize,
    max_count: usize,
    max_bytes: usize,
}

impl StringHeap {
    pub fn new(identity: VmIdentity, max_count: usize, max_bytes: usize) -> Self {
        Self {
            identity,
            entries: HashMap::new(),
            strings: Vec::new(),
            bytes: 0,
            max_count,
            max_bytes,
        }
    }

    pub const fn len(&self) -> usize {
        self.strings.len()
    }

    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn index_entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn intern(&mut self, text: &str) -> Result<JsString> {
        if let Some(id) = self.entries.get(text).copied() {
            return self.js_string(id);
        }
        self.insert_string(text.to_owned())
    }

    pub fn intern_owned(&mut self, text: String) -> Result<JsString> {
        if let Some(id) = self.entries.get(text.as_str()).copied() {
            return self.js_string(id);
        }
        self.insert_string(text)
    }

    pub fn get(&self, id: StringId) -> Result<&str> {
        self.strings
            .get(id.index()?)
            .map(StringDataRef::as_str)
            .ok_or_else(|| Error::runtime("string id is not defined"))
    }

    fn js_string(&self, id: StringId) -> Result<JsString> {
        let data = self
            .strings
            .get(id.index()?)
            .cloned()
            .ok_or_else(|| Error::runtime("string id is not defined"))?;
        Ok(JsString::new(id, data))
    }

    fn insert_string(&mut self, text: String) -> Result<JsString> {
        if self.strings.len() >= self.max_count {
            return Err(Error::limit(format!(
                "HeapString record count exceeded {}",
                self.max_count
            )));
        }
        let id = StringId::from_index(self.strings.len())?;
        let updated_bytes = self
            .bytes
            .checked_add(text.len())
            .ok_or_else(|| Error::limit("string heap byte count overflowed"))?;
        if updated_bytes > self.max_bytes {
            return Err(Error::limit(format!(
                "HeapString payload bytes exceeded {}",
                self.max_bytes
            )));
        }
        let data = StringDataRef::new(self.identity.clone(), text);
        self.strings.push(data.clone());
        self.entries.insert(data, id);
        self.bytes = updated_bytes;
        self.js_string(id)
    }
}
