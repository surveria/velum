use core::hash::{Hash, Hasher};
use std::collections::{HashMap, hash_map::DefaultHasher};

use crate::{
    error::{Error, Result},
    runtime::{VmStorageKind, storage_ledger::VmStorageLedger},
};

use super::PropertyKey;

const SHAPE_HASH_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const SHAPE_HASH_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub(super) struct PropertySlot(usize);

impl PropertySlot {
    pub(super) const fn from_index(index: usize) -> Self {
        Self(index)
    }

    pub(super) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
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

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
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

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd)]
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

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
struct ShapeFamilyId(u32);

impl ShapeFamilyId {
    fn from_index(index: usize) -> Result<Self> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| Error::limit("shape family table exceeded supported range"))
    }

    fn index(self) -> Result<usize> {
        usize::try_from(self.0)
            .map_err(|_| Error::limit("shape family id exceeded supported range"))
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
struct ShapeLayoutHash(u64);

impl ShapeLayoutHash {
    const fn root() -> Self {
        Self(SHAPE_HASH_OFFSET)
    }

    fn from_properties(properties: &[ShapePropertyLayout]) -> Self {
        properties
            .iter()
            .fold(Self::root(), |hash, property| hash.extended(*property))
    }

    fn extended(self, property: ShapePropertyLayout) -> Self {
        let mut hasher = DefaultHasher::new();
        property.hash(&mut hasher);
        Self((self.0 ^ hasher.finish()).wrapping_mul(SHAPE_HASH_PRIME))
    }
}

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
enum ShapeTransition {
    Add {
        current: ShapeId,
        property: ShapePropertyLayout,
    },
    Update {
        current: ShapeId,
        property: ShapePropertyLayout,
    },
    Remove {
        current: ShapeId,
        key: PropertyKey,
    },
}

#[derive(Debug, Clone)]
pub(super) struct ShapeTable {
    shapes: Vec<Shape>,
    families: Vec<ShapeFamily>,
    transitions: HashMap<ShapeTransition, ShapeId>,
    layout_index: HashMap<ShapeLayoutHash, Vec<ShapeId>>,
    storage_ledger: VmStorageLedger,
}

impl ShapeTable {
    pub(super) fn new(storage_ledger: VmStorageLedger) -> Self {
        Self {
            shapes: Vec::new(),
            families: Vec::new(),
            transitions: HashMap::new(),
            layout_index: HashMap::new(),
            storage_ledger,
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.shapes.len().saturating_add(1)
    }

    pub(in crate::runtime::object) fn storage_entry_count(&self) -> Result<usize> {
        let mut entries = self
            .shapes
            .len()
            .checked_add(self.transitions.len())
            .ok_or_else(|| Error::limit("shape cache entry count overflowed"))?;
        for family in &self.families {
            entries = entries
                .checked_add(1)
                .and_then(|count| count.checked_add(family.properties.len()))
                .and_then(|count| count.checked_add(family.offsets.len()))
                .ok_or_else(|| Error::limit("shape cache entry count overflowed"))?;
        }
        for ids in self.layout_index.values() {
            entries = entries
                .checked_add(ids.len())
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
        if self.property_slot(current, key)?.is_some() {
            return self.transition_after_update(current, key, attributes);
        }

        let property = ShapePropertyLayout::new(key, attributes);
        let transition = ShapeTransition::Add { current, property };
        if let Some(target) = self.transitions.get(&transition) {
            return Ok(*target);
        }

        let target_hash = self.layout_hash(current)?.extended(property);
        if let Some(target) = self.canonical_extended_shape(current, property, target_hash)? {
            self.cache_transition(transition, target)?;
            return Ok(target);
        }

        if let Some(family) = self.appendable_family(current)? {
            return self.append_shape(family, property, target_hash, transition);
        }

        let current_properties = self.properties(current)?;
        let target_len = current_properties
            .len()
            .checked_add(1)
            .ok_or_else(|| Error::limit("shape property count overflowed"))?;
        let mut properties = Vec::new();
        properties
            .try_reserve(target_len)
            .map_err(|error| Error::limit(format!("shape property allocation failed: {error}")))?;
        properties.extend_from_slice(current_properties);
        properties.push(property);
        self.create_shape_family(&properties, target_hash, Some(transition))
    }

    pub(super) fn transition_after_update(
        &mut self,
        current: ShapeId,
        key: PropertyKey,
        attributes: ShapePropertyAttributes,
    ) -> Result<ShapeId> {
        let Some(slot) = self.property_slot(current, key)? else {
            return Ok(current);
        };
        let Some(existing) = self.property_layout(current, slot)? else {
            return Err(Error::runtime("shape property layout is not available"));
        };
        let property = existing.with_attributes(attributes);
        if property == existing {
            return Ok(current);
        }

        let transition = ShapeTransition::Update { current, property };
        if let Some(target) = self.transitions.get(&transition) {
            return Ok(*target);
        }

        let mut properties = self.try_clone_properties(current)?;
        let Some(target) = properties.get_mut(slot.index()) else {
            return Err(Error::runtime("shape property slot is not available"));
        };
        *target = property;
        let target_hash = ShapeLayoutHash::from_properties(&properties);
        self.shape_for_properties(&properties, target_hash, Some(transition))
    }

    pub(super) fn transition_after_remove(
        &mut self,
        current: ShapeId,
        key: PropertyKey,
    ) -> Result<ShapeId> {
        let Some(slot) = self.property_slot(current, key)? else {
            return Ok(current);
        };
        let transition = ShapeTransition::Remove { current, key };
        if let Some(target) = self.transitions.get(&transition) {
            return Ok(*target);
        }

        let current_properties = self.properties(current)?;
        let mut properties = Vec::new();
        properties
            .try_reserve(current_properties.len().saturating_sub(1))
            .map_err(|error| Error::limit(format!("shape property allocation failed: {error}")))?;
        for (index, property) in current_properties.iter().copied().enumerate() {
            if index != slot.index() {
                properties.push(property);
            }
        }
        let target_hash = ShapeLayoutHash::from_properties(&properties);
        self.shape_for_properties(&properties, target_hash, Some(transition))
    }

    pub(super) fn transition_after_attributes<I>(
        &mut self,
        current: ShapeId,
        attributes: I,
    ) -> Result<ShapeId>
    where
        I: IntoIterator<Item = (PropertyKey, ShapePropertyAttributes)>,
    {
        let current_properties = self.properties(current)?;
        let mut properties = Vec::new();
        properties
            .try_reserve(current_properties.len())
            .map_err(|error| Error::limit(format!("shape property allocation failed: {error}")))?;
        let mut changed = false;
        for (index, (key, updated_attributes)) in attributes.into_iter().enumerate() {
            let Some(property) = current_properties.get(index).copied() else {
                return Err(Error::runtime(
                    "shape attribute update exceeds property layout",
                ));
            };
            if property.key() != key {
                return Err(Error::runtime(
                    "shape attribute update order does not match layout",
                ));
            }
            let updated = property.with_attributes(updated_attributes);
            changed |= updated != property;
            properties.push(updated);
        }
        if properties.len() != current_properties.len() {
            return Err(Error::runtime(
                "shape attribute update omitted property layout",
            ));
        }
        if !changed {
            return Ok(current);
        }
        let target_hash = ShapeLayoutHash::from_properties(&properties);
        self.shape_for_properties(&properties, target_hash, None)
    }

    pub(super) fn property_slot(
        &self,
        shape: ShapeId,
        key: PropertyKey,
    ) -> Result<Option<PropertySlot>> {
        if shape == ShapeId::root() {
            return Ok(None);
        }
        let shape = self.shape(shape)?;
        let family = self.family(shape.family)?;
        Ok(family
            .offsets
            .get(&key)
            .copied()
            .filter(|slot| slot.index() < shape.property_count))
    }

    fn property_layout(
        &self,
        shape: ShapeId,
        slot: PropertySlot,
    ) -> Result<Option<ShapePropertyLayout>> {
        if shape == ShapeId::root() {
            return Ok(None);
        }
        let shape = self.shape(shape)?;
        if slot.index() >= shape.property_count {
            return Ok(None);
        }
        Ok(self
            .family(shape.family)?
            .properties
            .get(slot.index())
            .copied())
    }

    fn try_clone_properties(&self, id: ShapeId) -> Result<Vec<ShapePropertyLayout>> {
        let properties = self.properties(id)?;
        let mut cloned = Vec::new();
        cloned
            .try_reserve(properties.len())
            .map_err(|error| Error::limit(format!("shape property allocation failed: {error}")))?;
        cloned.extend_from_slice(properties);
        Ok(cloned)
    }

    fn shape_for_properties(
        &mut self,
        properties: &[ShapePropertyLayout],
        hash: ShapeLayoutHash,
        transition: Option<ShapeTransition>,
    ) -> Result<ShapeId> {
        if properties.is_empty() {
            if let Some(transition) = transition {
                self.cache_transition(transition, ShapeId::root())?;
            }
            return Ok(ShapeId::root());
        }
        if let Some(id) = self.canonical_shape(properties, hash)? {
            if let Some(transition) = transition {
                self.cache_transition(transition, id)?;
            }
            return Ok(id);
        }
        self.create_shape_family(properties, hash, transition)
    }

    fn create_shape_family(
        &mut self,
        properties: &[ShapePropertyLayout],
        hash: ShapeLayoutHash,
        transition: Option<ShapeTransition>,
    ) -> Result<ShapeId> {
        let family = ShapeFamily::try_from_properties(properties)?;
        let family_id = ShapeFamilyId::from_index(self.families.len())?;
        let shape_id = ShapeId::from_storage_index(self.shapes.len())?;
        self.families
            .try_reserve(1)
            .map_err(|error| Error::limit(format!("shape family allocation failed: {error}")))?;
        self.shapes
            .try_reserve(1)
            .map_err(|error| Error::limit(format!("shape allocation failed: {error}")))?;
        let new_bucket = self.prepare_layout_index(hash)?;
        if transition.is_some() {
            self.transitions.try_reserve(1).map_err(|error| {
                Error::limit(format!("shape transition allocation failed: {error}"))
            })?;
        }

        let cache_entries = properties
            .len()
            .checked_mul(2)
            .and_then(|count| count.checked_add(3))
            .and_then(|count| count.checked_add(usize::from(transition.is_some())))
            .ok_or_else(|| Error::limit("shape cache entry count overflowed"))?;
        let reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::CacheEntry, cache_entries)?;
        reservation.commit()?;

        self.families.push(family);
        self.shapes
            .push(Shape::new(family_id, properties.len(), hash));
        self.finish_layout_index(hash, shape_id, new_bucket)?;
        if let Some(transition) = transition {
            self.transitions.insert(transition, shape_id);
        }
        Ok(shape_id)
    }

    fn append_shape(
        &mut self,
        family_id: ShapeFamilyId,
        property: ShapePropertyLayout,
        hash: ShapeLayoutHash,
        transition: ShapeTransition,
    ) -> Result<ShapeId> {
        let shape_id = ShapeId::from_storage_index(self.shapes.len())?;
        self.shapes
            .try_reserve(1)
            .map_err(|error| Error::limit(format!("shape allocation failed: {error}")))?;
        self.transitions.try_reserve(1).map_err(|error| {
            Error::limit(format!("shape transition allocation failed: {error}"))
        })?;
        let new_bucket = self.prepare_layout_index(hash)?;
        self.family_mut(family_id)?.prepare_append(property.key())?;

        let reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::CacheEntry, 5)?;
        reservation.commit()?;

        let property_count = self.family_mut(family_id)?.append(property)?;
        self.shapes
            .push(Shape::new(family_id, property_count, hash));
        self.finish_layout_index(hash, shape_id, new_bucket)?;
        self.transitions.insert(transition, shape_id);
        Ok(shape_id)
    }

    fn cache_transition(&mut self, transition: ShapeTransition, target: ShapeId) -> Result<()> {
        if self.transitions.contains_key(&transition) {
            return Ok(());
        }
        self.transitions.try_reserve(1).map_err(|error| {
            Error::limit(format!("shape transition allocation failed: {error}"))
        })?;
        let reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::CacheEntry, 1)?;
        reservation.commit()?;
        self.transitions.insert(transition, target);
        Ok(())
    }

    fn prepare_layout_index(&mut self, hash: ShapeLayoutHash) -> Result<Option<Vec<ShapeId>>> {
        if let Some(ids) = self.layout_index.get_mut(&hash) {
            ids.try_reserve(1).map_err(|error| {
                Error::limit(format!("shape layout index allocation failed: {error}"))
            })?;
            return Ok(None);
        }
        self.layout_index.try_reserve(1).map_err(|error| {
            Error::limit(format!("shape layout index allocation failed: {error}"))
        })?;
        let mut ids = Vec::new();
        ids.try_reserve(1).map_err(|error| {
            Error::limit(format!("shape layout index allocation failed: {error}"))
        })?;
        Ok(Some(ids))
    }

    fn finish_layout_index(
        &mut self,
        hash: ShapeLayoutHash,
        shape: ShapeId,
        new_bucket: Option<Vec<ShapeId>>,
    ) -> Result<()> {
        if let Some(ids) = new_bucket {
            self.layout_index.insert(hash, ids);
        }
        let Some(ids) = self.layout_index.get_mut(&hash) else {
            return Err(Error::runtime("shape layout index bucket is not available"));
        };
        ids.push(shape);
        Ok(())
    }

    fn canonical_shape(
        &self,
        properties: &[ShapePropertyLayout],
        hash: ShapeLayoutHash,
    ) -> Result<Option<ShapeId>> {
        let Some(ids) = self.layout_index.get(&hash) else {
            return Ok(None);
        };
        for id in ids {
            if self.properties(*id)? == properties {
                return Ok(Some(*id));
            }
        }
        Ok(None)
    }

    fn canonical_extended_shape(
        &self,
        current: ShapeId,
        property: ShapePropertyLayout,
        hash: ShapeLayoutHash,
    ) -> Result<Option<ShapeId>> {
        let Some(ids) = self.layout_index.get(&hash) else {
            return Ok(None);
        };
        let current_properties = self.properties(current)?;
        let target_len = current_properties
            .len()
            .checked_add(1)
            .ok_or_else(|| Error::limit("shape property count overflowed"))?;
        for id in ids {
            let candidate = self.properties(*id)?;
            if candidate.len() == target_len
                && candidate.get(..current_properties.len()) == Some(current_properties)
                && candidate.last() == Some(&property)
            {
                return Ok(Some(*id));
            }
        }
        Ok(None)
    }

    fn appendable_family(&self, id: ShapeId) -> Result<Option<ShapeFamilyId>> {
        if id == ShapeId::root() {
            return Ok(None);
        }
        let shape = self.shape(id)?;
        let family = self.family(shape.family)?;
        Ok((family.properties.len() == shape.property_count).then_some(shape.family))
    }

    fn layout_hash(&self, id: ShapeId) -> Result<ShapeLayoutHash> {
        if id == ShapeId::root() {
            return Ok(ShapeLayoutHash::root());
        }
        self.shape(id).map(|shape| shape.layout_hash)
    }

    fn properties(&self, id: ShapeId) -> Result<&[ShapePropertyLayout]> {
        if id == ShapeId::root() {
            return Ok(&[]);
        }
        let shape = self.shape(id)?;
        self.family(shape.family)?
            .properties
            .get(..shape.property_count)
            .ok_or_else(|| Error::runtime("shape property prefix is not available"))
    }

    fn shape(&self, id: ShapeId) -> Result<&Shape> {
        self.shapes
            .get(id.storage_index()?)
            .ok_or_else(|| Error::runtime("shape id is not defined"))
    }

    fn family(&self, id: ShapeFamilyId) -> Result<&ShapeFamily> {
        self.families
            .get(id.index()?)
            .ok_or_else(|| Error::runtime("shape family id is not defined"))
    }

    fn family_mut(&mut self, id: ShapeFamilyId) -> Result<&mut ShapeFamily> {
        self.families
            .get_mut(id.index()?)
            .ok_or_else(|| Error::runtime("shape family id is not defined"))
    }

    pub(in crate::runtime::object) fn property_keys(
        &self,
    ) -> impl Iterator<Item = PropertyKey> + '_ {
        self.families
            .iter()
            .flat_map(|family| family.properties.iter().map(|property| property.key()))
    }
}

