#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::Result,
    runtime::{
        Context,
        object::{
            AccessorPropertyUpdate, OwnPropertyDescriptor, PropertyKey, PropertyLookup,
            PropertyUpdate,
        },
    },
    value::{FunctionId, HostFunctionId, NativeFunctionId, Value},
};

use super::{FUNCTION_PROTOTYPE_CALLER_PROPERTY, properties::FunctionPropertyKind};
use crate::runtime::native::NativeFunctionKind;

impl Context {
    pub(crate) fn native_function_object_prototype_value(
        &mut self,
        id: NativeFunctionId,
    ) -> Result<Value> {
        let function = self.native_function(id)?;
        let kind = function.kind();
        if matches!(kind, NativeFunctionKind::BoundFunction(_)) {
            return Ok(function.properties().prototype());
        }
        let realm = function.realm();
        self.with_realm(realm, |context| {
            context.native_function_object_prototype_in_active_realm(kind)
        })
    }

    fn native_function_object_prototype_in_active_realm(
        &mut self,
        kind: NativeFunctionKind,
    ) -> Result<Value> {
        if matches!(kind, NativeFunctionKind::TypedArray(_)) {
            return self.typed_array_intrinsic_constructor_value();
        }
        if let NativeFunctionKind::ErrorConstructor(name) = kind
            && name != crate::value::ErrorName::Base
        {
            return self.error_constructor_value(crate::value::ErrorName::Base);
        }
        if matches!(
            kind,
            NativeFunctionKind::AsyncFunction
                | NativeFunctionKind::AsyncGeneratorFunction
                | NativeFunctionKind::GeneratorFunction
        ) {
            return self.function_constructor_value();
        }
        self.function_constructor_prototype_value()
    }

    pub(crate) fn define_native_function_accessor_property_key(
        &mut self,
        id: NativeFunctionId,
        property: &str,
        key: PropertyKey,
        update: AccessorPropertyUpdate,
    ) -> Result<()> {
        let property_kind = FunctionPropertyKind::from_name(property);
        let max_properties = self.limits.max_object_properties;
        let function = self.native_function_mut(id)?;
        function.properties_mut().define_property(
            key,
            property_kind,
            PropertyUpdate::Accessor(update),
            max_properties,
        )
    }

    pub(crate) fn get_native_function_property_lookup(
        &mut self,
        id: NativeFunctionId,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let property_name = property.name();
        let property_kind = FunctionPropertyKind::from_name(property_name);
        let own_descriptor = {
            let function = self.native_function(id)?;
            let intrinsic = function
                .properties()
                .intrinsic_value(property_kind)
                .map(|value| {
                    OwnPropertyDescriptor::Data(
                        crate::runtime::object::DataPropertyDescriptor::new(
                            value,
                            crate::runtime::object::PropertyWritable::No,
                            crate::runtime::object::PropertyEnumerable::No,
                            crate::runtime::object::PropertyConfigurable::Yes,
                        ),
                    )
                });
            intrinsic
                .or_else(|| {
                    function.intrinsic_property(property_name).map(|value| {
                        OwnPropertyDescriptor::Data(
                            crate::runtime::object::DataPropertyDescriptor::new(
                                value,
                                crate::runtime::object::PropertyWritable::No,
                                crate::runtime::object::PropertyEnumerable::No,
                                crate::runtime::object::PropertyConfigurable::No,
                            ),
                        )
                    })
                })
                .or_else(|| function.properties().own_property_descriptor(property))
        };
        if let Some(descriptor) = own_descriptor {
            return match descriptor {
                OwnPropertyDescriptor::Data(descriptor) => self.checked_value(descriptor.value()),
                OwnPropertyDescriptor::Accessor(descriptor) if descriptor.has_getter() => {
                    self.call_accessor_getter(descriptor.get_ref(), receiver.clone())
                }
                OwnPropertyDescriptor::Accessor(_) => Ok(Value::Undefined),
            };
        }
        self.get_native_function_object_prototype_property(id, receiver, property)
    }

    fn get_native_function_object_prototype_property(
        &mut self,
        id: NativeFunctionId,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let kind = self.native_function(id)?.kind();
        if !matches!(kind, NativeFunctionKind::TypedArray(_))
            && !self.should_materialize_function_prototype_for(property)
            && property.name() != crate::runtime::object::PROTOTYPE_PROPERTY
        {
            return Ok(Value::Undefined);
        }
        let prototype = self.native_function_object_prototype_value(id)?;
        let Some(property) = self.known_function_prototype_lookup(property) else {
            return Ok(Value::Undefined);
        };
        let Some(read) =
            self.semantic_property_read_with_receiver(&prototype, receiver, property)?
        else {
            return Ok(Value::Undefined);
        };
        self.finish_semantic_property_read(read, receiver, property)
    }

