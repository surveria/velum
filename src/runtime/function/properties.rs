use crate::{
    error::{Error, Result},
    runtime::object::{
        DataPropertyDescriptor, DataPropertyUpdate, OwnPropertyDescriptor, PropertyConfigurable,
        PropertyEnumerable, PropertyKey, PropertyLookup, PropertyUpdate, PropertyWritable,
    },
    runtime::trace::{StrongEdgeReference, StrongEdgeVisitor},
    runtime::{VmStorageKind, storage_ledger::VmStorageLedger},
    storage::atom::AtomTable,
    value::Value,
};

use super::intrinsic::{FunctionIntrinsicProperty, FunctionProperty};

pub(in crate::runtime) const FUNCTION_LENGTH_PROPERTY: &str = "length";
pub(in crate::runtime) const FUNCTION_NAME_PROPERTY: &str = "name";
pub(in crate::runtime) const FUNCTION_PROTOTYPE_PROPERTY: &str = "prototype";
pub(in crate::runtime) const PROTOTYPE_CONSTRUCTOR_PROPERTY: &str = "constructor";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum FunctionPropertyKind {
    Length,
    Name,
    Prototype,
    Custom,
}

impl FunctionPropertyKind {
    #[must_use]
    pub(in crate::runtime) fn from_name(property: &str) -> Self {
        match property {
            FUNCTION_LENGTH_PROPERTY => Self::Length,
            FUNCTION_NAME_PROPERTY => Self::Name,
            FUNCTION_PROTOTYPE_PROPERTY => Self::Prototype,
            _ => Self::Custom,
        }
    }

    #[must_use]
    pub(in crate::runtime) const fn is_intrinsic_slot(self) -> bool {
        matches!(self, Self::Length | Self::Name)
    }

