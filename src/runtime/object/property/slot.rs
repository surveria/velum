use crate::error::{Error, Result};

use super::{Object, ObjectProperty, PropertyKey, PropertySlot, ShapeTable};

#[derive(Debug, Clone)]
pub(in crate::runtime::object) struct NamedProperty {
    key: PropertyKey,
    property: ObjectProperty,
}

impl NamedProperty {
    const fn new(key: PropertyKey, property: ObjectProperty) -> Self {
        Self { key, property }
    }

    pub(in crate::runtime::object) const fn key(&self) -> PropertyKey {
        self.key
    }

    pub(in crate::runtime::object) const fn property(&self) -> &ObjectProperty {
        &self.property
    }

    pub(in crate::runtime::object) const fn property_mut(&mut self) -> &mut ObjectProperty {
        &mut self.property
    }
}

impl Object {
    pub(in crate::runtime::object) fn named_property(
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

    pub(in crate::runtime::object) fn named_property_mut(
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

    pub(in crate::runtime::object) fn named_properties(
        &self,
    ) -> impl Iterator<Item = &NamedProperty> {
        self.named_properties.iter()
    }

    pub(in crate::runtime::object) fn contains_named_property(
        &self,
        shapes: &ShapeTable,
        key: PropertyKey,
    ) -> Result<bool> {
        shapes
            .property_slot(self.shape, key)
            .map(|slot| slot.is_some())
    }

    pub(in crate::runtime::object) fn push_named_property(
        &mut self,
        shapes: &mut ShapeTable,
        key: PropertyKey,
        property: ObjectProperty,
    ) -> Result<()> {
        if self.contains_named_property(shapes, key)? {
            return Err(Error::runtime("object property slot replaced existing key"));
        }
        let reservation = self.reserve_property_growth()?;
        let attributes = property.shape_attributes();
        let shape = shapes.transition_after_add(self.shape, key, attributes)?;
        let Some(slot) = shapes.property_slot(shape, key)? else {
            return Err(Error::runtime("shape property slot is not defined"));
        };
        if slot.index() != self.named_properties.len() {
            return Err(Error::runtime("shape property slot does not match storage"));
        }
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
        self.named_properties
            .push(NamedProperty::new(key, property));
        self.shape = shape;
        Ok(())
    }

    pub(in crate::runtime::object) fn remove_named_property(
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
        self.release_property()?;
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
