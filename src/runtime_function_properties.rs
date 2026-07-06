use crate::{
    atom::AtomTable,
    error::{Error, Result},
    runtime_object::{
        DataPropertyDescriptor, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
        PropertyKey, PropertyLookup, PropertyWritable,
    },
    value::Value,
};

use super::runtime_function_intrinsic::{FunctionIntrinsicProperty, FunctionProperty};

pub(super) const FUNCTION_LENGTH_PROPERTY: &str = "length";
pub(super) const FUNCTION_NAME_PROPERTY: &str = "name";
pub(super) const FUNCTION_PROTOTYPE_PROPERTY: &str = "prototype";
pub(super) const PROTOTYPE_CONSTRUCTOR_PROPERTY: &str = "constructor";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum FunctionPropertyKind {
    Length,
    Name,
    Prototype,
    Custom,
}

impl FunctionPropertyKind {
    #[must_use]
    pub(super) fn from_name(property: &str) -> Self {
        match property {
            FUNCTION_LENGTH_PROPERTY => Self::Length,
            FUNCTION_NAME_PROPERTY => Self::Name,
            FUNCTION_PROTOTYPE_PROPERTY => Self::Prototype,
            _ => Self::Custom,
        }
    }

    #[must_use]
    pub(super) const fn is_intrinsic_slot(self) -> bool {
        matches!(self, Self::Length | Self::Name)
    }

    #[must_use]
    pub(super) const fn is_prototype(self) -> bool {
        matches!(self, Self::Prototype)
    }

