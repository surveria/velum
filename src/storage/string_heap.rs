use std::{
    collections::{BTreeSet, HashMap},
    fmt,
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
    owner: Option<StringOwner>,
    data: StringDataRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StringOwner {
    identity: VmIdentity,
    id: StringId,
}

impl JsString {
    const fn new_owned(identity: VmIdentity, id: StringId, data: StringDataRef) -> Self {
        Self {
            owner: Some(StringOwner { identity, id }),
            data,
        }
    }

    /// Creates a VM-independent JavaScript string from well-formed UTF-8.
    #[must_use]
    pub fn from_utf8(text: String) -> Self {
        let units = text.encode_utf16().collect::<Vec<_>>();
        Self {
            owner: None,
            data: StringDataRef::new(units, Some(text)),
        }
    }

    /// Creates a VM-independent JavaScript string from exact UTF-16 code units.
    #[must_use]
    pub fn from_utf16(units: Vec<u16>) -> Self {
        Self {
            owner: None,
            data: StringDataRef::new(units, None),
        }
    }

    /// Returns the VM owner and storage generation after heap admission.
    #[must_use]
    pub fn identity(&self) -> Option<&VmIdentity> {
        self.owner.as_ref().map(|owner| &owner.identity)
    }

    #[must_use]
    pub fn id(&self) -> Option<StringId> {
        self.owner.as_ref().map(|owner| owner.id)
    }

    /// Returns whether this string has been admitted to a VM string heap.
    #[must_use]
    pub const fn is_heap_owned(&self) -> bool {
        self.owner.is_some()
    }

    /// Returns UTF-8 text, replacing lone surrogates with U+FFFD.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.data.as_str()
    }

    /// Returns a lossless UTF-8 view when the code-unit sequence is well-formed.
    #[must_use]
    pub fn as_utf8(&self) -> Option<&str> {
        self.is_well_formed().then_some(self.data.as_str())
    }

    /// Returns the exact ECMAScript UTF-16 code-unit sequence.
    #[must_use]
    pub fn as_utf16(&self) -> &[u16] {
        self.data.as_utf16()
    }

    /// Returns whether this value can be represented losslessly as UTF-8.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.data.is_well_formed()
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.data.as_str().to_owned()
    }

    /// Converts into UTF-8 when the code-unit sequence is well-formed.
    #[must_use]
    pub fn into_utf8(self) -> Option<String> {
        self.is_well_formed().then(|| self.data.as_str().to_owned())
    }

    pub(crate) fn into_utf8_accumulator(self) -> Option<String> {
        if !self.is_well_formed() {
            return None;
        }
        match Rc::try_unwrap(self.data.0) {
            Ok(data) => Some(data.text),
            Err(data) => Some(data.text.clone()),
        }
    }
}

impl From<String> for JsString {
    fn from(value: String) -> Self {
        Self::from_utf8(value)
    }
}

impl From<&str> for JsString {
    fn from(value: &str) -> Self {
        Self::from_utf8(value.to_owned())
    }
}

impl PartialEq for JsString {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

#[derive(Debug)]
struct StringData {
    units: Rc<[u16]>,
    /// UTF-8 for well-formed strings and replacement-character rendering for
    /// strings containing lone surrogates. JavaScript semantics use `units`.
    text: String,
    well_formed: bool,
}

#[derive(Clone, Debug)]
struct StringDataRef(Rc<StringData>);

impl StringDataRef {
    fn new(units: Vec<u16>, utf8: Option<String>) -> Self {
        let decoded = String::from_utf16(&units);
        let (text, well_formed) = decoded.map_or_else(
            |_| (String::from_utf16_lossy(&units), false),
            |text| (utf8.unwrap_or(text), true),
        );
        Self(Rc::new(StringData {
            units: Rc::from(units.into_boxed_slice()),
            text,
            well_formed,
        }))
    }

    fn as_str(&self) -> &str {
        self.0.text.as_str()
    }

    fn as_utf16(&self) -> &[u16] {
        self.0.units.as_ref()
    }

    fn is_well_formed(&self) -> bool {
        self.0.well_formed
    }

    fn storage_bytes(&self) -> usize {
        self.0.text.len()
    }
}

impl PartialEq for StringDataRef {
    fn eq(&self, other: &Self) -> bool {
        self.as_utf16() == other.as_utf16()
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
    entries: HashMap<Rc<[u16]>, StringId>,
    strings: Vec<Option<StringDataRef>>,
    free: Vec<usize>,
    live: usize,
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
            free: Vec::new(),
            live: 0,
            bytes: 0,
            max_count,
            max_bytes,
        }
    }

