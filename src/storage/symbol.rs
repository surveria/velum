#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    ownership::VmIdentity,
    storage::string_heap::JsString,
};
use alloc::{collections::BTreeSet, rc::Rc};

const FOREIGN_SYMBOL_DESCRIPTION_ERROR: &str = "Symbol description belongs to another VM";
const FOREIGN_SYMBOL_REGISTRY_KEY_ERROR: &str = "Symbol registry key belongs to another VM";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Ord, PartialOrd)]
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

    pub(crate) fn description_string(&self) -> Option<&JsString> {
        self.data.description.as_ref()
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
    entries: Vec<Option<JsSymbol>>,
    free: Vec<usize>,
    live: usize,
    registry: Vec<(JsString, SymbolId)>,
    max_count: usize,
}

impl SymbolTable {
    pub const fn new(identity: VmIdentity, max_count: usize) -> Self {
        Self {
            identity,
            entries: Vec::new(),
            free: Vec::new(),
            live: 0,
            registry: Vec::new(),
            max_count,
        }
    }

    pub const fn len(&self) -> usize {
        self.live
    }

    pub(crate) const fn registry_entry_count(&self) -> usize {
        self.registry.len()
    }

    pub(crate) fn registered_ids(&self) -> impl Iterator<Item = SymbolId> + '_ {
        self.registry.iter().map(|(_, id)| *id)
    }

    pub(crate) fn has_registry_key(&self, key: &JsString) -> bool {
        self.registry
            .iter()
            .any(|(registered, _)| registered == key)
    }

    pub fn create(&mut self, description: Option<JsString>) -> Result<JsSymbol> {
        if let Some(value) = &description
            && value.identity() != Some(&self.identity)
        {
            return Err(Error::runtime(FOREIGN_SYMBOL_DESCRIPTION_ERROR));
        }
        if self.live >= self.max_count {
            return Err(Error::limit(format!(
                "Symbol record count exceeded {}",
                self.max_count
            )));
        }
        let index = self.free.last().copied().unwrap_or(self.entries.len());
        if self.free.is_empty() {
            self.entries
                .try_reserve(1)
                .map_err(|error| Error::limit(format!("Symbol table exhausted: {error}")))?;
        }
        let id = SymbolId::from_index(index)?;
        let symbol = JsSymbol::new(self.identity.clone(), id, description);
        if self.free.pop().is_some() {
            let Some(slot) = self.entries.get_mut(index) else {
                return Err(Error::runtime("Symbol free slot is not defined"));
            };
            if slot.replace(symbol.clone()).is_some() {
                return Err(Error::runtime("Symbol free slot is occupied"));
            }
        } else {
            self.entries.push(Some(symbol.clone()));
        }
        self.live = self
            .live
            .checked_add(1)
            .ok_or_else(|| Error::limit("Symbol live count overflowed"))?;
        Ok(symbol)
    }

    pub fn for_key(&mut self, key: JsString) -> Result<JsSymbol> {
        if key.identity() != Some(&self.identity) {
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
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::runtime("symbol id is not defined"))
    }

    pub(crate) fn sweep_unmarked(&mut self, marked: &BTreeSet<SymbolId>) -> Result<usize> {
        let removed = self
            .entries
            .iter()
            .filter_map(Option::as_ref)
            .filter(|symbol| !marked.contains(&symbol.id()))
            .count();
        self.free
            .try_reserve(removed)
            .map_err(|error| Error::limit(format!("Symbol free list exhausted: {error}")))?;
        for index in 0..self.entries.len() {
            let remove = self
                .entries
                .get(index)
                .and_then(Option::as_ref)
                .is_some_and(|symbol| !marked.contains(&symbol.id()));
            if !remove {
                continue;
            }
            let Some(slot) = self.entries.get_mut(index) else {
                return Err(Error::runtime("Symbol slot disappeared during sweep"));
            };
            if slot.take().is_none() {
                return Err(Error::runtime("Symbol record disappeared during sweep"));
            }
            self.free.push(index);
        }
        self.live = self
            .live
            .checked_sub(removed)
            .ok_or_else(|| Error::runtime("Symbol live count underflowed"))?;
        Ok(removed)
    }
}
