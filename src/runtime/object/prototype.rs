use crate::{
    error::{Error, Result},
    value::{ObjectId, Value},
};

use super::property::{
    CacheablePropertyPresence, CacheablePropertyValue, PrototypeTraversalBudget,
};
use super::{ObjectHeap, ObjectPropertyValue, PropertyLookup};

const PROTOTYPE_CYCLE_SET_ERROR: &str = "prototype cycle is not allowed";

impl ObjectHeap {
    pub(super) fn prototype_get_in_chain(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<ObjectPropertyValue> {
        match self.cacheable_property_value(id, property)? {
            CacheablePropertyValue::Hit(value) => return Ok(ObjectPropertyValue::Value(value)),
            CacheablePropertyValue::Missing => {
                return Ok(ObjectPropertyValue::Value(Value::Undefined));
            }
            CacheablePropertyValue::Uncacheable => {}
        }
        if let Some(value) = self.prototype_property_value_in_chain(id, property)? {
            return Ok(value);
        }
        Ok(ObjectPropertyValue::Value(Value::Undefined))
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
        let mut current = object.ordinary_prototype_id();
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if object.has_own(property, &self.shapes)? {
                return Ok(true);
            }
            current = object.ordinary_prototype_id();
        }
        Ok(false)
    }

    pub(crate) fn set_prototype_value(&mut self, id: ObjectId, value: &Value) -> Result<()> {
        let prototype = match value {
            Value::Object(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => Some(value.clone()),
            Value::Null => None,
            _ => return Ok(()),
        };
        if self.object_prototype == Some(id) && prototype.is_some() {
            return Err(Error::type_error(
                "Object.prototype has an immutable prototype",
            ));
        }
        if let Some(Value::Object(prototype)) = prototype
            && self.prototype_chain_contains(prototype, id)?
        {
            return Err(Error::type_error(PROTOTYPE_CYCLE_SET_ERROR));
        }
        let before = self.object(id)?.structure_snapshot();
        let object = self.object_mut(id)?;
        if object.prototype == prototype {
            return Ok(());
        }
        if !object.extensibility.is_extensible() {
            return Err(Error::type_error(
                "cannot change prototype of non-extensible object",
            ));
        }
        object.prototype = prototype;
        self.bump_if_structure_changed(id, &before)
    }

    pub(crate) fn try_set_prototype_value(&mut self, id: ObjectId, value: &Value) -> Result<bool> {
        let prototype = match value {
            Value::Object(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => Some(value.clone()),
            Value::Null => None,
            _ => return Ok(true),
        };
        if self.object_prototype == Some(id) && prototype.is_some() {
            return Ok(false);
        }
        if let Some(Value::Object(prototype)) = prototype
            && self.prototype_chain_contains(prototype, id)?
        {
            return Ok(false);
        }
        let before = self.object(id)?.structure_snapshot();
        let object = self.object_mut(id)?;
        if object.prototype == prototype {
            return Ok(true);
        }
        if !object.extensibility.is_extensible() {
            return Ok(false);
        }
        object.prototype = prototype;
        self.bump_if_structure_changed(id, &before)?;
        Ok(true)
    }

    pub(crate) fn prototype_chain_has_object(
        &self,
        id: ObjectId,
        target: ObjectId,
    ) -> Result<bool> {
        let object = self.object(id)?;
        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        let mut current = object.ordinary_prototype_id();
        while let Some(current_id) = current {
            budget.enter_next()?;
            if current_id == target {
                return Ok(true);
            }
            current = self.object(current_id)?.ordinary_prototype_id();
        }
        Ok(false)
    }

    fn prototype_property_value_in_chain(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<ObjectPropertyValue>> {
        let object = self.object(id)?;
        if let Some(value) = object.get_own(property, &self.shapes)? {
            return Ok(Some(value));
        }

        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        let mut current = object.ordinary_prototype_id();
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if let Some(value) = object.get_own(property, &self.shapes)? {
                return Ok(Some(value));
            }
            current = object.ordinary_prototype_id();
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
            current = self.object(current_id)?.ordinary_prototype_id();
        }
        Ok(false)
    }

    pub(crate) fn prototype_value(&self, id: ObjectId) -> Result<Value> {
        let object = self.object(id)?;
        // Copy object-like handles directly so ordinary prototype reads avoid
        // the generic clone path used for heap-backed primitive values.
        match object.prototype.as_ref() {
            Some(Value::Object(id)) => Ok(Value::Object(*id)),
            Some(Value::Function(id)) => Ok(Value::Function(*id)),
            Some(Value::NativeFunction(id)) => Ok(Value::NativeFunction(*id)),
            Some(Value::HostFunction(id)) => Ok(Value::HostFunction(*id)),
            Some(value) => Ok(value.clone()),
            None => Ok(Value::Null),
        }
    }

    pub(crate) fn prototype_chain_requires_semantic_index_write(
        &self,
        id: ObjectId,
    ) -> Result<bool> {
        let root = self.object(id)?;
        if root.proxy_value.is_some() || root.has_semantic_prototype() {
            return Ok(true);
        }
        let mut current = root.ordinary_prototype_id();
        let mut budget = PrototypeTraversalBudget::from_object_count(self.objects.len());
        while let Some(current_id) = current {
            budget.enter_next()?;
            let object = self.object(current_id)?;
            if object.typed_array.is_some() || object.proxy_value.is_some() {
                return Ok(true);
            }
            if object.has_semantic_prototype() {
                return Ok(true);
            }
            current = object.ordinary_prototype_id();
        }
        Ok(false)
    }
}
