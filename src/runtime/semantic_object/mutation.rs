use crate::{
    error::Result,
    runtime::{
        Context,
        object::{
            DataPropertyDescriptor, DataPropertyUpdate, OwnPropertyDescriptor,
            PropertyConfigurable, PropertyEnumerable, PropertyKey, PropertyLookup, PropertyUpdate,
            PropertyWritable,
        },
        property::{DynamicPropertyKey, delete_property, set_property},
    },
    value::Value,
};

use super::{SemanticPropertyDelete, SemanticPropertyWrite};

impl Context {
    /// Runs shared object-like `[[Set]]` pre-dispatch. Only an ordinary object
    /// reaches the tail consumed by storage or an inline cache.
    pub(in crate::runtime) fn semantic_property_write(
        &mut self,
        object: &Value,
        property: PropertyLookup<'_>,
        value: Value,
    ) -> Result<Option<SemanticPropertyWrite>> {
        let Some(object_ref) = self.semantic_object_ref(object)? else {
            return Ok(None);
        };
        let write = match object_ref.value {
            Value::Object(id) => {
                if self.objects.is_proxy(*id) {
                    SemanticPropertyWrite::Resolved(self.proxy_set(
                        *id,
                        property,
                        value,
                        object.clone(),
                    )?)
                } else {
                    SemanticPropertyWrite::ObjectTail(*id)
                }
            }
            Value::Function(id) => {
                let key = self.semantic_property_key(property)?;
                self.set_function_property_key(*id, property.name(), key, value)?;
                SemanticPropertyWrite::Resolved(true)
            }
            Value::NativeFunction(id) => {
                let key = self.semantic_property_key(property)?;
                self.set_native_function_property_key(*id, property.name(), key, value)?;
                SemanticPropertyWrite::Resolved(true)
            }
            Value::HostFunction(_) | Value::Error(_) => {
                let key = self.semantic_property_key(property)?;
                set_property(
                    &mut self.objects,
                    object,
                    key,
                    property.name(),
                    value,
                    self.limits.max_object_properties,
                )?;
                SemanticPropertyWrite::Resolved(true)
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        };
        Ok(Some(write))
    }

    /// Finishes an object-like write after an optimizer declines the ordinary
    /// object tail. Accessor lookup stays on the semantic slow path.
    pub(in crate::runtime) fn finish_semantic_property_write(
        &mut self,
        write: SemanticPropertyWrite,
        property: PropertyLookup<'_>,
        value: Value,
    ) -> Result<bool> {
        match write {
            SemanticPropertyWrite::Resolved(updated) => Ok(updated),
            SemanticPropertyWrite::ObjectTail(id) => {
                let key = self.semantic_property_key(property)?;
                self.write_ordinary_object_property_with_accessors(
                    id,
                    key,
                    property.name(),
                    value,
                )?;
                Ok(true)
            }
        }
    }

    /// Runs shared object-like `[[Delete]]` pre-dispatch and returns an
    /// ordinary tail only after Proxy/function/native behavior is resolved.
    pub(in crate::runtime) fn semantic_property_delete(
        &mut self,
        object: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Option<SemanticPropertyDelete>> {
        let Some(object_ref) = self.semantic_object_ref(object)? else {
            return Ok(None);
        };
        let deletion = match object_ref.value {
            Value::Object(id) => {
                if self.objects.is_proxy(*id) {
                    SemanticPropertyDelete::Resolved(self.proxy_delete(*id, property)?)
                } else {
                    SemanticPropertyDelete::ObjectTail(*id)
                }
            }
            Value::Function(id) => SemanticPropertyDelete::Resolved(
                self.delete_function_property_lookup(*id, property)?,
            ),
            Value::NativeFunction(id) => SemanticPropertyDelete::Resolved(
                self.delete_native_function_property_lookup(*id, property)?,
            ),
            Value::HostFunction(_) | Value::Error(_) => SemanticPropertyDelete::Resolved(
                delete_property(&mut self.objects, object, property)?,
            ),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        };
        Ok(Some(deletion))
    }

    /// Finishes a shared delete after an optimizer declines the ordinary tail.
    pub(in crate::runtime) fn finish_semantic_property_delete(
        &mut self,
        deletion: SemanticPropertyDelete,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        match deletion {
            SemanticPropertyDelete::Resolved(deleted) => Ok(deleted),
            SemanticPropertyDelete::ObjectTail(id) => self.objects.delete(id, property),
        }
    }

    pub(in crate::runtime) fn delete_property_value_with_lookup(
        &mut self,
        object: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        if let Some(deletion) = self.semantic_property_delete(object, property)? {
            return self.finish_semantic_property_delete(deletion, property);
        }
        delete_property(&mut self.objects, object, property)
    }

    /// Spec-shaped `[[Set]]` recursion used by `Reflect.set` and Proxy default dispatch.
    /// It preserves an explicit receiver across descriptors and prototypes.
    pub(in crate::runtime) fn semantic_reflect_property_write(
        &mut self,
        target: &Value,
        property: &mut DynamicPropertyKey,
        value: Value,
        receiver: &Value,
    ) -> Result<Option<bool>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return Ok(None);
        };
        if let Value::Object(id) = object_ref.value
            && self.objects.is_proxy(*id)
        {
            return self
                .proxy_set(*id, property.lookup(), value, receiver.clone())
                .map(Some);
        }
        if matches!(object_ref.value, Value::HostFunction(_)) {
            return Ok(Some(false));
        }
        if let Some(descriptor) = self.semantic_own_property_descriptor(target, property)? {
            return self
                .reflect_write_with_descriptor(property, value, receiver, descriptor)
                .map(Some);
        }
        if let Some(prototype) = self.semantic_get_prototype(target)?
            && !matches!(prototype, Value::Null)
        {
            self.step()?;
            return self.semantic_reflect_property_write(&prototype, property, value, receiver);
        }
        self.reflect_define_receiver_property(property, value, receiver)
            .map(Some)
    }

