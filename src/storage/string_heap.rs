use std::{fmt, rc::Rc};

use crate::error::{Error, Result};

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
    text: Rc<str>,
}

impl JsString {
    const fn new(id: StringId, text: Rc<str>) -> Self {
        Self { id, text }
    }

    #[must_use]
    pub const fn id(&self) -> StringId {
        self.id
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.text.as_ref().to_owned()
    }
}

impl PartialEq for JsString {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl fmt::Display for JsString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub struct StringHeap {
    entries: Vec<StringEntry>,
    strings: Vec<Rc<str>>,
    bytes: usize,
}

impl StringHeap {
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            strings: Vec::new(),
            bytes: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.strings.len()
    }

    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn intern(&mut self, text: &str) -> Result<JsString> {
        let position = self.string_position(text);
        let position = match position {
            Ok(position) => {
                let id = self
                    .entries
                    .get(position)
                    .map(StringEntry::id)
                    .ok_or_else(|| Error::runtime("string heap index entry is not available"))?;
                return self.js_string(id);
            }
            Err(position) => position,
        };

        let id = StringId::from_index(self.strings.len())?;
        if position > self.entries.len() {
            return Err(Error::runtime(
                "string heap insert position is out of range",
            ));
        }
        let text: Rc<str> = Rc::from(text);
        self.bytes = self
            .bytes
            .checked_add(text.len())
            .ok_or_else(|| Error::limit("string heap byte count overflowed"))?;
        self.strings.push(Rc::clone(&text));
        self.entries.insert(position, StringEntry::new(text, id));
        self.js_string(id)
    }

    pub fn get(&self, id: StringId) -> Result<&str> {
        self.strings
            .get(id.index()?)
            .map(AsRef::as_ref)
            .ok_or_else(|| Error::runtime("string id is not defined"))
    }

    fn js_string(&self, id: StringId) -> Result<JsString> {
        let text = self
            .strings
            .get(id.index()?)
            .map(Rc::clone)
            .ok_or_else(|| Error::runtime("string id is not defined"))?;
        Ok(JsString::new(id, text))
    }

    fn string_position(&self, text: &str) -> std::result::Result<usize, usize> {
        self.entries
            .binary_search_by(|entry| entry.text().cmp(text))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct StringEntry {
    text: Rc<str>,
    id: StringId,
}

impl StringEntry {
    const fn new(text: Rc<str>, id: StringId) -> Self {
        Self { text, id }
    }

    fn text(&self) -> &str {
        self.text.as_ref()
    }

    const fn id(&self) -> StringId {
        self.id
    }
}
