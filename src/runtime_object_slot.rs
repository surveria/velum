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
        let slot = self.properties.get(&key)?;
        self.named_properties
            .get(slot.index())
            .map(NamedProperty::property)
    }

    pub(super) fn named_property_mut(&mut self, key: PropertyKey) -> Result<&mut ObjectProperty> {
        let slot = self
            .properties
            .get(&key)
            .ok_or_else(|| Error::runtime("object property slot is not defined"))?;
        self.named_properties
            .get_mut(slot.index())
            .map(|entry| &mut entry.property)
            .ok_or_else(|| Error::runtime("object property slot is not available"))
    }

    pub(super) fn named_properties(&self) -> impl Iterator<Item = &NamedProperty> {
        self.named_properties.iter()
    }

    pub(super) fn push_named_property(
        &mut self,
        key: PropertyKey,
        property: ObjectProperty,
    ) -> Result<()> {
        let slot = PropertySlot::from_index(self.named_properties.len());
        self.named_properties
            .push(NamedProperty::new(key, property));
        let previous = self.properties.insert(key, slot);
        if previous.is_some() {
            return Err(Error::runtime("object property slot replaced existing key"));
        }
        Ok(())
    }

    pub(super) fn remove_named_property(&mut self, key: PropertyKey) -> Option<ObjectProperty> {
        let slot = self.properties.remove(&key)?;
        let index = slot.index();
        self.named_properties.get(index)?;
        let removed = self.named_properties.remove(index);
        self.reindex_named_properties_from(index);
        Some(removed.property)
    }

    fn reindex_named_properties_from(&mut self, start: usize) {
        for (index, property) in self.named_properties.iter().enumerate().skip(start) {
            self.properties
                .insert(property.key(), PropertySlot::from_index(index));
        }
    }
}