    fn reflect_write_with_descriptor(
        &mut self,
        property: &mut DynamicPropertyKey,
        value: Value,
        receiver: &Value,
        descriptor: OwnPropertyDescriptor,
    ) -> Result<bool> {
        match descriptor {
            OwnPropertyDescriptor::Data(descriptor) => {
                if !descriptor.writable().is_yes() {
                    return Ok(false);
                }
                self.reflect_define_receiver_property(property, value, receiver)
            }
            OwnPropertyDescriptor::Accessor(descriptor) => {
                if !descriptor.has_setter() {
                    return Ok(false);
                }
                self.call_accessor_function(&descriptor.set(), receiver.clone(), &[value])?;
                Ok(true)
            }
        }
    }

    fn reflect_define_receiver_property(
        &mut self,
        property: &mut DynamicPropertyKey,
        value: Value,
        receiver: &Value,
    ) -> Result<bool> {
        if self.semantic_object_ref(receiver)?.is_none()
            || matches!(receiver, Value::HostFunction(_) | Value::Error(_))
        {
            return Ok(false);
        }
        let mut new_property = true;
        if let Some(descriptor) = self.semantic_own_property_descriptor(receiver, property)? {
            new_property = false;
            match descriptor {
                OwnPropertyDescriptor::Accessor(_) => return Ok(false),
                OwnPropertyDescriptor::Data(descriptor) if !descriptor.writable().is_yes() => {
                    return Ok(false);
                }
                OwnPropertyDescriptor::Data(_) => {}
            }
        }
        let update = DataPropertyUpdate::new(
            Some(value.clone()),
            new_property.then_some(PropertyWritable::Yes),
            new_property.then_some(PropertyEnumerable::Yes),
            new_property.then_some(PropertyConfigurable::Yes),
        );
        let complete = DataPropertyDescriptor::new(
            value,
            PropertyWritable::Yes,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        );
        let descriptor_value =
            self.create_property_descriptor_object(&OwnPropertyDescriptor::Data(complete))?;
        self.semantic_define_own_property_update_with_descriptor(
            receiver,
            property,
            PropertyUpdate::Data(update),
            &descriptor_value,
        )
    }

    fn semantic_property_key(&mut self, property: PropertyLookup<'_>) -> Result<PropertyKey> {
        if let Some(key) = property.key() {
            return Ok(key);
        }
        self.intern_property_key(property.name())
    }
}