    #[must_use]
    const fn enumerable_name(self) -> Option<&'static str> {
        match self {
            Self::Length => Some(FUNCTION_LENGTH_PROPERTY),
            Self::Name => Some(FUNCTION_NAME_PROPERTY),
            Self::Prototype | Self::Custom => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct FunctionProperties {
    prototype: Value,
    intrinsic_defaults: FunctionIntrinsicDefaults,
    length: FunctionIntrinsicProperty,
    name: FunctionIntrinsicProperty,
    properties: Vec<FunctionPropertyEntry>,
    property_order: Vec<PropertyKey>,
}

#[derive(Debug, Clone)]
pub(super) struct FunctionIntrinsicDefaults {
    length: DataPropertyDescriptor,
    name: DataPropertyDescriptor,
    prototype: Option<DataPropertyDescriptor>,
}

impl FunctionIntrinsicDefaults {
    pub(super) const fn new(
        length: Value,
        name: Value,
        prototype: Option<DataPropertyDescriptor>,
    ) -> Self {
        Self {
            length: DataPropertyDescriptor::new(
                length,
                PropertyWritable::No,
                PropertyEnumerable::No,
                PropertyConfigurable::Yes,
            ),
            name: DataPropertyDescriptor::new(
                name,
                PropertyWritable::No,
                PropertyEnumerable::No,
                PropertyConfigurable::Yes,
            ),
            prototype,
        }
    }

    fn descriptor(&self, property: FunctionPropertyKind) -> Option<DataPropertyDescriptor> {
        match property {
            FunctionPropertyKind::Length => Some(self.length.clone()),
            FunctionPropertyKind::Name => Some(self.name.clone()),
            FunctionPropertyKind::Prototype => self.prototype.clone(),
            FunctionPropertyKind::Custom => None,
        }
    }

    fn set_prototype_value(&mut self, value: Value) {
        if let Some(descriptor) = &self.prototype {
            self.prototype = Some(DataPropertyDescriptor::new(
                value,
                descriptor.writable(),
                descriptor.enumerable(),
                descriptor.configurable(),
            ));
        }
    }
}

#[derive(Debug, Clone)]
struct FunctionPropertyEntry {
    key: PropertyKey,
    property: FunctionProperty,
}

impl FunctionPropertyEntry {
    const fn new(key: PropertyKey, property: FunctionProperty) -> Self {
        Self { key, property }
    }

    const fn key(&self) -> PropertyKey {
        self.key
    }

    const fn property(&self) -> &FunctionProperty {
        &self.property
    }

    const fn property_mut(&mut self) -> &mut FunctionProperty {
        &mut self.property
    }
}

impl FunctionProperties {
    pub(super) const fn new(
        prototype: Value,
        intrinsic_defaults: FunctionIntrinsicDefaults,
    ) -> Self {
        Self {
            prototype,
            intrinsic_defaults,
            length: FunctionIntrinsicProperty::new(),
            name: FunctionIntrinsicProperty::new(),
            properties: Vec::new(),
            property_order: Vec::new(),
        }
    }

    pub(super) fn prototype(&self) -> Value {
        self.prototype.clone()
    }

    pub(super) fn get(&self, property: PropertyLookup<'_>) -> Value {
        let Some(key) = property.key() else {
            return Value::Undefined;
        };
        self.function_property(key)
            .map_or(Value::Undefined, FunctionProperty::value)
    }

    pub(super) fn own_property_descriptor(
        &self,
        property: PropertyLookup<'_>,
    ) -> Option<DataPropertyDescriptor> {
        let key = property.key()?;
        self.function_property(key)
            .map(FunctionProperty::descriptor)
    }

    pub(super) fn intrinsic_descriptor(
        &self,
        property: FunctionPropertyKind,
    ) -> Option<DataPropertyDescriptor> {
        let default = self.intrinsic_defaults.descriptor(property)?;
        if property.is_prototype() {
            return Some(default);
        }
        self.intrinsic(property)
            .and_then(|intrinsic| intrinsic.descriptor(default))
    }

    pub(super) fn intrinsic_value(&self, property: FunctionPropertyKind) -> Option<Value> {
        let default = self.intrinsic_defaults.descriptor(property)?;
        if property.is_prototype() {
            return Some(default.value());
        }
        self.intrinsic(property)
            .and_then(|intrinsic| intrinsic.value(default))
    }

    pub(super) fn has(&self, property: PropertyLookup<'_>) -> bool {
        property
            .key()
            .is_some_and(|key| self.contains_function_property(key))
    }

    pub(super) fn has_intrinsic(&self, property: FunctionPropertyKind) -> bool {
        self.intrinsic(property)
            .is_some_and(FunctionIntrinsicProperty::has)
    }

    pub(super) fn set(
        &mut self,
        property: PropertyKey,
        property_kind: FunctionPropertyKind,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        if let Some(default) = self.intrinsic_defaults.descriptor(property_kind)
            && self.set_intrinsic_value(property_kind, default, value.clone())
        {
            return Ok(());
        }
        if property_kind.is_prototype() {
            self.intrinsic_defaults.set_prototype_value(value.clone());
            self.prototype = value;
            return Ok(());
        }
        if let Some(existing) = self.function_property_mut(property) {
            existing.set_value(value);
            return Ok(());
        }
        if self.property_order.len() >= max_properties {
            return Err(Error::limit(format!(
                "function property count exceeded {max_properties}"
            )));
        }
        self.push_function_property(
            property,
            FunctionProperty::new(value, PropertyEnumerable::Yes),
        );
        Ok(())
    }

    pub(super) fn delete(
        &mut self,
        property: PropertyLookup<'_>,
        property_kind: FunctionPropertyKind,
    ) -> bool {
        if let Some(default) = self.intrinsic_defaults.descriptor(property_kind)
            && let Some(deleted) = self.delete_intrinsic(property_kind, default)
        {
            return deleted;
        }
        if property_kind.is_prototype() {
            return false;
        }
        let Some(key) = property.key() else {
            return true;
        };
        let Some(existing_property) = self.function_property(key) else {
            return true;
        };
        if !existing_property.is_configurable() {
            return false;
        }
        let Some(_) = self.remove_function_property(key) else {
            return true;
        };
        self.property_order.retain(|stored_key| *stored_key != key);
        true
    }

    pub(super) fn keys(&self, atoms: &AtomTable) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        self.push_intrinsic_key(&mut keys, FunctionPropertyKind::Length);
        self.push_intrinsic_key(&mut keys, FunctionPropertyKind::Name);
        for key in &self.property_order {
            if self
                .function_property(*key)
                .is_some_and(FunctionProperty::is_enumerable)
            {
                keys.push(atoms.name(key.atom())?.to_owned());
            }
        }
        Ok(keys)
    }

    pub(super) fn define_builtin(
        &mut self,
        property: PropertyKey,
        value: Value,
        enumerable: PropertyEnumerable,
    ) {
        if let Some(existing) = self.function_property_mut(property) {
            existing.set_value(value);
            existing.set_enumerable(enumerable);
            return;
        }
        self.push_function_property(property, FunctionProperty::new(value, enumerable));
    }

    pub(super) fn define_property(
        &mut self,
        property: PropertyKey,
        property_kind: FunctionPropertyKind,
        update: DataPropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        if let Some(default) = self.intrinsic_defaults.descriptor(property_kind)
            && self.define_intrinsic(property_kind, default, &update)
        {
            return Ok(());
        }
        if property_kind.is_prototype() {
            if let Some(value) = update.value() {
                self.intrinsic_defaults.set_prototype_value(value.clone());
                self.prototype = value;
            }
            return Ok(());
        }
        if let Some(existing) = self.function_property_mut(property) {
            existing.define(&update);
            return Ok(());
        }
        if self.property_order.len() >= max_properties {
            return Err(Error::limit(format!(
                "function property count exceeded {max_properties}"
            )));
        }
        self.push_function_property(property, FunctionProperty::from_update(update));
        Ok(())
    }

    const fn intrinsic(
        &self,
        property: FunctionPropertyKind,
    ) -> Option<&FunctionIntrinsicProperty> {
        match property {
            FunctionPropertyKind::Length => Some(&self.length),
            FunctionPropertyKind::Name => Some(&self.name),
            FunctionPropertyKind::Prototype | FunctionPropertyKind::Custom => None,
        }
    }

    const fn intrinsic_mut(
        &mut self,
        property: FunctionPropertyKind,
    ) -> Option<&mut FunctionIntrinsicProperty> {
        match property {
            FunctionPropertyKind::Length => Some(&mut self.length),
            FunctionPropertyKind::Name => Some(&mut self.name),
            FunctionPropertyKind::Prototype | FunctionPropertyKind::Custom => None,
        }
    }

    fn set_intrinsic_value(
        &mut self,
        property: FunctionPropertyKind,
        default: DataPropertyDescriptor,
        value: Value,
    ) -> bool {
        let Some(intrinsic) = self.intrinsic_mut(property) else {
            return false;
        };
        intrinsic.set_value(default, value)
    }

    fn define_intrinsic(
        &mut self,
        property: FunctionPropertyKind,
        default: DataPropertyDescriptor,
        update: &DataPropertyUpdate,
    ) -> bool {
        let Some(intrinsic) = self.intrinsic_mut(property) else {
            return false;
        };
        intrinsic.define(default, update)
    }

    fn delete_intrinsic(
        &mut self,
        property: FunctionPropertyKind,
        default: DataPropertyDescriptor,
    ) -> Option<bool> {
        self.intrinsic_mut(property)
            .and_then(|intrinsic| intrinsic.delete(default))
    }

    fn push_intrinsic_key(&self, keys: &mut Vec<String>, property: FunctionPropertyKind) {
        let Some(name) = property.enumerable_name() else {
            return;
        };
        let Some(descriptor) = self.intrinsic_descriptor(property) else {
            return;
        };
        if descriptor.enumerable().is_yes() {
            keys.push(name.to_owned());
        }
    }

    fn contains_function_property(&self, property: PropertyKey) -> bool {
        self.property_position(property).is_ok()
    }

    fn function_property(&self, property: PropertyKey) -> Option<&FunctionProperty> {
        let position = self.property_position(property).ok()?;
        self.properties
            .get(position)
            .map(FunctionPropertyEntry::property)
    }

    fn function_property_mut(&mut self, property: PropertyKey) -> Option<&mut FunctionProperty> {
        let position = self.property_position(property).ok()?;
        self.properties
            .get_mut(position)
            .map(FunctionPropertyEntry::property_mut)
    }

    fn push_function_property(&mut self, property: PropertyKey, value: FunctionProperty) {
        let Err(position) = self.property_position(property) else {
            return;
        };
        self.property_order.push(property);
        self.properties
            .insert(position, FunctionPropertyEntry::new(property, value));
    }

    fn remove_function_property(&mut self, property: PropertyKey) -> Option<FunctionProperty> {
        let position = self.property_position(property).ok()?;
        let entry = self.properties.get(position)?;
        if entry.key() != property {
            return None;
        }
        Some(self.properties.remove(position).property)
    }

    fn property_position(&self, property: PropertyKey) -> std::result::Result<usize, usize> {
        self.properties
            .binary_search_by(|entry| entry.key().cmp(&property))
    }
}
