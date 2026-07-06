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

    pub(super) fn transition_after_add(
        &mut self,
        current: ShapeId,
        key: PropertyKey,
    ) -> Result<ShapeId> {
        let current_keys = self.keys(current)?;
        if current_keys.contains(&key) {
            return Ok(current);
        }

        let mut keys = Vec::with_capacity(
            current_keys
                .len()
                .checked_add(1)
                .ok_or_else(|| Error::limit("shape property count overflowed"))?,
        );
        keys.extend_from_slice(current_keys);
        keys.push(key);
        self.shape_for_keys(&keys)
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
        let current_keys = self.keys(current)?;
        if !current_keys.contains(&key) {
            return Ok(current);
        }

        let mut keys = Vec::with_capacity(current_keys.len().saturating_sub(1));
        for existing_key in current_keys {
            if *existing_key != key {
                keys.push(*existing_key);
            }
        }
        self.shape_for_keys(&keys)
    }

    fn shape_for_keys(&mut self, keys: &[PropertyKey]) -> Result<ShapeId> {
        if keys.is_empty() {
            return Ok(ShapeId::root());
        }

        if let Some(position) = self.shapes.iter().position(|shape| shape.keys() == keys) {
            return ShapeId::from_storage_index(position);
        }

        let id = ShapeId::from_storage_index(self.shapes.len())?;
        self.shapes.push(Shape::from_keys(keys));
        Ok(id)
    }

    fn keys(&self, id: ShapeId) -> Result<&[PropertyKey]> {
        if id == ShapeId::root() {
            return Ok(&[]);
        }
        let index = id.storage_index()?;
        self.shapes
            .get(index)
            .map(Shape::keys)
            .ok_or_else(|| Error::runtime("shape id is not defined"))
    }
}

#[derive(Debug, Clone)]
struct Shape {
    keys: Box<[PropertyKey]>,
    offsets: Box<[ShapePropertyOffset]>,
}

impl Shape {
    fn from_keys(keys: &[PropertyKey]) -> Self {
        let mut offsets = Vec::with_capacity(keys.len());
        for (index, key) in keys.iter().copied().enumerate() {
            offsets.push(ShapePropertyOffset::new(
                key,
                PropertySlot::from_index(index),
            ));
        }
        offsets.sort_by_key(ShapePropertyOffset::key);
        Self {
            keys: keys.into(),
            offsets: offsets.into(),
        }
    }

    fn keys(&self) -> &[PropertyKey] {
        &self.keys
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
