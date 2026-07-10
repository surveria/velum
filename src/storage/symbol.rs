use crate::{
    error::{Error, Result},
    ownership::VmIdentity,
    storage::string_heap::JsString,
};
use std::rc::Rc;

const FOREIGN_SYMBOL_DESCRIPTION_ERROR: &str = "Symbol description belongs to another VM";
const FOREIGN_SYMBOL_REGISTRY_KEY_ERROR: &str = "Symbol registry key belongs to another VM";

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
    data: Rc<SymbolData>,
}

impl JsSymbol {
    fn new(identity: VmIdentity, id: SymbolId, description: Option<JsString>) -> Self {
        Self {
            id,
            data: Rc::new(SymbolData {
                identity,
                description,
            }),
        }
    }

    /// Returns the VM owner and storage generation of this Symbol.
    #[must_use]
    pub fn identity(&self) -> &VmIdentity {
        &self.data.identity
    }

    #[must_use]
    pub const fn id(&self) -> SymbolId {
        self.id
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.data.description.as_ref().map(JsString::as_str)
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
        self.identity() == other.identity() && self.id == other.id
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SymbolData {
    identity: VmIdentity,
    description: Option<JsString>,
}

#[derive(Debug, Clone)]
pub struct SymbolTable {
    identity: VmIdentity,
    entries: Vec<JsSymbol>,
    registry: Vec<(JsString, SymbolId)>,
}

impl SymbolTable {
    pub const fn new(identity: VmIdentity) -> Self {
        Self {
            identity,
            entries: Vec::new(),
            registry: Vec::new(),
        }
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) const fn registry_entry_count(&self) -> usize {
        self.registry.len()
    }

    pub fn create(&mut self, description: Option<JsString>) -> Result<JsSymbol> {
        if let Some(value) = &description
            && value.identity() != &self.identity
        {
            return Err(Error::runtime(FOREIGN_SYMBOL_DESCRIPTION_ERROR));
        }
        let id = SymbolId::from_index(self.entries.len())?;
        let symbol = JsSymbol::new(self.identity.clone(), id, description);
        self.entries.push(symbol.clone());
        Ok(symbol)
    }

    pub fn for_key(&mut self, key: JsString) -> Result<JsSymbol> {
        if key.identity() != &self.identity {
            return Err(Error::runtime(FOREIGN_SYMBOL_REGISTRY_KEY_ERROR));
        }
        if let Some((_, id)) = self
            .registry
            .iter()
            .find(|(registered, _)| registered == &key)
        {
            return self.get(*id).cloned();
        }
        let symbol = self.create(Some(key.clone()))?;
        self.registry.push((key, symbol.id()));
        Ok(symbol)
    }

    pub fn key_for(&self, id: SymbolId) -> Result<Option<JsString>> {
        self.get(id)?;
        Ok(self
            .registry
            .iter()
            .find(|(_, registered_id)| *registered_id == id)
            .map(|(key, _)| key.clone()))
    }

    pub fn get(&self, id: SymbolId) -> Result<&JsSymbol> {
        self.entries
            .get(id.index()?)
            .ok_or_else(|| Error::runtime("symbol id is not defined"))
    }
}
