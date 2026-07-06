use crate::runtime_object::PropertyKey;

const CONSTRUCTOR_PROPERTY: &str = "constructor";
const CONFIGURABLE_PROPERTY: &str = "configurable";
const ENUMERABLE_PROPERTY: &str = "enumerable";
const GET_PROPERTY: &str = "get";
const LENGTH_PROPERTY: &str = "length";
const NAME_PROPERTY: &str = "name";
const PROTOTYPE_PROPERTY: &str = "prototype";
const SET_PROPERTY: &str = "set";
const VALUE_PROPERTY: &str = "value";
const WRITABLE_PROPERTY: &str = "writable";

#[derive(Debug, Clone, Copy)]
pub(super) struct DescriptorPropertyKeys {
    value: PropertyKey,
    writable: PropertyKey,
    enumerable: PropertyKey,
    configurable: PropertyKey,
}

impl DescriptorPropertyKeys {
    pub(super) const fn new(
        value: PropertyKey,
        writable: PropertyKey,
        enumerable: PropertyKey,
        configurable: PropertyKey,
    ) -> Self {
        Self {
            value,
            writable,
            enumerable,
            configurable,
        }
    }

    pub(super) const fn value(self) -> PropertyKey {
        self.value
    }

    pub(super) const fn writable(self) -> PropertyKey {
        self.writable
    }

    pub(super) const fn enumerable(self) -> PropertyKey {
        self.enumerable
    }

    pub(super) const fn configurable(self) -> PropertyKey {
        self.configurable
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WellKnownPropertyKeys {
    configurable: Option<PropertyKey>,
    constructor: Option<PropertyKey>,
    enumerable: Option<PropertyKey>,
    get: Option<PropertyKey>,
    length: Option<PropertyKey>,
    name: Option<PropertyKey>,
    prototype: Option<PropertyKey>,
    set: Option<PropertyKey>,
    value: Option<PropertyKey>,
    writable: Option<PropertyKey>,
}

impl WellKnownPropertyKeys {
    pub(super) const fn new() -> Self {
        Self {
            configurable: None,
            constructor: None,
            enumerable: None,
            get: None,
            length: None,
            name: None,
            prototype: None,
            set: None,
            value: None,
            writable: None,
        }
    }

    #[must_use]
    pub(super) fn lookup(&self, name: &str) -> Option<PropertyKey> {
        match name {
            CONFIGURABLE_PROPERTY => self.configurable,
            CONSTRUCTOR_PROPERTY => self.constructor,
            ENUMERABLE_PROPERTY => self.enumerable,
            GET_PROPERTY => self.get,
            LENGTH_PROPERTY => self.length,
            NAME_PROPERTY => self.name,
            PROTOTYPE_PROPERTY => self.prototype,
            SET_PROPERTY => self.set,
            VALUE_PROPERTY => self.value,
            WRITABLE_PROPERTY => self.writable,
            _ => None,
        }
    }

    pub(super) fn remember(&mut self, name: &str, key: PropertyKey) {
        match name {
            CONFIGURABLE_PROPERTY => self.configurable = Some(key),
            CONSTRUCTOR_PROPERTY => self.constructor = Some(key),
            ENUMERABLE_PROPERTY => self.enumerable = Some(key),
            GET_PROPERTY => self.get = Some(key),
            LENGTH_PROPERTY => self.length = Some(key),
            NAME_PROPERTY => self.name = Some(key),
            PROTOTYPE_PROPERTY => self.prototype = Some(key),
            SET_PROPERTY => self.set = Some(key),
            VALUE_PROPERTY => self.value = Some(key),
            WRITABLE_PROPERTY => self.writable = Some(key),
            _ => {}
        }
    }
}

impl Default for WellKnownPropertyKeys {
    fn default() -> Self {
        Self::new()
    }
}
