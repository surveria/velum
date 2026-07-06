use crate::{
    error::{Error, Result},
    value::{ObjectId, Value},
};

use super::runtime_object_index::ArrayIndex;
use super::runtime_object_shape::{PropertySlot, ShapeId};
use super::{ARRAY_LENGTH_PROPERTY, Object, ObjectHeap, PropertyKey, PropertyLookup};

const PROTOTYPE_CYCLE_DETECTED_ERROR: &str = "prototype cycle detected";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct PropertyLookupGuard {
    receiver: ObjectId,
    receiver_shape: ShapeId,
    prototype_lookup_version: u64,
}

impl PropertyLookupGuard {
    const fn new(
        receiver: ObjectId,
        receiver_shape: ShapeId,
        prototype_lookup_version: u64,
    ) -> Self {
        Self {
            receiver,
            receiver_shape,
            prototype_lookup_version,
        }
    }

    fn is_valid(self, objects: &ObjectHeap) -> Result<bool> {
        let object = objects.object(self.receiver)?;
        Ok(object.shape == self.receiver_shape
            && objects.prototype_lookup_version() == self.prototype_lookup_version)
    }

    fn is_valid_for(self, objects: &ObjectHeap, receiver: ObjectId) -> Result<bool> {
        if self.receiver != receiver {
            return Ok(false);
        }
        self.is_valid(objects)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct CacheablePropertyLookup {
    guard: PropertyLookupGuard,
    result: CacheablePropertyLookupResult,
}

impl CacheablePropertyLookup {
    const fn hit(guard: PropertyLookupGuard, hit: CacheablePropertyHit) -> Self {
        Self {
            guard,
            result: CacheablePropertyLookupResult::Hit(hit),
        }
    }

    const fn missing(guard: PropertyLookupGuard) -> Self {
        Self {
            guard,
            result: CacheablePropertyLookupResult::Missing,
        }
    }

    const fn uncacheable(guard: PropertyLookupGuard) -> Self {
        Self {
            guard,
            result: CacheablePropertyLookupResult::Uncacheable,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CacheablePropertyLookupResult {
    Hit(CacheablePropertyHit),
    Missing,
    Uncacheable,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct CacheablePropertyHit {
    owner: ObjectId,
    owner_shape: ShapeId,
    slot: PropertySlot,
    depth: PrototypeLookupDepth,
}

impl CacheablePropertyHit {
    const fn new(
        owner: ObjectId,
        owner_shape: ShapeId,
        slot: PropertySlot,
        depth: PrototypeLookupDepth,
    ) -> Self {
        Self {
            owner,
            owner_shape,
            slot,
            depth,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct PrototypeLookupDepth(usize);

impl PrototypeLookupDepth {
    const fn root() -> Self {
        Self(0)
    }

    fn next(self) -> Result<Self> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or_else(|| Error::limit("prototype lookup depth overflowed"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CacheablePropertyValue {
    Hit(Value),
    Missing,
    Uncacheable,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CacheablePropertyPresence {
    Hit,
    Missing,
    Uncacheable,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CacheablePropertyWrite {
    Updated,
    Uncacheable,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PrototypeTraversalBudget {
    remaining: usize,
}

impl PrototypeTraversalBudget {
    pub(super) const fn from_object_count(object_count: usize) -> Self {
        Self {
            remaining: object_count,
        }
    }

    pub(super) fn enter_next(&mut self) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| Error::runtime(PROTOTYPE_CYCLE_DETECTED_ERROR))?;
        Ok(())
    }
}

impl ObjectHeap {
    pub(super) fn cacheable_property_value(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<CacheablePropertyValue> {
        let lookup = self.cacheable_property_lookup(id, property)?;
        self.read_cacheable_property_value(lookup)
    }

    pub(super) fn cacheable_property_presence(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<CacheablePropertyPresence> {
        let lookup = self.cacheable_property_lookup(id, property)?;
        self.read_cacheable_property_presence(lookup)
    }

    pub(crate) fn cacheable_property_lookup(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<CacheablePropertyLookup> {
        let guard = self.lookup_guard(id)?;
        let Some(key) = property.key() else {
            return Ok(CacheablePropertyLookup::uncacheable(guard));
        };

        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        let mut current = Some(id);
        let mut depth = PrototypeLookupDepth::root();
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if object.has_uncacheable_own_property(property) {
                return Ok(CacheablePropertyLookup::uncacheable(guard));
            }
            if let Some(hit) =
                object.cacheable_property_hit(current_id, key, depth, &self.shapes)?
            {
                return Ok(CacheablePropertyLookup::hit(guard, hit));
            }
            current = object.prototype;
            depth = depth.next()?;
        }

        Ok(CacheablePropertyLookup::missing(guard))
    }

    fn lookup_guard(&self, receiver: ObjectId) -> Result<PropertyLookupGuard> {
        let receiver_shape = self.object(receiver)?.shape;
        Ok(PropertyLookupGuard::new(
            receiver,
            receiver_shape,
            self.prototype_lookup_version(),
        ))
    }

    pub(crate) fn read_cacheable_property_value_for(
        &self,
        id: ObjectId,
        lookup: CacheablePropertyLookup,
    ) -> Result<CacheablePropertyValue> {
        if !lookup.guard.is_valid_for(self, id)? {
            return Ok(CacheablePropertyValue::Uncacheable);
        }
        self.read_valid_cacheable_property_value(lookup)
    }

    pub(crate) fn read_cacheable_property_presence_for(
        &self,
        id: ObjectId,
        lookup: CacheablePropertyLookup,
    ) -> Result<CacheablePropertyPresence> {
        if !lookup.guard.is_valid_for(self, id)? {
            return Ok(CacheablePropertyPresence::Uncacheable);
        }
        self.read_valid_cacheable_property_presence(lookup)
    }

    pub(crate) fn write_cacheable_own_property_value_for(
        &mut self,
        id: ObjectId,
        lookup: CacheablePropertyLookup,
        value: Value,
    ) -> Result<CacheablePropertyWrite> {
        if !lookup.guard.is_valid_for(self, id)? {
            return Ok(CacheablePropertyWrite::Uncacheable);
        }
        let CacheablePropertyLookupResult::Hit(hit) = lookup.result else {
            return Ok(CacheablePropertyWrite::Uncacheable);
        };
        if hit.owner != id {
            return Ok(CacheablePropertyWrite::Uncacheable);
        }
        let object = self.object_mut(hit.owner)?;
        if object.shape != hit.owner_shape || object.array_length.is_some() {
            return Ok(CacheablePropertyWrite::Uncacheable);
        }
        object.update_named_property_at_slot(hit.slot, value)?;
        Ok(CacheablePropertyWrite::Updated)
    }

    fn read_cacheable_property_value(
        &self,
        lookup: CacheablePropertyLookup,
    ) -> Result<CacheablePropertyValue> {
        if !lookup.guard.is_valid(self)? {
            return Ok(CacheablePropertyValue::Uncacheable);
        }
        self.read_valid_cacheable_property_value(lookup)
    }

    fn read_valid_cacheable_property_value(
        &self,
        lookup: CacheablePropertyLookup,
    ) -> Result<CacheablePropertyValue> {
        match lookup.result {
            CacheablePropertyLookupResult::Hit(hit) => self
                .cacheable_hit_value(hit)
                .map(CacheablePropertyValue::Hit),
            CacheablePropertyLookupResult::Missing => Ok(CacheablePropertyValue::Missing),
            CacheablePropertyLookupResult::Uncacheable => Ok(CacheablePropertyValue::Uncacheable),
        }
    }

    fn read_cacheable_property_presence(
        &self,
        lookup: CacheablePropertyLookup,
    ) -> Result<CacheablePropertyPresence> {
        if !lookup.guard.is_valid(self)? {
            return Ok(CacheablePropertyPresence::Uncacheable);
        }
        self.read_valid_cacheable_property_presence(lookup)
    }

    fn read_valid_cacheable_property_presence(
        &self,
        lookup: CacheablePropertyLookup,
    ) -> Result<CacheablePropertyPresence> {
        match lookup.result {
            CacheablePropertyLookupResult::Hit(hit) => self
                .ensure_cacheable_hit(hit)
                .map(|()| CacheablePropertyPresence::Hit),
            CacheablePropertyLookupResult::Missing => Ok(CacheablePropertyPresence::Missing),
            CacheablePropertyLookupResult::Uncacheable => {
                Ok(CacheablePropertyPresence::Uncacheable)
            }
        }
    }

    fn cacheable_hit_value(&self, hit: CacheablePropertyHit) -> Result<Value> {
        self.ensure_cacheable_hit(hit)?;
        self.object(hit.owner)?
            .named_property_at_slot(hit.slot)
            .map(super::ObjectProperty::value)
    }

    fn ensure_cacheable_hit(&self, hit: CacheablePropertyHit) -> Result<()> {
        let object = self.object(hit.owner)?;
        if object.shape != hit.owner_shape {
            return Err(Error::runtime("cacheable property owner shape changed"));
        }
        object.named_property_at_slot(hit.slot).map(|_| ())
    }
}

impl Object {
    fn cacheable_property_hit(
        &self,
        owner: ObjectId,
        key: PropertyKey,
        depth: PrototypeLookupDepth,
        shapes: &super::ShapeTable,
    ) -> Result<Option<CacheablePropertyHit>> {
        let Some(slot) = shapes.property_slot(self.shape, key)? else {
            return Ok(None);
        };
        self.named_property_at_slot(slot)?;
        Ok(Some(CacheablePropertyHit::new(
            owner, self.shape, slot, depth,
        )))
    }

    fn named_property_at_slot(&self, slot: PropertySlot) -> Result<&super::ObjectProperty> {
        self.named_properties
            .get(slot.index())
            .map(super::runtime_object_slot::NamedProperty::property)
            .ok_or_else(|| Error::runtime("object property slot is not available"))
    }

    fn update_named_property_at_slot(&mut self, slot: PropertySlot, value: Value) -> Result<()> {
        let property = self
            .named_properties
            .get_mut(slot.index())
            .map(super::runtime_object_slot::NamedProperty::property_mut)
            .ok_or_else(|| Error::runtime("object property slot is not available"))?;
        property.set_value(value);
        Ok(())
    }

    fn has_uncacheable_own_property(&self, property: PropertyLookup<'_>) -> bool {
        if self.array_length.is_none() {
            return false;
        }
        if property.name() == ARRAY_LENGTH_PROPERTY {
            return true;
        }
        ArrayIndex::parse(property.name()).is_some_and(|index| self.has_array_element(index))
    }
}
