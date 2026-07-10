use crate::error::{Error, Result};

use super::PropertyKey;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct PropertySlot(usize);

impl PropertySlot {
    pub(super) const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ShapePropertyAttributes {
    writable: bool,
    enumerable: bool,
    configurable: bool,
}

impl ShapePropertyAttributes {
    pub(super) const fn new(writable: bool, enumerable: bool, configurable: bool) -> Self {
        Self {
            writable,
            enumerable,
            configurable,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ShapePropertyLayout {
    key: PropertyKey,
    attributes: ShapePropertyAttributes,
}

impl ShapePropertyLayout {
    const fn new(key: PropertyKey, attributes: ShapePropertyAttributes) -> Self {
        Self { key, attributes }
    }

    const fn key(self) -> PropertyKey {
        self.key
    }

    const fn with_attributes(self, attributes: ShapePropertyAttributes) -> Self {
        Self {
            key: self.key,
            attributes,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct ShapeId(u32);

impl ShapeId {
    pub(super) const fn root() -> Self {
        Self(0)
    }

    fn from_storage_index(index: usize) -> Result<Self> {
        let id = index
            .checked_add(1)
            .ok_or_else(|| Error::limit("shape id overflowed"))?;
        u32::try_from(id)
            .map(Self)
            .map_err(|_| Error::limit("shape table exceeded supported range"))
    }

    fn storage_index(self) -> Result<usize> {
        let index = usize::try_from(self.0)
            .map_err(|_| Error::limit("shape id exceeded supported range"))?;
        index
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("root shape has no storage index"))
    }
}

impl Default for ShapeId {
    fn default() -> Self {
        Self::root()
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct ShapeTable {
    shapes: Vec<Shape>,
}

impl ShapeTable {
    pub(super) const fn new() -> Self {
        Self { shapes: Vec::new() }
    }

    pub(super) const fn len(&self) -> usize {
        self.shapes.len().saturating_add(1)
    }

    pub(in crate::runtime::object) fn storage_entry_count(&self) -> Result<usize> {
        let mut entries = 0_usize;
        for shape in &self.shapes {
            entries = entries
                .checked_add(1)
                .and_then(|count| count.checked_add(shape.properties.len()))
                .and_then(|count| count.checked_add(shape.offsets.len()))
                .ok_or_else(|| Error::limit("shape cache entry count overflowed"))?;
        }
        Ok(entries)
    }

    pub(super) fn transition_after_add(
        &mut self,
        current: ShapeId,
        key: PropertyKey,
        attributes: ShapePropertyAttributes,
    ) -> Result<ShapeId> {
        let current_properties = self.properties(current)?;
        if current_properties
            .iter()
            .any(|property| property.key() == key)
        {
            return self.transition_after_update(current, key, attributes);
        }

        let mut properties = Vec::with_capacity(
            current_properties
                .len()
                .checked_add(1)
                .ok_or_else(|| Error::limit("shape property count overflowed"))?,
        );
        properties.extend_from_slice(current_properties);
        properties.push(ShapePropertyLayout::new(key, attributes));
        self.shape_for_properties(&properties)
    }

    pub(super) fn transition_after_update(
        &mut self,
        current: ShapeId,
        key: PropertyKey,
        attributes: ShapePropertyAttributes,
    ) -> Result<ShapeId> {
        let current_properties = self.properties(current)?;
        let mut properties = Vec::with_capacity(current_properties.len());
        let mut changed = false;

        for property in current_properties.iter().copied() {
            if property.key() == key {
                let updated = property.with_attributes(attributes);
                changed = changed || updated != property;
                properties.push(updated);
            } else {
                properties.push(property);
            }
        }

        if !changed {
            return Ok(current);
        }
        self.shape_for_properties(&properties)
    }

    pub(super) fn property_slot(
        &self,
        shape: ShapeId,
        key: PropertyKey,
    ) -> Result<Option<PropertySlot>> {
        if shape == ShapeId::root() {
            return Ok(None);
        }
        let index = shape.storage_index()?;
        let Some(shape) = self.shapes.get(index) else {
            return Err(Error::runtime("shape id is not defined"));
        };
        Ok(shape.property_slot(key))
    }

    pub(super) fn transition_after_remove(
        &mut self,
        current: ShapeId,
        key: PropertyKey,
    ) -> Result<ShapeId> {
        let current_properties = self.properties(current)?;
        if !current_properties
            .iter()
            .any(|property| property.key() == key)
        {
            return Ok(current);
        }

        let mut properties = Vec::with_capacity(current_properties.len().saturating_sub(1));
        for property in current_properties {
            if property.key() != key {
                properties.push(*property);
            }
        }
        self.shape_for_properties(&properties)
    }

    fn shape_for_properties(&mut self, properties: &[ShapePropertyLayout]) -> Result<ShapeId> {
        if properties.is_empty() {
            return Ok(ShapeId::root());
        }

        if let Some(position) = self
            .shapes
            .iter()
            .position(|shape| shape.properties() == properties)
        {
            return ShapeId::from_storage_index(position);
        }

        let id = ShapeId::from_storage_index(self.shapes.len())?;
        self.shapes.push(Shape::from_properties(properties));
        Ok(id)
    }

    fn properties(&self, id: ShapeId) -> Result<&[ShapePropertyLayout]> {
        if id == ShapeId::root() {
            return Ok(&[]);
        }
        let index = id.storage_index()?;
        self.shapes
            .get(index)
            .map(Shape::properties)
            .ok_or_else(|| Error::runtime("shape id is not defined"))
    }

    pub(in crate::runtime::object) fn property_keys(
        &self,
    ) -> impl Iterator<Item = PropertyKey> + '_ {
        self.shapes.iter().flat_map(|shape| {
            let properties = shape.properties.iter().map(|property| property.key);
            let offsets = shape.offsets.iter().map(|offset| offset.key);
            properties.chain(offsets)
        })
    }
}

#[derive(Debug, Clone)]
struct Shape {
    properties: Box<[ShapePropertyLayout]>,
    offsets: Box<[ShapePropertyOffset]>,
}

impl Shape {
    fn from_properties(properties: &[ShapePropertyLayout]) -> Self {
        let mut offsets = Vec::with_capacity(properties.len());
        for (index, property) in properties.iter().copied().enumerate() {
            offsets.push(ShapePropertyOffset::new(
                property.key(),
                PropertySlot::from_index(index),
            ));
        }
        offsets.sort_by_key(ShapePropertyOffset::key);
        Self {
            properties: properties.into(),
            offsets: offsets.into(),
        }
    }

    fn properties(&self) -> &[ShapePropertyLayout] {
        &self.properties
    }

    fn property_slot(&self, key: PropertyKey) -> Option<PropertySlot> {
        let position = self
            .offsets
            .binary_search_by_key(&key, ShapePropertyOffset::key);
        let Ok(position) = position else {
            return None;
        };
        self.offsets.get(position).map(ShapePropertyOffset::slot)
    }
}

#[derive(Debug, Clone, Copy)]
struct ShapePropertyOffset {
    key: PropertyKey,
    slot: PropertySlot,
}

impl ShapePropertyOffset {
    const fn new(key: PropertyKey, slot: PropertySlot) -> Self {
        Self { key, slot }
    }

    const fn key(&self) -> PropertyKey {
        self.key
    }

    const fn slot(&self) -> PropertySlot {
        self.slot
    }
}
