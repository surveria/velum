use crate::{
    error::Result,
    runtime::{
        Context,
        object::{OwnPropertyDescriptor, PropertyKey, PropertyLookup, PropertyUpdate},
    },
    value::{FunctionId, Value},
};

use super::properties::FunctionPropertyKind;

impl Context {
    pub(crate) fn get_function_property_lookup(
        &mut self,
        id: FunctionId,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        if let Some(descriptor) = self.function_own_property_descriptor_lookup(id, property)? {
            return match descriptor {
                OwnPropertyDescriptor::Data(descriptor) => self.checked_value(descriptor.value()),
                OwnPropertyDescriptor::Accessor(descriptor) if descriptor.has_getter() => {
                    self.call_accessor_getter(descriptor.get_ref(), receiver.clone())
                }
                OwnPropertyDescriptor::Accessor(_) => Ok(Value::Undefined),
            };
        }
        if Self::is_restricted_property(property)
            && !self.function_uses_restricted_prototype(id, property)?
        {
            return Ok(Value::Undefined);
        }
        let parent = if let Some(parent) = self.function_static_parent_value(id)? {
            parent
        } else {
            if !self.function_should_materialize_prototype_for(id, property)? {
                return Ok(Value::Undefined);
            }
            self.function_object_prototype_value(id)?
        };
        if matches!(parent, Value::Null | Value::Undefined) {
            return Ok(Value::Undefined);
        }
        let property = self
            .known_function_prototype_lookup(property)
            .unwrap_or(property);
        let Some(read) = self.semantic_property_read_with_receiver(&parent, receiver, property)?
        else {
            return Ok(Value::Undefined);
        };
        self.finish_semantic_property_read(read, receiver, property)
    }

    pub(in crate::runtime) fn function_inheritance_prototype_value(
        &mut self,
        id: FunctionId,
    ) -> Result<Value> {
        if let Some(parent) = self.function_static_parent_value(id)? {
            return Ok(parent);
        }
        self.function_object_prototype_value(id)
    }

    pub(in crate::runtime) fn function_static_parent_value(
        &self,
        id: FunctionId,
    ) -> Result<Option<Value>> {
        self.function(id)
            .map(|function| function.static_parent.clone())
    }

    pub(crate) fn function_object_prototype_value(&mut self, id: FunctionId) -> Result<Value> {
        let kind = self.function(id)?.kind;
        if kind.is_async() {
            return self.async_function_constructor_prototype_value();
        }
        if kind.is_generator() {
            return self.generator_function_prototype_value();
        }
        self.function_constructor_prototype_value()
    }

    pub(crate) fn function_own_property_descriptor_lookup(
        &self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        let function = self.function(id)?;
        if let Some(descriptor) = function
            .properties
            .intrinsic_descriptor(FunctionPropertyKind::from_name(property.name()))
        {
            return Ok(Some(OwnPropertyDescriptor::Data(descriptor)));
        }
        Ok(function.properties.own_property_descriptor(property))
    }

    pub(crate) fn has_function_property_lookup(
        &self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let function = self.function(id)?;
        let property_kind = FunctionPropertyKind::from_name(property.name());
        if function.properties.has_intrinsic(property_kind) {
            return Ok(true);
        }
        Ok(function.properties.has(property))
    }

    pub(crate) fn has_function_property_including_prototype_lookup(
        &mut self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        if self.has_function_property_lookup(id, property)? {
            return Ok(true);
        }
        let parent = if let Some(parent) = self.function_static_parent_value(id)? {
            parent
        } else {
            if !self.function_should_materialize_prototype_for(id, property)? {
                return Ok(false);
            }
            self.function_object_prototype_value(id)?
        };
        if matches!(parent, Value::Null | Value::Undefined) {
            return Ok(false);
        }
        let property = self
            .known_function_prototype_lookup(property)
            .unwrap_or(property);
        let Some(presence) = self.semantic_property_presence(&parent, property)? else {
            return Ok(false);
        };
        self.finish_semantic_property_presence(presence, property)
    }

    pub(crate) fn set_function_property_key(
        &mut self,
        id: FunctionId,
        property: &str,
        key: PropertyKey,
        value: Value,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let property_kind = FunctionPropertyKind::from_name(property);
        let function = self.function_mut(id)?;
        function
            .properties
            .set(key, property_kind, value, max_properties)
    }

    pub(crate) fn define_function_property_key(
        &mut self,
        id: FunctionId,
        property: &str,
        key: PropertyKey,
        update: PropertyUpdate,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let property_kind = FunctionPropertyKind::from_name(property);
        let function = self.function_mut(id)?;
        function
            .properties
            .define_property(key, property_kind, update, max_properties)
    }

    pub(crate) fn delete_function_property_lookup(
        &mut self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let property_kind = FunctionPropertyKind::from_name(property.name());
        let function = self.function_mut(id)?;
        function.properties.delete(property, property_kind)
    }

    pub(crate) fn function_enumerable_keys(&self, id: FunctionId) -> Result<Vec<String>> {
        let function = self.function(id)?;
        function.properties.keys(&self.atoms)
    }

    pub(in crate::runtime) fn set_function_static_parent(
        &mut self,
        id: FunctionId,
        parent: Value,
    ) -> Result<()> {
        self.function_mut(id)?.static_parent = Some(parent);
        Ok(())
    }
}