#[derive(Debug, Clone)]
struct Shape {
    family: ShapeFamilyId,
    property_count: usize,
    layout_hash: ShapeLayoutHash,
}

impl Shape {
    const fn new(
        family: ShapeFamilyId,
        property_count: usize,
        layout_hash: ShapeLayoutHash,
    ) -> Self {
        Self {
            family,
            property_count,
            layout_hash,
        }
    }
}

#[derive(Debug, Clone)]
struct ShapeFamily {
    properties: Vec<ShapePropertyLayout>,
    offsets: HashMap<PropertyKey, PropertySlot>,
}

impl ShapeFamily {
    fn try_from_properties(properties: &[ShapePropertyLayout]) -> Result<Self> {
        let mut family = Self {
            properties: Vec::new(),
            offsets: HashMap::new(),
        };
        family
            .properties
            .try_reserve(properties.len())
            .map_err(|error| {
                Error::limit(format!("shape family property allocation failed: {error}"))
            })?;
        family
            .offsets
            .try_reserve(properties.len())
            .map_err(|error| {
                Error::limit(format!("shape family offset allocation failed: {error}"))
            })?;
        for property in properties.iter().copied() {
            let slot = PropertySlot::from_index(family.properties.len());
            if family.offsets.insert(property.key(), slot).is_some() {
                return Err(Error::runtime(
                    "shape family contains duplicate property key",
                ));
            }
            family.properties.push(property);
        }
        Ok(family)
    }

    fn prepare_append(&mut self, key: PropertyKey) -> Result<()> {
        if self.offsets.contains_key(&key) {
            return Err(Error::runtime(
                "shape family append duplicates property key",
            ));
        }
        self.properties.try_reserve(1).map_err(|error| {
            Error::limit(format!("shape family property allocation failed: {error}"))
        })?;
        self.offsets.try_reserve(1).map_err(|error| {
            Error::limit(format!("shape family offset allocation failed: {error}"))
        })
    }

    fn append(&mut self, property: ShapePropertyLayout) -> Result<usize> {
        let slot = PropertySlot::from_index(self.properties.len());
        if self.offsets.insert(property.key(), slot).is_some() {
            return Err(Error::runtime("shape family append replaced property key"));
        }
        self.properties.push(property);
        Ok(self.properties.len())
    }
}
