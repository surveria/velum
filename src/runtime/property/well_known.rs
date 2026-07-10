use crate::runtime::object::PropertyKey;

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
pub(in crate::runtime) struct DescriptorPropertyKeys {
    value: PropertyKey,
    writable: PropertyKey,
    enumerable: PropertyKey,
    configurable: PropertyKey,
    get: PropertyKey,
    set: PropertyKey,
}

impl DescriptorPropertyKeys {
    pub(in crate::runtime) const fn new(
        value: PropertyKey,
        writable: PropertyKey,
        enumerable: PropertyKey,
        configurable: PropertyKey,
        get: PropertyKey,
        set: PropertyKey,
    ) -> Self {
        Self {
            value,
            writable,
            enumerable,
            configurable,
            get,
            set,
        }
    }

    pub(in crate::runtime) const fn get(self) -> PropertyKey {
        self.get
    }

    pub(in crate::runtime) const fn set(self) -> PropertyKey {
        self.set
    }

    pub(in crate::runtime) const fn value(self) -> PropertyKey {
        self.value
    }

    pub(in crate::runtime) const fn writable(self) -> PropertyKey {
        self.writable
    }

    pub(in crate::runtime) const fn enumerable(self) -> PropertyKey {
        self.enumerable
    }

    pub(in crate::runtime) const fn configurable(self) -> PropertyKey {
        self.configurable
    }

    pub(in crate::runtime) fn keys(self) -> impl Iterator<Item = PropertyKey> {
        [
            self.value,
            self.writable,
            self.enumerable,
            self.configurable,
            self.get,
            self.set,
        ]
        .into_iter()
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::runtime) struct WellKnownPropertyKeys {
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
    pub(in crate::runtime) const fn new() -> Self {
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
    pub(in crate::runtime) fn lookup(&self, name: &str) -> Option<PropertyKey> {
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

    pub(in crate::runtime) fn should_remember(&self, name: &str) -> bool {
        self.lookup(name).is_none()
            && matches!(
                name,
                CONFIGURABLE_PROPERTY
                    | CONSTRUCTOR_PROPERTY
                    | ENUMERABLE_PROPERTY
                    | GET_PROPERTY
                    | LENGTH_PROPERTY
                    | NAME_PROPERTY
                    | PROTOTYPE_PROPERTY
                    | SET_PROPERTY
                    | VALUE_PROPERTY
                    | WRITABLE_PROPERTY
            )
    }

    pub(in crate::runtime) fn remember(&mut self, name: &str, key: PropertyKey) {
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

    pub(in crate::runtime) fn keys(self) -> impl Iterator<Item = PropertyKey> {
        [
            self.configurable,
            self.constructor,
            self.enumerable,
            self.get,
            self.length,
            self.name,
            self.prototype,
            self.set,
            self.value,
            self.writable,
        ]
        .into_iter()
        .flatten()
    }

    pub(in crate::runtime) fn entry_count(self) -> usize {
        self.keys().count()
    }
}

impl Default for WellKnownPropertyKeys {
    fn default() -> Self {
        Self::new()
    }
}
