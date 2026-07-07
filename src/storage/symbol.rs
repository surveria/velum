use crate::{
    error::{Error, Result},
    storage::string_heap::JsString,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SymbolId(u32);

impl SymbolId {
    fn from_index(index: usize) -> Result<Self> {
        let id = u32::try_from(index)
            .map_err(|_| Error::limit("symbol table exceeded supported range"))?;
        Ok(Self(id))
    }

    fn index(self) -> Result<usize> {
        usize::try_from(self.0).map_err(|_| Error::limit("symbol id exceeded supported range"))
    }
}

#[derive(Clone, Debug, Eq)]
pub struct JsSymbol {
    id: SymbolId,
    description: Option<JsString>,
}

impl JsSymbol {
    const fn new(id: SymbolId, description: Option<JsString>) -> Self {
        Self { id, description }
    }

    #[must_use]
    pub const fn id(&self) -> SymbolId {
        self.id
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_ref().map(JsString::as_str)
    }

    #[must_use]
    pub fn display_name(&self) -> String {
        self.description().map_or_else(
            || "Symbol()".to_owned(),
            |description| format!("Symbol({description})"),
        )
    }
}

impl PartialEq for JsSymbol {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    entries: Vec<JsSymbol>,
}

impl SymbolTable {
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn create(&mut self, description: Option<JsString>) -> Result<JsSymbol> {
        let id = SymbolId::from_index(self.entries.len())?;
        let symbol = JsSymbol::new(id, description);
        self.entries.push(symbol.clone());
        Ok(symbol)
    }

    pub fn get(&self, id: SymbolId) -> Result<&JsSymbol> {
        self.entries
            .get(id.index()?)
            .ok_or_else(|| Error::runtime("symbol id is not defined"))
    }
}
