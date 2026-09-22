#[cfg(not(feature = "std"))]
use crate::prelude::*;

use alloc::{collections::BTreeSet, rc::Rc};
use core::{cell::OnceCell, fmt, mem::size_of};
use hashbrown::{HashMap, hash_map::EntryRef};

use crate::{
    error::{Error, Result},
    ownership::VmIdentity,
};

const UTF16_UNIT_BYTES: usize = size_of::<u16>();
const REPLACEMENT_CHARACTER: char = '\u{FFFD}';
const ASCII_MAX_CODE_UNIT: u16 = 0x7F;

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
    data: StringDataRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StringOwner {
    identity: VmIdentity,
    id: StringId,
}

impl JsString {
    const fn new(data: StringDataRef) -> Self {
        Self { data }
    }

    /// Creates a VM-independent JavaScript string from well-formed UTF-8.
    #[must_use]
    pub fn from_utf8(text: &str) -> Self {
        let units = text.encode_utf16().collect::<Vec<_>>();
        Self {
            data: StringDataRef::new(units),
        }
    }

    /// Creates a VM-independent JavaScript string from exact UTF-16 code units.
    #[must_use]
    pub fn from_utf16(units: Vec<u16>) -> Self {
        Self {
            data: StringDataRef::new(units),
        }
    }

    /// Returns the VM owner and storage generation after heap admission.
    #[must_use]
    pub fn identity(&self) -> Option<&VmIdentity> {
        self.data.owner().map(|owner| &owner.identity)
    }

    #[must_use]
    pub fn id(&self) -> Option<StringId> {
        self.data.owner().map(|owner| owner.id)
    }

    /// Returns whether this string has been admitted to a VM string heap.
    #[must_use]
    pub fn is_heap_owned(&self) -> bool {
        self.data.is_heap_owned()
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

    pub(crate) fn shared_utf16(&self) -> Rc<[u16]> {
        self.data.0.payload.0.units.clone()
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
            Ok(data) => match Rc::try_unwrap(data.payload.0) {
                Ok(payload) => payload
                    .text
                    .into_inner()
                    .or_else(|| String::from_utf16(payload.units.as_ref()).ok()),
                Err(payload) => Some(payload.as_str().to_owned()),
            },
            Err(data) => Some(data.payload.as_str().to_owned()),
        }
    }
}

impl From<String> for JsString {
    fn from(value: String) -> Self {
        Self::from_utf8(&value)
    }
}

impl From<&str> for JsString {
    fn from(value: &str) -> Self {
        Self::from_utf8(value)
    }
}

impl PartialEq for JsString {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

#[derive(Debug)]
struct StringData {
    owner: Option<StringOwner>,
    payload: StringPayloadRef,
}

#[derive(Debug)]
struct StringPayload {
    units: Rc<[u16]>,
    /// Lazily materialized UTF-8 for well-formed strings or replacement-text
    /// diagnostics for strings containing lone surrogates.
    text: OnceCell<String>,
    well_formed: bool,
    rendered_bytes: usize,
}

#[derive(Clone, Debug)]
struct StringPayloadRef(Rc<StringPayload>);

#[derive(Clone, Debug)]
struct StringDataRef(Rc<StringData>);

impl StringDataRef {
    fn new(units: Vec<u16>) -> Self {
        Self::from_shared_units(Rc::from(units.into_boxed_slice()))
    }

    fn from_shared_units(units: Rc<[u16]>) -> Self {
        let (well_formed, rendered_bytes) = utf16_rendering_metadata(&units);
        let payload = StringPayloadRef(Rc::new(StringPayload {
            units,
            text: OnceCell::new(),
            well_formed,
            rendered_bytes,
        }));
        Self(Rc::new(StringData {
            owner: None,
            payload,
        }))
    }

    fn with_owner(&self, identity: VmIdentity, id: StringId) -> Self {
        Self(Rc::new(StringData {
            owner: Some(StringOwner { identity, id }),
            payload: self.0.payload.clone(),
        }))
    }

    fn owner(&self) -> Option<&StringOwner> {
        self.0.owner.as_ref()
    }

    fn is_heap_owned(&self) -> bool {
        self.0.owner.is_some()
    }

    fn as_str(&self) -> &str {
        self.0.payload.as_str()
    }

    fn as_utf16(&self) -> &[u16] {
        self.0.payload.0.units.as_ref()
    }

    fn is_well_formed(&self) -> bool {
        self.0.payload.0.well_formed
    }

