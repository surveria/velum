use crate::{
    error::{Error, Result},
    value::{ObjectId, Value},
};

use super::runtime_object_lookup::{
    CacheablePropertyPresence, CacheablePropertyValue, PrototypeTraversalBudget,
};
use super::{ObjectHeap, PROTOTYPE_PROPERTY, PropertyLookup};

const PROTOTYPE_CYCLE_SET_ERROR: &str = "prototype cycle is not allowed";

impl ObjectHeap {
    pub(super) fn prototype_get_in_chain(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        match self.cacheable_property_value(id, property)? {
            CacheablePropertyValue::Hit(value) => return Ok(value),
            CacheablePropertyValue::Missing => {
                if property.name() == PROTOTYPE_PROPERTY {
                    return self.prototype_value(id);
                }
                return Ok(Value::Undefined);
            }
            CacheablePropertyValue::Uncacheable => {}
        }
        if let Some(value) = self.prototype_property_value_in_chain(id, property)? {
            return Ok(value);
        }
        if property.name() == PROTOTYPE_PROPERTY {
            return self.prototype_value(id);
        }
        Ok(Value::Undefined)
    }

    pub(super) fn prototype_has_in_chain(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        match self.cacheable_property_presence(id, property)? {
            CacheablePropertyPresence::Hit => return Ok(true),
            CacheablePropertyPresence::Missing => return Ok(false),
            CacheablePropertyPresence::Uncacheable => {}
        }
        let object = self.object(id)?;
        if object.has_own(property, &self.shapes)? {
            return Ok(true);
        }

        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        let mut current = object.prototype;
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if object.has_own(property, &self.shapes)? {
                return Ok(true);
            }
            current = object.prototype;
        }
        Ok(false)
    }

    pub(super) fn set_prototype_value(&mut self, id: ObjectId, value: &Value) -> Result<()> {
        let prototype = match value {
            Value::Object(prototype) => Some(*prototype),
            Value::Null => None,
            _ => return Ok(()),
        };
        if let Some(prototype) = prototype
            && self.prototype_chain_contains(prototype, id)?
        {
            return Err(Error::runtime(PROTOTYPE_CYCLE_SET_ERROR));
        }
        let before = self.object(id)?.structure_snapshot();
        let object = self.object_mut(id)?;
        if object.prototype == prototype {
            return Ok(());
        }
        object.prototype = prototype;
        self.bump_if_structure_changed(id, before)
    }

    fn prototype_property_value_in_chain(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<Value>> {
        let object = self.object(id)?;
        if let Some(value) = object.get_own(property, &self.shapes)? {
            return Ok(Some(value));
        }

        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        let mut current = object.prototype;
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if let Some(value) = object.get_own(property, &self.shapes)? {
                return Ok(Some(value));
            }
            current = object.prototype;
        }
        Ok(None)
    }

    fn prototype_chain_contains(&self, start: ObjectId, target: ObjectId) -> Result<bool> {
        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        let mut current = Some(start);
        while let Some(current_id) = current {
            budget.enter_next()?;
            if current_id == target {
                return Ok(true);
            }
            current = self.object(current_id)?.prototype;
        }
        Ok(false)
    }

    fn prototype_value(&self, id: ObjectId) -> Result<Value> {
        let object = self.object(id)?;
        Ok(object.prototype.map_or(Value::Null, Value::Object))
    }
}
