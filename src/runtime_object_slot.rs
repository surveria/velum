use crate::error::{Error, Result};

use super::{Object, ObjectProperty, PropertyKey};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct PropertySlot(usize);

impl PropertySlot {
    const fn from_index(index: usize) -> Self {
        Self(index)
    }

    const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PropertyIndexEntry {
    key: PropertyKey,
    slot: PropertySlot,
}

impl PropertyIndexEntry {
    const fn new(key: PropertyKey, slot: PropertySlot) -> Self {
        Self { key, slot }
    }

    const fn key(self) -> PropertyKey {
        self.key
    }

    const fn slot(self) -> PropertySlot {
        self.slot
    }

    const fn set_slot(&mut self, slot: PropertySlot) {
        self.slot = slot;
    }
}

#[derive(Debug, Clone)]
pub(super) struct NamedProperty {
    key: PropertyKey,
    property: ObjectProperty,
}

impl NamedProperty {
    const fn new(key: PropertyKey, property: ObjectProperty) -> Self {
        Self { key, property }
    }

    pub(super) const fn key(&self) -> PropertyKey {
        self.key
    }

    pub(super) const fn property(&self) -> &ObjectProperty {
        &self.property
    }
}

impl Object {
    pub(super) fn named_property(&self, key: PropertyKey) -> Option<&ObjectProperty> {
        let slot = self.named_property_slot(key)?;
        self.named_properties
            .get(slot.index())
            .map(NamedProperty::property)
    }

    pub(super) fn named_property_mut(&mut self, key: PropertyKey) -> Result<&mut ObjectProperty> {
        let slot = self
            .named_property_slot(key)
            .ok_or_else(|| Error::runtime("object property slot is not defined"))?;
        self.named_properties
            .get_mut(slot.index())
            .map(|entry| &mut entry.property)
            .ok_or_else(|| Error::runtime("object property slot is not available"))
    }

    pub(super) fn named_properties(&self) -> impl Iterator<Item = &NamedProperty> {
        self.named_properties.iter()
    }

    pub(super) fn contains_named_property(&self, key: PropertyKey) -> bool {
        self.property_position(key).is_ok()
    }

    pub(super) fn push_named_property(
        &mut self,
        key: PropertyKey,
        property: ObjectProperty,
    ) -> Result<()> {
        let Err(position) = self.property_position(key) else {
            return Err(Error::runtime("object property slot replaced existing key"));
        };
        let slot = PropertySlot::from_index(self.named_properties.len());
        self.named_properties
            .push(NamedProperty::new(key, property));
        self.properties
            .insert(position, PropertyIndexEntry::new(key, slot));
        Ok(())
    }

    pub(super) fn remove_named_property(&mut self, key: PropertyKey) -> Option<ObjectProperty> {
        let position = self.property_position(key).ok()?;
        let slot = self.properties.get(position)?.slot();
        let index = slot.index();
        self.named_properties.get(index)?;
        let removed = self.named_properties.remove(index);
        self.properties.remove(position);
        self.reindex_named_properties_from(index);
        Some(removed.property)
    }

    fn reindex_named_properties_from(&mut self, start: usize) {
        let mut index = start;
        while index < self.named_properties.len() {
            let Some(property) = self.named_properties.get(index) else {
                return;
            };
            let key = property.key();
            let slot = PropertySlot::from_index(index);
            if let Some(entry) = self.property_index_entry_mut(key) {
                entry.set_slot(slot);
            }
            index = index.saturating_add(1);
        }
    }

    fn named_property_slot(&self, key: PropertyKey) -> Option<PropertySlot> {
        let position = self.property_position(key).ok()?;
        self.properties.get(position).map(|entry| entry.slot())
    }

    fn property_index_entry_mut(&mut self, key: PropertyKey) -> Option<&mut PropertyIndexEntry> {
        let position = self.property_position(key).ok()?;
        self.properties.get_mut(position)
    }

    fn property_position(&self, key: PropertyKey) -> std::result::Result<usize, usize> {
        self.properties
            .binary_search_by(|entry| entry.key().cmp(&key))
    }
}