    fn storage_bytes(&self) -> Result<usize> {
        let utf16_bytes = self
            .as_utf16()
            .len()
            .checked_mul(UTF16_UNIT_BYTES)
            .ok_or_else(|| Error::limit("string UTF-16 byte count overflowed"))?;
        utf16_bytes
            .checked_add(self.0.payload.0.rendered_bytes)
            .ok_or_else(|| Error::limit("string payload byte count overflowed"))
    }
}

impl StringPayload {
    fn as_str(&self) -> &str {
        self.text.get_or_init(|| {
            String::from_utf16(self.units.as_ref())
                .unwrap_or_else(|_| String::from_utf16_lossy(self.units.as_ref()))
        })
    }
}

impl StringPayloadRef {
    fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn utf16_rendering_metadata(units: &[u16]) -> (bool, usize) {
    // The reduction can be vectorized without a per-code-unit decode branch.
    // ASCII needs one rendering byte per exact UTF-16 code unit, including NUL.
    if units.iter().fold(0, |bits, unit| bits | unit) <= ASCII_MAX_CODE_UNIT {
        return (true, units.len());
    }
    let mut well_formed = true;
    let mut rendered_bytes = 0_usize;
    for decoded in char::decode_utf16(units.iter().copied()) {
        let width = decoded.map_or_else(
            |_| {
                well_formed = false;
                REPLACEMENT_CHARACTER.len_utf8()
            },
            char::len_utf8,
        );
        rendered_bytes = rendered_bytes.saturating_add(width);
    }
    (well_formed, rendered_bytes)
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
    records: StringRecords,
}

#[derive(Debug, Clone)]
struct StringRecords {
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
            records: StringRecords {
                strings: Vec::new(),
                free: Vec::new(),
                live: 0,
                bytes: 0,
                max_count,
                max_bytes,
            },
        }
    }

    pub const fn len(&self) -> usize {
        self.records.live
    }

    pub const fn bytes(&self) -> usize {
        self.records.bytes
    }

    pub(crate) fn index_entry_count(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn intern<R>(
        &mut self,
        text: &str,
        reserve: impl FnOnce() -> Result<R>,
    ) -> Result<(JsString, Option<R>)> {
        let units = text.encode_utf16().collect::<Vec<_>>();
        self.intern_utf16(&units, reserve)
    }

    pub(crate) fn intern_js_string<R>(
        &mut self,
        string: &JsString,
        reserve: impl FnOnce() -> Result<R>,
    ) -> Result<(JsString, Option<R>)> {
        self.intern_data(string.as_utf16(), || string.data.clone(), reserve)
    }

    pub(crate) fn intern_utf16<R>(
        &mut self,
        units: &[u16],
        reserve: impl FnOnce() -> Result<R>,
    ) -> Result<(JsString, Option<R>)> {
        self.intern_data(
            units,
            || StringDataRef::from_shared_units(Rc::from(units)),
            reserve,
        )
    }

    // The reservation is created only for a vacant entry. An insertion error
    // drops it; the caller commits it only after admission succeeds. Keeping the
    // entry borrowed also avoids rehashing the same text to publish the index.
    fn intern_data<R>(
        &mut self,
        units: &[u16],
        make_data: impl FnOnce() -> StringDataRef,
        reserve: impl FnOnce() -> Result<R>,
    ) -> Result<(JsString, Option<R>)> {
        match self.entries.entry_ref(units) {
            EntryRef::Occupied(entry) => {
                let string = self.records.js_string(*entry.get())?;
                Ok((string, None))
            }
            EntryRef::Vacant(entry) => {
                let reservation = reserve()?;
                let (id, data) = self.records.insert_data(&make_data(), &self.identity)?;
                // Both producers above retain exactly the queried code units.
                // No caller can supply a different key for this vacant entry.
                entry.insert_with_key(data.0.payload.0.units.clone(), id);
                Ok((JsString::new(data), Some(reservation)))
            }
        }
    }

    pub(crate) fn validate_id(&self, id: StringId) -> Result<()> {
        self.records.string_data(id).map(|_data| ())
    }

    pub(crate) fn sweep_unmarked(&mut self, marked: &BTreeSet<StringId>) -> Result<usize> {
        self.records.sweep_unmarked(marked, &mut self.entries)
    }
}

impl StringRecords {
    fn string_data(&self, id: StringId) -> Result<&StringDataRef> {
        self.strings
            .get(id.index()?)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::runtime("string id is not defined"))
    }

    fn js_string(&self, id: StringId) -> Result<JsString> {
        self.string_data(id).cloned().map(JsString::new)
    }

    fn insert_data(
        &mut self,
        data: &StringDataRef,
        identity: &VmIdentity,
    ) -> Result<(StringId, StringDataRef)> {
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
        let data = data.with_owner(identity.clone(), id);
        let updated_bytes = self
            .bytes
            .checked_add(data.storage_bytes()?)
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
        self.live = self
            .live
            .checked_add(1)
            .ok_or_else(|| Error::limit("string heap live count overflowed"))?;
        self.bytes = updated_bytes;
        Ok((id, data))
    }

    fn sweep_unmarked(
        &mut self,
        marked: &BTreeSet<StringId>,
        entries: &mut HashMap<Rc<[u16]>, StringId>,
    ) -> Result<usize> {
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
                .checked_sub(data.storage_bytes()?)
                .ok_or_else(|| Error::runtime("string heap byte count underflowed"))?;
            let removed_id = entries.remove(data.as_utf16());
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