    fn known_function_prototype_lookup<'a>(
        &self,
        property: PropertyLookup<'a>,
    ) -> Option<PropertyLookup<'a>> {
        let Some(key) = property.key() else {
            return self
                .known_property_key(property.name())
                .map(|key| PropertyLookup::from_key(property.name(), key));
        };
        Some(PropertyLookup::from_key(property.name(), key))
    }

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
            return if property.name() == FUNCTION_PROTOTYPE_CALLER_PROPERTY {
                self.legacy_function_caller_value(id)
            } else {
                self.legacy_function_arguments_value(id)
            };
        }
        let parent = if let Some(parent) = self.function_static_parent_value(id)? {
            parent
        } else {
            if !self.function_should_materialize_prototype_for(id, property)?
                && property.name() != crate::runtime::object::PROTOTYPE_PROPERTY
            {
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

    fn legacy_function_caller_value(&self, id: FunctionId) -> Result<Value> {
        let mut found_callee = false;
        let caller = self
            .activation_frames
            .iter()
            .rev()
            .filter_map(crate::runtime::activation::ActivationFrame::function_id)
            .find(|active| {
                if found_callee {
                    return true;
                }
                found_callee = *active == id;
                false
            });
        let Some(caller) = caller else {
            return Ok(Value::Null);
        };
        if self.function_uses_restricted_prototype(
            caller,
            PropertyLookup::new(FUNCTION_PROTOTYPE_CALLER_PROPERTY, None),
        )? {
            return Ok(Value::Null);
        }
        Ok(Value::Function(caller))
    }

    pub(crate) fn get_host_function_property_lookup(
        &mut self,
        id: HostFunctionId,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        if let Some(descriptor) = self.host_function_own_property_descriptor_lookup(id, property)? {
            return match descriptor {
                OwnPropertyDescriptor::Data(descriptor) => self.checked_value(descriptor.value()),
                OwnPropertyDescriptor::Accessor(descriptor) if descriptor.has_getter() => {
                    self.call_accessor_getter(descriptor.get_ref(), receiver.clone())
                }
                OwnPropertyDescriptor::Accessor(_) => Ok(Value::Undefined),
            };
        }
        let parent = self.host_function_inheritance_prototype_value(id)?;
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
        let function = self.function(id)?;
        let kind = function.kind;
        let realm = function.realm;
        self.with_realm(realm, |context| {
            context.function_object_prototype_in_active_realm(kind)
        })
    }

    fn function_object_prototype_in_active_realm(
        &mut self,
        kind: crate::syntax::FunctionKind,
    ) -> Result<Value> {
        if kind.is_async_generator() {
            return self.async_generator_function_prototype_value();
        }
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

    pub(crate) fn host_function_own_property_descriptor_lookup(
        &self,
        id: HostFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        let function = self.host_function(id)?;
        if let Some(descriptor) = function
            .properties()
            .intrinsic_descriptor(FunctionPropertyKind::from_name(property.name()))
        {
            return Ok(Some(OwnPropertyDescriptor::Data(descriptor)));
        }
        Ok(function.properties().own_property_descriptor(property))
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

    pub(crate) fn has_host_function_property_lookup(
        &self,
        id: HostFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let function = self.host_function(id)?;
        let property_kind = FunctionPropertyKind::from_name(property.name());
        if function.properties().has_intrinsic(property_kind) {
            return Ok(true);
        }
        Ok(function.properties().has(property))
    }

    pub(crate) fn has_host_function_property_including_prototype_lookup(
        &mut self,
        id: HostFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        if self.has_host_function_property_lookup(id, property)? {
            return Ok(true);
        }
        let parent = self.host_function_inheritance_prototype_value(id)?;
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

    pub(crate) fn define_host_function_property_key(
        &mut self,
        id: HostFunctionId,
        property: &str,
        key: PropertyKey,
        update: PropertyUpdate,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let property_kind = FunctionPropertyKind::from_name(property);
        self.host_function_mut(id)?
            .properties_mut()
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

    pub(crate) fn delete_host_function_property_lookup(
        &mut self,
        id: HostFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let property_kind = FunctionPropertyKind::from_name(property.name());
        self.host_function_mut(id)?
            .properties_mut()
            .delete(property, property_kind)
    }

    pub(crate) fn function_own_keys(
        &self,
        id: FunctionId,
    ) -> Result<(Vec<String>, Vec<crate::storage::symbol::SymbolId>)> {
        let function = self.function(id)?;
        function.properties.own_keys(&self.atoms)
    }

    pub(crate) fn native_function_own_keys(
        &self,
        id: NativeFunctionId,
    ) -> Result<(Vec<String>, Vec<crate::storage::symbol::SymbolId>)> {
        let function = self.native_function(id)?;
        function.properties().own_keys(&self.atoms)
    }

    pub(crate) fn host_function_own_keys(
        &self,
        id: HostFunctionId,
    ) -> Result<(Vec<String>, Vec<crate::storage::symbol::SymbolId>)> {
        self.host_function(id)?.properties().own_keys(&self.atoms)
    }

    pub(in crate::runtime) fn function_is_extensible(&self, id: FunctionId) -> Result<bool> {
        Ok(self.function(id)?.properties.is_extensible())
    }

    pub(in crate::runtime) fn prevent_function_extensions(&mut self, id: FunctionId) -> Result<()> {
        self.function_mut(id)?.properties.prevent_extensions();
        Ok(())
    }

    pub(in crate::runtime) fn seal_function(&mut self, id: FunctionId) -> Result<()> {
        self.function_mut(id)?.properties.seal();
        Ok(())
    }

    pub(in crate::runtime) fn freeze_function(&mut self, id: FunctionId) -> Result<()> {
        self.function_mut(id)?.properties.freeze();
        Ok(())
    }

    pub(in crate::runtime) fn function_is_sealed(&self, id: FunctionId) -> Result<bool> {
        Ok(self.function(id)?.properties.is_sealed())
    }

    pub(in crate::runtime) fn function_is_frozen(&self, id: FunctionId) -> Result<bool> {
        Ok(self.function(id)?.properties.is_frozen())
    }

    pub(in crate::runtime) fn native_function_is_extensible(
        &self,
        id: NativeFunctionId,
    ) -> Result<bool> {
        Ok(self.native_function(id)?.properties().is_extensible())
    }

    pub(in crate::runtime) fn prevent_native_function_extensions(
        &mut self,
        id: NativeFunctionId,
    ) -> Result<()> {
        self.native_function_mut(id)?
            .properties_mut()
            .prevent_extensions();
        Ok(())
    }

    pub(in crate::runtime) fn seal_native_function(&mut self, id: NativeFunctionId) -> Result<()> {
        self.native_function_mut(id)?.properties_mut().seal();
        Ok(())
    }

    pub(in crate::runtime) fn freeze_native_function(
        &mut self,
        id: NativeFunctionId,
    ) -> Result<()> {
        self.native_function_mut(id)?.properties_mut().freeze();
        Ok(())
    }

    pub(in crate::runtime) fn native_function_is_sealed(
        &self,
        id: NativeFunctionId,
    ) -> Result<bool> {
        Ok(self.native_function(id)?.properties().is_sealed())
    }

    pub(in crate::runtime) fn native_function_is_frozen(
        &self,
        id: NativeFunctionId,
    ) -> Result<bool> {
        Ok(self.native_function(id)?.properties().is_frozen())
    }

    pub(in crate::runtime) fn host_function_is_extensible(
        &self,
        id: HostFunctionId,
    ) -> Result<bool> {
        Ok(self.host_function(id)?.properties().is_extensible())
    }

    pub(in crate::runtime) fn prevent_host_function_extensions(
        &mut self,
        id: HostFunctionId,
    ) -> Result<()> {
        self.host_function_mut(id)?
            .properties_mut()
            .prevent_extensions();
        Ok(())
    }

    pub(in crate::runtime) fn seal_host_function(&mut self, id: HostFunctionId) -> Result<()> {
        self.host_function_mut(id)?.properties_mut().seal();
        Ok(())
    }

    pub(in crate::runtime) fn freeze_host_function(&mut self, id: HostFunctionId) -> Result<()> {
        self.host_function_mut(id)?.properties_mut().freeze();
        Ok(())
    }

    pub(in crate::runtime) fn host_function_is_sealed(&self, id: HostFunctionId) -> Result<bool> {
        Ok(self.host_function(id)?.properties().is_sealed())
    }

    pub(in crate::runtime) fn host_function_is_frozen(&self, id: HostFunctionId) -> Result<bool> {
        Ok(self.host_function(id)?.properties().is_frozen())
    }

    pub(in crate::runtime) fn host_function_inheritance_prototype_value(
        &self,
        id: HostFunctionId,
    ) -> Result<Value> {
        Ok(self.host_function(id)?.properties().prototype())
    }

    pub(in crate::runtime) fn try_set_host_function_inheritance_prototype(
        &mut self,
        id: HostFunctionId,
        prototype: Value,
    ) -> Result<bool> {
        Ok(self
            .host_function_mut(id)?
            .properties_mut()
            .try_set_inheritance_prototype(prototype))
    }

    pub(in crate::runtime) fn set_function_static_parent(
        &mut self,
        id: FunctionId,
        parent: Value,
    ) -> Result<()> {
        self.function_mut(id)?.static_parent = Some(parent);
        Ok(())
    }

    pub(in crate::runtime) fn try_set_function_static_parent(
        &mut self,
        id: FunctionId,
        parent: Value,
    ) -> Result<bool> {
        let current = self.function_inheritance_prototype_value(id)?;
        if crate::runtime::abstract_operations::same_value(&current, &parent) {
            return Ok(true);
        }
        if !self.function_is_extensible(id)? {
            return Ok(false);
        }
        self.set_function_static_parent(id, parent)?;
        Ok(true)
    }
}
