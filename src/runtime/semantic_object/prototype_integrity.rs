use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            AccessorPropertyDescriptor, AccessorPropertyUpdate, DataPropertyDescriptor,
            DataPropertyUpdate, OwnPropertyDescriptor, PropertyConfigurable, PropertyUpdate,
            PropertyWritable,
        },
    },
    value::Value,
};

#[derive(Clone, Copy)]
pub(in crate::runtime) enum SemanticIntegrityLevel {
    Sealed,
    Frozen,
}

impl Context {
    pub(in crate::runtime) fn semantic_get_prototype(
        &mut self,
        target: &Value,
    ) -> Result<Option<Value>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return Ok(None);
        };
        let prototype = match object_ref.value {
            Value::Object(id) if self.objects.is_proxy(*id) => self.proxy_get_prototype_of(*id)?,
            Value::Object(id) => self.objects.prototype_value(*id)?,
            Value::Function(id) => self.function_object_prototype_value(*id)?,
            Value::NativeFunction(id) => self.native_function_object_prototype_value(*id)?,
            Value::Error(error) => self
                .error_constructor_prototype(error.name())
                .map(Value::Object)?,
            Value::HostFunction(_) => {
                return Err(Error::runtime("host function prototype is not available"));
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        };
        Ok(Some(prototype))
    }

    pub(in crate::runtime) fn semantic_try_set_prototype(
        &mut self,
        target: &Value,
        prototype: Value,
    ) -> Result<Option<bool>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return Ok(None);
        };
        let updated = match object_ref.value {
            Value::Object(id) if self.objects.is_proxy(*id) => {
                self.proxy_set_prototype_of(*id, prototype)?
            }
            Value::Object(id) => self.objects.try_set_prototype_value(*id, &prototype)?,
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => false,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        };
        Ok(Some(updated))
    }

    pub(in crate::runtime) fn semantic_is_extensible(
        &mut self,
        target: &Value,
    ) -> Result<Option<bool>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return Ok(None);
        };
        let extensible = match object_ref.value {
            Value::Object(id) if self.objects.is_proxy(*id) => self.proxy_is_extensible(*id)?,
            Value::Object(id) => self.objects.is_extensible(*id)?,
            Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_) => true,
            Value::Error(_) => false,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        };
        Ok(Some(extensible))
    }

    pub(in crate::runtime) fn semantic_prevent_extensions(
        &mut self,
        target: &Value,
    ) -> Result<Option<bool>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return Ok(None);
        };
        let prevented = match object_ref.value {
            Value::Object(id) if self.objects.is_proxy(*id) => {
                self.proxy_prevent_extensions(*id)?
            }
            Value::Object(id) => {
                self.objects.prevent_extensions(*id)?;
                true
            }
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => true,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        };
        Ok(Some(prevented))
    }

    pub(in crate::runtime) fn semantic_set_integrity_level(
        &mut self,
        target: &Value,
        level: SemanticIntegrityLevel,
    ) -> Result<Option<bool>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return Ok(None);
        };
        match object_ref.value {
            Value::Object(id) if !self.objects.is_proxy(*id) => {
                match level {
                    SemanticIntegrityLevel::Sealed => self.objects.seal(*id)?,
                    SemanticIntegrityLevel::Frozen => self.objects.freeze(*id)?,
                }
                return Ok(Some(true));
            }
            Value::Object(_) => {}
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => return Ok(Some(true)),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        }
        if !self.semantic_prevent_extensions(target)?.unwrap_or(false) {
            return Ok(Some(false));
        }
        for key in self.semantic_own_property_keys(target)? {
            let mut property = self.dynamic_property_key(&key)?;
            let Some(descriptor) = self.semantic_own_property_descriptor(target, &property)? else {
                continue;
            };
            let (update, complete) = Self::integrity_property_update(descriptor, level);
            let descriptor_value = self.create_property_descriptor_object(&complete)?;
            if !self.semantic_define_own_property_update_with_descriptor(
                target,
                &mut property,
                update,
                &descriptor_value,
            )? {
                return Ok(Some(false));
            }
        }
        Ok(Some(true))
    }

    pub(in crate::runtime) fn semantic_test_integrity_level(
        &mut self,
        target: &Value,
        level: SemanticIntegrityLevel,
    ) -> Result<Option<bool>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return Ok(None);
        };
        match object_ref.value {
            Value::Object(id) if !self.objects.is_proxy(*id) => {
                return match level {
                    SemanticIntegrityLevel::Sealed => self.objects.is_sealed(*id).map(Some),
                    SemanticIntegrityLevel::Frozen => self.objects.is_frozen(*id).map(Some),
                };
            }
            Value::Object(_) => {}
            Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_) => {
                return Ok(Some(false));
            }
            Value::Error(_) => return Ok(Some(true)),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Ok(None),
        }
        if self.semantic_is_extensible(target)?.unwrap_or(false) {
            return Ok(Some(false));
        }
        for key in self.semantic_own_property_keys(target)? {
            let property = self.dynamic_property_key(&key)?;
            let Some(descriptor) = self.semantic_own_property_descriptor(target, &property)? else {
                continue;
            };
            if Self::integrity_descriptor_fails(&descriptor, level) {
                return Ok(Some(false));
            }
        }
        Ok(Some(true))
    }

    fn integrity_property_update(
        descriptor: OwnPropertyDescriptor,
        level: SemanticIntegrityLevel,
    ) -> (PropertyUpdate, OwnPropertyDescriptor) {
        match descriptor {
            OwnPropertyDescriptor::Data(descriptor) => {
                let writable = match level {
                    SemanticIntegrityLevel::Sealed => descriptor.writable(),
                    SemanticIntegrityLevel::Frozen => PropertyWritable::No,
                };
                let complete = DataPropertyDescriptor::new(
                    descriptor.value(),
                    writable,
                    descriptor.enumerable(),
                    PropertyConfigurable::No,
                );
                let update = PropertyUpdate::Data(DataPropertyUpdate::new(
                    None,
                    matches!(level, SemanticIntegrityLevel::Frozen).then_some(PropertyWritable::No),
                    None,
                    Some(PropertyConfigurable::No),
                ));
                (update, OwnPropertyDescriptor::Data(complete))
            }
            OwnPropertyDescriptor::Accessor(descriptor) => {
                let complete = AccessorPropertyDescriptor::new(
                    descriptor.get(),
                    descriptor.set(),
                    descriptor.enumerable(),
                    PropertyConfigurable::No,
                );
                let update = PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    None,
                    None,
                    None,
                    Some(PropertyConfigurable::No),
                ));
                (update, OwnPropertyDescriptor::Accessor(complete))
            }
        }
    }

    const fn integrity_descriptor_fails(
        descriptor: &OwnPropertyDescriptor,
        level: SemanticIntegrityLevel,
    ) -> bool {
        match descriptor {
            OwnPropertyDescriptor::Data(descriptor) => {
                descriptor.configurable().is_yes()
                    || (matches!(level, SemanticIntegrityLevel::Frozen)
                        && descriptor.writable().is_yes())
            }
            OwnPropertyDescriptor::Accessor(descriptor) => descriptor.configurable().is_yes(),
        }
    }
}