    pub const fn len(&self) -> usize {
        self.live
    }

    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn index_entry_count(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn contains(&self, text: &str) -> bool {
        let units = text.encode_utf16().collect::<Vec<_>>();
        self.contains_utf16(&units)
    }

    pub(crate) fn contains_utf16(&self, units: &[u16]) -> bool {
        self.entries.contains_key(units)
    }

    pub fn intern(&mut self, text: &str) -> Result<JsString> {
        self.intern_owned(text.to_owned())
    }

    pub fn intern_owned(&mut self, text: String) -> Result<JsString> {
        let units = text.encode_utf16().collect::<Vec<_>>();
        if let Some(id) = self.entries.get(units.as_slice()).copied() {
            return self.js_string(id);
        }
        self.insert_string(units, Some(text))
    }

    pub(crate) fn intern_js_string(&mut self, string: &JsString) -> Result<JsString> {
        if let Some(id) = self.entries.get(string.as_utf16()).copied() {
            return self.js_string(id);
        }
        self.insert_data(&string.data)
    }

    pub fn intern_utf16(&mut self, units: &[u16]) -> Result<JsString> {
        if let Some(id) = self.entries.get(units).copied() {
            return self.js_string(id);
        }
        self.insert_string(units.to_vec(), None)
    }

    pub fn get(&self, id: StringId) -> Result<&str> {
        self.strings
            .get(id.index()?)
            .and_then(Option::as_ref)
            .map(StringDataRef::as_str)
            .ok_or_else(|| Error::runtime("string id is not defined"))
    }

    fn js_string(&self, id: StringId) -> Result<JsString> {
        let data = self
            .strings
            .get(id.index()?)
            .and_then(Option::as_ref)
            .cloned()
            .ok_or_else(|| Error::runtime("string id is not defined"))?;
        Ok(JsString::new_owned(self.identity.clone(), id, data))
    }

    fn insert_string(&mut self, units: Vec<u16>, utf8: Option<String>) -> Result<JsString> {
        self.insert_data(&StringDataRef::new(units, utf8))
    }

    fn insert_data(&mut self, data: &StringDataRef) -> Result<JsString> {
        if self.live >= self.max_count {
            return Err(Error::limit(format!(
                "HeapString record count exceeded {}",
                self.max_count
            )));
        }
        let index = self.free.last().copied().unwrap_or(self.strings.len());
        if self.free.is_empty() {
            self.strings
                .try_reserve(1)
                .map_err(|error| Error::limit(format!("string heap exhausted: {error}")))?;
        }
        let id = StringId::from_index(index)?;
        let updated_bytes = self
            .bytes
            .checked_add(data.storage_bytes())
            .ok_or_else(|| Error::limit("string heap byte count overflowed"))?;
        if updated_bytes > self.max_bytes {
            return Err(Error::limit(format!(
                "HeapString payload bytes exceeded {}",
                self.max_bytes
            )));
        }
        if self.free.pop().is_some() {
            let Some(slot) = self.strings.get_mut(index) else {
                return Err(Error::runtime("string heap free slot is not defined"));
            };
            if slot.replace(data.clone()).is_some() {
                return Err(Error::runtime("string heap free slot is occupied"));
            }
        } else {
            self.strings.push(Some(data.clone()));
        }
        self.entries.insert(data.0.units.clone(), id);
        self.live = self
            .live
            .checked_add(1)
            .ok_or_else(|| Error::limit("string heap live count overflowed"))?;
        self.bytes = updated_bytes;
        self.js_string(id)
    }

    pub(crate) fn sweep_unmarked(&mut self, marked: &BTreeSet<StringId>) -> Result<usize> {
        let mut removed = 0_usize;
        for (index, slot) in self.strings.iter().enumerate() {
            let id = StringId::from_index(index)?;
            if slot.is_some() && !marked.contains(&id) {
                removed = removed
                    .checked_add(1)
                    .ok_or_else(|| Error::limit("string sweep count overflowed"))?;
            }
        }
        self.free
            .try_reserve(removed)
            .map_err(|error| Error::limit(format!("string free list exhausted: {error}")))?;
        for index in 0..self.strings.len() {
            let id = StringId::from_index(index)?;
            if marked.contains(&id) {
                continue;
            }
            let Some(data) = self.strings.get_mut(index).and_then(Option::take) else {
                continue;
            };
            self.bytes = self
                .bytes
                .checked_sub(data.storage_bytes())
                .ok_or_else(|| Error::runtime("string heap byte count underflowed"))?;
            let removed_id = self.entries.remove(data.as_utf16());
            if removed_id != Some(id) {
                return Err(Error::runtime("string heap index removal mismatch"));
            }
            self.free.push(index);
        }
        self.live = self
            .live
            .checked_sub(removed)
            .ok_or_else(|| Error::runtime("string heap live count underflowed"))?;
        Ok(removed)
    }
}
