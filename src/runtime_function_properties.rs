use std::collections::{BTreeMap, btree_map::Entry};

use crate::{
    atom::AtomTable,
    error::{Error, Result},
    runtime_object::{
        DataPropertyDescriptor, DataPropertyUpdate, PropertyEnumerable, PropertyKey, PropertyLookup,
    },
    value::Value,
};

use super::runtime_function_intrinsic::{FunctionIntrinsicProperty, FunctionProperty};

pub(super) const FUNCTION_LENGTH_PROPERTY: &str = "length";
pub(super) const FUNCTION_NAME_PROPERTY: &str = "name";
pub(super) const FUNCTION_PROTOTYPE_PROPERTY: &str = "prototype";
pub(super) const PROTOTYPE_CONSTRUCTOR_PROPERTY: &str = "constructor";

#[derive(Debug, Clone)]
pub(super) struct FunctionProperties {
    prototype: Value,
    length: FunctionIntrinsicProperty,
    name: FunctionIntrinsicProperty,
    properties: BTreeMap<PropertyKey, FunctionProperty>,
    property_order: Vec<PropertyKey>,
}

impl FunctionProperties {
    pub(super) const fn new(prototype: Value) -> Self {
        Self {
            prototype,
            length: FunctionIntrinsicProperty::new(),
            name: FunctionIntrinsicProperty::new(),
            properties: BTreeMap::new(),
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
        self.properties
            .get(&key)
            .map_or(Value::Undefined, FunctionProperty::value)
    }

    pub(super) fn own_property_descriptor(
        &self,
        property: PropertyLookup<'_>,
    ) -> Option<DataPropertyDescriptor> {
        let key = property.key()?;
        self.properties.get(&key).map(FunctionProperty::descriptor)
    }

    pub(super) fn intrinsic_descriptor(
        &self,
        property: &str,
        default: DataPropertyDescriptor,
    ) -> Option<DataPropertyDescriptor> {
        self.intrinsic(property)
            .and_then(|intrinsic| intrinsic.descriptor(default))
    }

    pub(super) fn intrinsic_value_or_property(
        &self,
        property: &str,
        lookup: PropertyLookup<'_>,
        default: DataPropertyDescriptor,
    ) -> Value {
        self.intrinsic(property)
            .and_then(|intrinsic| intrinsic.value(default))
            .unwrap_or_else(|| self.get(lookup))
    }

    pub(super) fn has(&self, property: PropertyLookup<'_>) -> bool {
        property
            .key()
            .is_some_and(|key| self.properties.contains_key(&key))
    }

    pub(super) fn has_intrinsic(&self, property: &str) -> bool {
        self.intrinsic(property)
            .is_some_and(FunctionIntrinsicProperty::has)
    }

    pub(super) fn set(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        max_properties: usize,
        default_intrinsic: Option<DataPropertyDescriptor>,
    ) -> Result<()> {
        if let Some(default) = default_intrinsic
            && self.set_intrinsic_value(property_name, default, value.clone())
        {
            return Ok(());
        }
        if property_name == FUNCTION_PROTOTYPE_PROPERTY {
            self.prototype = value;
            return Ok(());
        }
        match self.properties.entry(property) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().set_value(value);
            }
            Entry::Vacant(entry) => {
                if self.property_order.len() >= max_properties {
                    return Err(Error::limit(format!(
                        "function property count exceeded {max_properties}"
                    )));
                }
                self.property_order.push(*entry.key());
                entry.insert(FunctionProperty::new(value, PropertyEnumerable::Yes));
            }
        }
        Ok(())
    }

    pub(super) fn delete(
        &mut self,
        property: PropertyLookup<'_>,
        default_intrinsic: Option<DataPropertyDescriptor>,
    ) -> bool {
        if let Some(default) = default_intrinsic
            && let Some(deleted) = self.delete_intrinsic(property.name(), default)
        {
            return deleted;
        }
        if property.name() == FUNCTION_PROTOTYPE_PROPERTY {
            return false;
        }
        let Some(key) = property.key() else {
            return true;
        };
        let Some(existing_property) = self.properties.get(&key) else {
            return true;
        };
        if !existing_property.is_configurable() {
            return false;
        }
        let Some(_) = self.properties.remove(&key) else {
            return true;
        };
        self.property_order.retain(|stored_key| *stored_key != key);
        true
    }

    pub(super) fn keys(
        &self,
        atoms: &AtomTable,
        length: Option<DataPropertyDescriptor>,
        name: Option<DataPropertyDescriptor>,
    ) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        self.push_intrinsic_key(&mut keys, FUNCTION_LENGTH_PROPERTY, length);
        self.push_intrinsic_key(&mut keys, FUNCTION_NAME_PROPERTY, name);
        for key in &self.property_order {
            if self
                .properties
                .get(key)
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
        match self.properties.entry(property) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().set_value(value);
                entry.get_mut().set_enumerable(enumerable);
            }
            Entry::Vacant(entry) => {
                self.property_order.push(*entry.key());
                entry.insert(FunctionProperty::new(value, enumerable));
            }
        }
    }

    pub(super) fn define_property(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        update: DataPropertyUpdate,
        max_properties: usize,
        default_intrinsic: Option<DataPropertyDescriptor>,
    ) -> Result<()> {
        if let Some(default) = default_intrinsic
            && self.define_intrinsic(property_name, default, &update)
        {
            return Ok(());
        }
        if property_name == FUNCTION_PROTOTYPE_PROPERTY {
            if let Some(value) = update.value() {
                self.prototype = value;
            }
            return Ok(());
        }
        match self.properties.entry(property) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().define(&update);
            }
            Entry::Vacant(entry) => {
                if self.property_order.len() >= max_properties {
                    return Err(Error::limit(format!(
                        "function property count exceeded {max_properties}"
                    )));
                }
                self.property_order.push(*entry.key());
                entry.insert(FunctionProperty::from_update(update));
            }
        }
        Ok(())
    }

    fn intrinsic(&self, property: &str) -> Option<&FunctionIntrinsicProperty> {
        match property {
            FUNCTION_LENGTH_PROPERTY => Some(&self.length),
            FUNCTION_NAME_PROPERTY => Some(&self.name),
            _ => None,
        }
    }

    fn intrinsic_mut(&mut self, property: &str) -> Option<&mut FunctionIntrinsicProperty> {
        match property {
            FUNCTION_LENGTH_PROPERTY => Some(&mut self.length),
            FUNCTION_NAME_PROPERTY => Some(&mut self.name),
            _ => None,
        }
    }

    fn set_intrinsic_value(
        &mut self,
        property: &str,
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
        property: &str,
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
        property: &str,
        default: DataPropertyDescriptor,
    ) -> Option<bool> {
        self.intrinsic_mut(property)
            .and_then(|intrinsic| intrinsic.delete(default))
    }

    fn push_intrinsic_key(
        &self,
        keys: &mut Vec<String>,
        property: &str,
        descriptor: Option<DataPropertyDescriptor>,
    ) {
        let Some(descriptor) =
            descriptor.and_then(|default| self.intrinsic_descriptor(property, default))
        else {
            return;
        };
        if descriptor.enumerable().is_yes() {
            keys.push(property.to_owned());
        }
    }
}
