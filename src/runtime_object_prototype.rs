use crate::{
    error::{Error, Result},
    value::{ObjectId, Value},
};

use super::{ObjectHeap, PROTOTYPE_PROPERTY, PropertyLookup};

const PROTOTYPE_CYCLE_DETECTED_ERROR: &str = "prototype cycle detected";
const PROTOTYPE_CYCLE_SET_ERROR: &str = "prototype cycle is not allowed";

impl ObjectHeap {
    pub(super) fn prototype_get_in_chain(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
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
        let object = self.object(id)?;
        if object.has_own(property) {
            return Ok(true);
        }

        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        let mut current = object.prototype;
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if object.has_own(property) {
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
        let object = self.object_mut(id)?;
        object.prototype = prototype;
        Ok(())
    }

    fn prototype_property_value_in_chain(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<Value>> {
        let object = self.object(id)?;
        if let Some(value) = object.get_own(property) {
            return Ok(Some(value));
        }

        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        let mut current = object.prototype;
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if let Some(value) = object.get_own(property) {
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

#[derive(Debug, Clone, Copy)]
struct PrototypeTraversalBudget {
    remaining: usize,
}

impl PrototypeTraversalBudget {
    const fn from_object_count(object_count: usize) -> Self {
        Self {
            remaining: object_count,
        }
    }

    fn enter_next(&mut self) -> Result<()> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| Error::runtime(PROTOTYPE_CYCLE_DETECTED_ERROR))?;
        Ok(())
    }
}
