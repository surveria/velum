use std::collections::BTreeMap;

use crate::error::{Error, Result};

use super::{ARRAY_INDEX_LIMIT_ERROR, ArrayIndex, ObjectProperty, PropertyKey};

#[derive(Debug, Clone)]
pub(super) struct ArrayStorage {
    elements: ArrayElements,
    sparse_keys: BTreeMap<ArrayIndex, PropertyKey>,
    property_count: usize,
}

impl ArrayStorage {
    pub(super) const fn new() -> Self {
        Self {
            elements: ArrayElements::Packed(Vec::new()),
            sparse_keys: BTreeMap::new(),
            property_count: 0,
        }
    }

    pub(super) const fn property_count(&self) -> usize {
        self.property_count
    }

    pub(super) fn dense_property(&self, index: ArrayIndex) -> Option<&ObjectProperty> {
        let position = index.position().ok()?;
        self.dense_property_at_position(position)
    }

    pub(super) fn dense_property_mut(
        &mut self,
        index: ArrayIndex,
    ) -> Result<Option<&mut ObjectProperty>> {
        let position = index.position()?;
        Ok(match &mut self.elements {
            ArrayElements::Packed(elements) => elements.get_mut(position),
            ArrayElements::Holey(elements) => elements.get_mut(position).and_then(Option::as_mut),
        })
    }

    pub(super) fn dense_property_at_position(&self, position: usize) -> Option<&ObjectProperty> {
        match &self.elements {
            ArrayElements::Packed(elements) => elements.get(position),
            ArrayElements::Holey(elements) => elements.get(position).and_then(Option::as_ref),
        }
    }

    pub(super) fn packed_properties_for_len(&self, len: usize) -> Option<&[ObjectProperty]> {
        if self.has_sparse_keys() {
            return None;
        }
        match &self.elements {
            ArrayElements::Packed(elements) if elements.len() == len => Some(elements.as_slice()),
            ArrayElements::Packed(_) | ArrayElements::Holey(_) => None,
        }
    }

    pub(super) const fn dense_len(&self) -> usize {
        match &self.elements {
            ArrayElements::Packed(elements) => elements.len(),
            ArrayElements::Holey(elements) => elements.len(),
        }
    }

    pub(super) fn insert_dense_property(
        &mut self,
        index: ArrayIndex,
        property: ObjectProperty,
    ) -> Result<Option<ObjectProperty>> {
        let position = index.position()?;
        match &mut self.elements {
            ArrayElements::Packed(elements) => {
                if let Some(existing) = elements.get_mut(position) {
                    return Ok(Some(std::mem::replace(existing, property)));
                }
                if position == elements.len() {
                    elements.push(property);
                    self.property_count = self.property_count.saturating_add(1);
                    return Ok(None);
                }
                let mut holey = Vec::with_capacity(Self::checked_dense_len(position)?);
                holey.extend(elements.drain(..).map(Some));
                holey.resize_with(Self::checked_dense_len(position)?, || None);
                let slot = holey
                    .get_mut(position)
                    .ok_or_else(|| Error::runtime("array index storage is not available"))?;
                *slot = Some(property);
                self.elements = ArrayElements::Holey(holey);
                self.property_count = self.property_count.saturating_add(1);
                Ok(None)
            }
            ArrayElements::Holey(elements) => {
                if elements.get(position).is_none() {
                    elements.resize_with(Self::checked_dense_len(position)?, || None);
                }
                let slot = elements
                    .get_mut(position)
                    .ok_or_else(|| Error::runtime("array index storage is not available"))?;
                let previous = slot.replace(property);
                if previous.is_none() {
                    self.property_count = self.property_count.saturating_add(1);
                }
                Ok(previous)
            }
        }
    }

    pub(super) fn remove_dense_property(
        &mut self,
        index: ArrayIndex,
    ) -> Result<Option<ObjectProperty>> {
        let position = index.position()?;
        let removed = match &mut self.elements {
            ArrayElements::Packed(elements) => {
                if elements.get(position).is_none() {
                    return Ok(None);
                }
                if position.checked_add(1) == Some(elements.len()) {
                    elements.pop()
                } else {
                    let mut holey = Vec::with_capacity(elements.len());
                    holey.extend(elements.drain(..).map(Some));
                    let removed = holey.get_mut(position).and_then(Option::take);
                    self.elements = ArrayElements::Holey(holey);
                    removed
                }
            }
            ArrayElements::Holey(elements) => {
                let Some(slot) = elements.get_mut(position) else {
                    return Ok(None);
                };
                slot.take()
            }
        };
        if removed.is_some() {
            self.property_count = self.property_count.saturating_sub(1);
        }
        Ok(removed)
    }

    pub(super) fn sparse_key(&self, index: ArrayIndex) -> Option<PropertyKey> {
        self.sparse_keys.get(&index).copied()
    }

    pub(super) fn insert_sparse_key(&mut self, index: ArrayIndex, key: PropertyKey) {
        self.sparse_keys.insert(index, key);
    }

    pub(super) fn remove_sparse_key(&mut self, index: ArrayIndex) -> Option<PropertyKey> {
        self.sparse_keys.remove(&index)
    }

    pub(super) fn sparse_keys(&self) -> impl Iterator<Item = (&ArrayIndex, &PropertyKey)> {
        self.sparse_keys.iter()
    }

    pub(super) fn has_sparse_keys(&self) -> bool {
        !self.sparse_keys.is_empty()
    }

    fn checked_dense_len(position: usize) -> Result<usize> {
        position
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))
    }
}

impl Default for ArrayStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
enum ArrayElements {
    // Packed means the materialized dense prefix has no holes; callers must still
    // compare storage length with the JavaScript array length before full fast paths.
    Packed(Vec<ObjectProperty>),
    Holey(Vec<Option<ObjectProperty>>),
}