    #[must_use]
    pub(in crate::runtime) const fn is_prototype(self) -> bool {
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
pub(in crate::runtime) struct FunctionProperties {
    prototype: Value,
    intrinsic_defaults: FunctionIntrinsicDefaults,
    length: FunctionIntrinsicProperty,
    name: FunctionIntrinsicProperty,
    properties: Vec<FunctionPropertyEntry>,
    property_order: Vec<PropertyKey>,
    storage_ledger: Option<VmStorageLedger>,
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct FunctionIntrinsicDefaults {
    length: DataPropertyDescriptor,
    name: DataPropertyDescriptor,
    prototype: Option<DataPropertyDescriptor>,
}

impl FunctionIntrinsicDefaults {
    pub(in crate::runtime) const fn new(
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

    fn set_name_value(&mut self, value: Value) {
        let descriptor = &self.name;
        self.name = DataPropertyDescriptor::new(
            value,
            descriptor.writable(),
            descriptor.enumerable(),
            descriptor.configurable(),
        );
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
    pub(in crate::runtime) const fn new(
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
            storage_ledger: None,
        }
    }

    pub(in crate::runtime) fn activate_storage(
        &mut self,
        storage_ledger: VmStorageLedger,
    ) -> Result<()> {
        if self.storage_ledger.is_some() {
            return Err(Error::runtime(
                "function property storage is already active",
            ));
        }
        let property_count = self.storage_property_count()?;
        let cache_count = self.storage_cache_entry_count();
        let property_reservation =
            storage_ledger.reserve_count(VmStorageKind::ObjectProperty, property_count)?;
        let cache_reservation =
            storage_ledger.reserve_count(VmStorageKind::CacheEntry, cache_count)?;
        property_reservation.commit()?;
        if let Err(error) = cache_reservation.commit() {
            storage_ledger.release_count(VmStorageKind::ObjectProperty, property_count)?;
            return Err(error);
        }
        self.storage_ledger = Some(storage_ledger);
        Ok(())
    }

    pub(in crate::runtime) fn set_generated_name(&mut self, value: Value) {
        self.intrinsic_defaults.set_name_value(value);
    }

    pub(in crate::runtime) fn storage_property_count(&self) -> Result<usize> {
        usize::from(self.length.has())
            .checked_add(usize::from(self.name.has()))
            .and_then(|count| {
                count.checked_add(usize::from(self.intrinsic_defaults.prototype.is_some()))
            })
            .and_then(|count| count.checked_add(self.properties.len()))
            .ok_or_else(|| Error::limit("function property count overflowed"))
    }

    pub(in crate::runtime) const fn storage_cache_entry_count(&self) -> usize {
        self.property_order.len()
    }

    pub(in crate::runtime) fn visit_strong_edges<Kind: Copy, V: StrongEdgeVisitor<Kind>>(
        &self,
        kind: Kind,
        visitor: &mut V,
    ) -> Result<()> {
        visitor.visit(kind, StrongEdgeReference::Value(&self.prototype))?;
        visitor.visit(
            kind,
            StrongEdgeReference::Value(self.intrinsic_defaults.length.value_ref()),
        )?;
        visitor.visit(
            kind,
            StrongEdgeReference::Value(self.intrinsic_defaults.name.value_ref()),
        )?;
        if let Some(prototype) = &self.intrinsic_defaults.prototype {
            visitor.visit(kind, StrongEdgeReference::Value(prototype.value_ref()))?;
        }
        if let Some(value) = self.length.stored_value() {
            visitor.visit(kind, StrongEdgeReference::Value(value))?;
        }
        if let Some(value) = self.name.stored_value() {
            visitor.visit(kind, StrongEdgeReference::Value(value))?;
        }
        for entry in &self.properties {
            visitor.visit(kind, StrongEdgeReference::PropertyKey(entry.key))?;
            entry.property.visit_strong_edges(kind, visitor)?;
        }
        for key in &self.property_order {
            visitor.visit(kind, StrongEdgeReference::PropertyKey(*key))?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn prototype(&self) -> Value {
        self.prototype.clone()
    }

    pub(in crate::runtime) fn own_property_descriptor(
        &self,
        property: PropertyLookup<'_>,
    ) -> Option<OwnPropertyDescriptor> {
        let key = property.key()?;
        self.function_property(key)
            .map(FunctionProperty::descriptor)
    }

    pub(in crate::runtime) fn intrinsic_descriptor(
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

    pub(in crate::runtime) fn intrinsic_value(
        &self,
        property: FunctionPropertyKind,
    ) -> Option<Value> {
        let default = self.intrinsic_defaults.descriptor(property)?;
        if property.is_prototype() {
            return Some(default.value());
        }
        self.intrinsic(property)
            .and_then(|intrinsic| intrinsic.value(default))
    }

    pub(in crate::runtime) fn has(&self, property: PropertyLookup<'_>) -> bool {
        property
            .key()
            .is_some_and(|key| self.contains_function_property(key))
    }

    pub(in crate::runtime) fn has_intrinsic(&self, property: FunctionPropertyKind) -> bool {
        self.intrinsic(property)
            .is_some_and(FunctionIntrinsicProperty::has)
    }

    pub(in crate::runtime) fn set(
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
        )
    }

    pub(in crate::runtime) fn delete(
        &mut self,
        property: PropertyLookup<'_>,
        property_kind: FunctionPropertyKind,
    ) -> Result<bool> {
        let previous_property_count = self.storage_property_count()?;
        let previous_cache_count = self.storage_cache_entry_count();
        if let Some(default) = self.intrinsic_defaults.descriptor(property_kind)
            && let Some(deleted) = self.delete_intrinsic(property_kind, default)
        {
            self.release_removed_storage(previous_property_count, previous_cache_count)?;
            return Ok(deleted);
        }
        if property_kind.is_prototype() {
            return Ok(false);
        }
        let Some(key) = property.key() else {
            return Ok(true);
        };
        let Some(existing_property) = self.function_property(key) else {
            return Ok(true);
        };
        if !existing_property.is_configurable() {
            return Ok(false);
        }
        let Some(_) = self.remove_function_property(key) else {
            return Ok(true);
        };
        self.property_order.retain(|stored_key| *stored_key != key);
        self.release_removed_storage(previous_property_count, previous_cache_count)?;
        Ok(true)
    }

    pub(in crate::runtime) fn keys(&self, atoms: &AtomTable) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        self.push_intrinsic_key(&mut keys, FunctionPropertyKind::Length);
        self.push_intrinsic_key(&mut keys, FunctionPropertyKind::Name);
        for key in &self.property_order {
            if self
                .function_property(*key)
                .is_some_and(FunctionProperty::is_enumerable)
            {
                let Some(atom) = key.atom() else {
                    continue;
                };
                keys.push(atoms.name(atom)?.to_owned());
            }
        }
        Ok(keys)
    }

    pub(in crate::runtime) fn define_builtin(
        &mut self,
        property: PropertyKey,
        value: Value,
        enumerable: PropertyEnumerable,
    ) -> Result<()> {
        if let Some(existing) = self.function_property_mut(property) {
            existing.set_value(value);
            existing.set_enumerable(enumerable);
            return Ok(());
        }
        self.push_function_property(property, FunctionProperty::new(value, enumerable))
    }

    pub(in crate::runtime) fn define_property(
        &mut self,
        property: PropertyKey,
        property_kind: FunctionPropertyKind,
        update: PropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        if let Some(default) = self.intrinsic_defaults.descriptor(property_kind) {
            match &update {
                PropertyUpdate::Data(data)
                    if self.define_intrinsic(property_kind, &default, data) =>
                {
                    return Ok(());
                }
                PropertyUpdate::Accessor(_) => {
                    if self.property_order.len() >= max_properties {
                        return Err(Error::limit(format!(
                            "function property count exceeded {max_properties}"
                        )));
                    }
                    if self
                        .delete_intrinsic(property_kind, default)
                        .is_some_and(|deleted| deleted)
                    {
                        self.push_function_property(
                            property,
                            FunctionProperty::from_update(update),
                        )?;
                        if let Some(storage_ledger) = &self.storage_ledger {
                            storage_ledger.release_count(VmStorageKind::ObjectProperty, 1)?;
                        }
                        return Ok(());
                    }
                }
                PropertyUpdate::Data(_) => {}
            }
        }
        if property_kind.is_prototype() {
            if let PropertyUpdate::Data(update) = update
                && let Some(value) = update.value()
            {
                self.intrinsic_defaults.set_prototype_value(value.clone());
                self.prototype = value;
            }
            return Ok(());
        }
        if let Some(existing) = self.function_property_mut(property) {
            existing.define(update);
            return Ok(());
        }
        if self.property_order.len() >= max_properties {
            return Err(Error::limit(format!(
                "function property count exceeded {max_properties}"
            )));
        }
        self.push_function_property(property, FunctionProperty::from_update(update))
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
        default: &DataPropertyDescriptor,
        update: &DataPropertyUpdate,
    ) -> bool {
        let Some(intrinsic) = self.intrinsic_mut(property) else {
            return false;
        };
        intrinsic.define(default.clone(), update)
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

    fn push_function_property(
        &mut self,
        property: PropertyKey,
        value: FunctionProperty,
    ) -> Result<()> {
        let Err(position) = self.property_position(property) else {
            return Ok(());
        };
        let reservations = if let Some(storage_ledger) = &self.storage_ledger {
            Some((
                storage_ledger.reserve_count(VmStorageKind::ObjectProperty, 1)?,
                storage_ledger.reserve_count(VmStorageKind::CacheEntry, 1)?,
            ))
        } else {
            None
        };
        if let Some((property_reservation, cache_reservation)) = reservations {
            property_reservation.commit()?;
            cache_reservation.commit()?;
        }
        self.property_order.push(property);
        self.properties
            .insert(position, FunctionPropertyEntry::new(property, value));
        Ok(())
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

    fn release_removed_storage(
        &self,
        previous_property_count: usize,
        previous_cache_count: usize,
    ) -> Result<()> {
        let Some(storage_ledger) = &self.storage_ledger else {
            return Ok(());
        };
        let property_count = self.storage_property_count()?;
        let removed_properties = previous_property_count
            .checked_sub(property_count)
            .ok_or_else(|| Error::runtime("function property count grew during deletion"))?;
        let removed_cache_entries = previous_cache_count
            .checked_sub(self.storage_cache_entry_count())
            .ok_or_else(|| Error::runtime("function property cache grew during deletion"))?;
        storage_ledger.release_count(VmStorageKind::ObjectProperty, removed_properties)?;
        storage_ledger.release_count(VmStorageKind::CacheEntry, removed_cache_entries)
    }
}
