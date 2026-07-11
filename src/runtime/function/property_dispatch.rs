use crate::{
    error::Result,
    runtime::{
        Context,
        object::{OwnPropertyDescriptor, PropertyKey, PropertyLookup, PropertyUpdate},
        property::get_property,
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
        if let Some(parent) = self.function_static_parent_value(id)? {
            if matches!(parent, Value::Null | Value::Undefined) {
                return Ok(Value::Undefined);
            }
            let Some(read) =
                self.semantic_property_read_with_receiver(&parent, receiver, property)?
            else {
                return Ok(Value::Undefined);
            };
            return self.finish_semantic_property_read(read, receiver, property);
        }
        self.get_function_object_prototype_property(id, property)
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

    fn get_function_object_prototype_property(
        &mut self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        if !self.should_materialize_function_prototype_for(property) {
            return Ok(Value::Undefined);
        }
        let prototype = self.function_object_prototype_value(id)?;
        let Some(property) = self.known_function_prototype_lookup(property) else {
            return Ok(Value::Undefined);
        };
        let value = get_property(&self.objects, &prototype, property)?;
        self.runtime_property_value(value)
    }

    pub(crate) fn function_object_prototype_value(&mut self, id: FunctionId) -> Result<Value> {
        let is_async = self.function(id)?.is_async;
        if is_async {
            return self.async_function_constructor_prototype_value();
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
        if property_kind.is_intrinsic_slot() && function.properties.has_intrinsic(property_kind) {
            return Ok(true);
        }
        Ok((property_kind.is_prototype() && function.constructable)
            || function.properties.has(property))
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
