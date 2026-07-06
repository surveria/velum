use crate::runtime_object::PropertyKey;

const CONSTRUCTOR_PROPERTY: &str = "constructor";
const LENGTH_PROPERTY: &str = "length";
const NAME_PROPERTY: &str = "name";
const PROTOTYPE_PROPERTY: &str = "prototype";

#[derive(Debug, Clone, Copy)]
pub(super) struct WellKnownPropertyKeys {
    constructor: Option<PropertyKey>,
    length: Option<PropertyKey>,
    name: Option<PropertyKey>,
    prototype: Option<PropertyKey>,
}

impl WellKnownPropertyKeys {
    pub(super) const fn new() -> Self {
        Self {
            constructor: None,
            length: None,
            name: None,
            prototype: None,
        }
    }

    #[must_use]
    pub(super) fn lookup(&self, name: &str) -> Option<PropertyKey> {
        match name {
            CONSTRUCTOR_PROPERTY => self.constructor,
            LENGTH_PROPERTY => self.length,
            NAME_PROPERTY => self.name,
            PROTOTYPE_PROPERTY => self.prototype,
            _ => None,
        }
    }

    pub(super) fn remember(&mut self, name: &str, key: PropertyKey) {
        match name {
            CONSTRUCTOR_PROPERTY => self.constructor = Some(key),
            LENGTH_PROPERTY => self.length = Some(key),
            NAME_PROPERTY => self.name = Some(key),
            PROTOTYPE_PROPERTY => self.prototype = Some(key),
            _ => {}
        }
    }
}

impl Default for WellKnownPropertyKeys {
    fn default() -> Self {
        Self::new()
    }
}
