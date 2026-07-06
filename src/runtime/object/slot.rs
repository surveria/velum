use crate::error::{Error, Result};

use super::shape::PropertySlot;
use super::{Object, ObjectProperty, PropertyKey, ShapeTable};

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

    pub(super) const fn property_mut(&mut self) -> &mut ObjectProperty {
        &mut self.property
    }
}

impl Object {
    pub(super) fn named_property(
        &self,
        shapes: &ShapeTable,
        key: PropertyKey,
    ) -> Result<Option<&ObjectProperty>> {
        let Some(slot) = self.named_property_slot(shapes, key)? else {
            return Ok(None);
        };
        self.named_properties
            .get(slot.index())
            .map(NamedProperty::property)
            .map(Some)
            .ok_or_else(|| Error::runtime("object property slot is not available"))
    }

    pub(super) fn named_property_mut(
        &mut self,
        shapes: &ShapeTable,
        key: PropertyKey,
    ) -> Result<&mut ObjectProperty> {
        let slot = self
            .named_property_slot(shapes, key)?
            .ok_or_else(|| Error::runtime("object property slot is not defined"))?;
        self.named_properties
            .get_mut(slot.index())
            .map(|entry| &mut entry.property)
            .ok_or_else(|| Error::runtime("object property slot is not available"))
    }

    pub(super) fn named_properties(&self) -> impl Iterator<Item = &NamedProperty> {
        self.named_properties.iter()
    }

    pub(super) fn contains_named_property(
        &self,
        shapes: &ShapeTable,
        key: PropertyKey,
    ) -> Result<bool> {
        shapes
            .property_slot(self.shape, key)
            .map(|slot| slot.is_some())
    }

    pub(super) fn push_named_property(
        &mut self,
        shapes: &mut ShapeTable,
        key: PropertyKey,
        property: ObjectProperty,
    ) -> Result<()> {
        if self.contains_named_property(shapes, key)? {
            return Err(Error::runtime("object property slot replaced existing key"));
        }
        let attributes = property.shape_attributes();
        let shape = shapes.transition_after_add(self.shape, key, attributes)?;
        let Some(slot) = shapes.property_slot(shape, key)? else {
            return Err(Error::runtime("shape property slot is not defined"));
        };
        if slot.index() != self.named_properties.len() {
            return Err(Error::runtime("shape property slot does not match storage"));
        }
        self.named_properties
            .push(NamedProperty::new(key, property));
        self.shape = shape;
        Ok(())
    }

    pub(super) fn remove_named_property(
        &mut self,
        shapes: &mut ShapeTable,
        key: PropertyKey,
    ) -> Result<Option<ObjectProperty>> {
        let Some(slot) = self.named_property_slot(shapes, key)? else {
            return Ok(None);
        };
        let index = slot.index();
        if self.named_properties.get(index).is_none() {
            return Ok(None);
        }
        let shape = shapes.transition_after_remove(self.shape, key)?;
        let removed = self.named_properties.remove(index);
        self.shape = shape;
        Ok(Some(removed.property))
    }

    fn named_property_slot(
        &self,
        shapes: &ShapeTable,
        key: PropertyKey,
    ) -> Result<Option<PropertySlot>> {
        shapes.property_slot(self.shape, key)
    }
}
