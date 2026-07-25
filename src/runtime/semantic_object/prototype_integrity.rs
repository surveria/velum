use crate::{
    error::Result,
    runtime::{
        Context,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, OwnPropertyDescriptor,
            PropertyConfigurable, PropertyUpdate, PropertyWritable,
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
            Value::Function(id) => self.function_inheritance_prototype_value(*id)?,
            Value::NativeFunction(id) => self.native_function_object_prototype_value(*id)?,
            Value::HostFunction(id) => self.host_function_inheritance_prototype_value(*id)?,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
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
        if !matches!(prototype, Value::Null) && self.semantic_object_ref(&prototype)?.is_none() {
            return Ok(Some(false));
        }
        let updated = match object_ref.value {
            Value::Object(id) if self.objects.is_proxy(*id) => {
                self.proxy_set_prototype_of(*id, prototype)?
            }
            Value::Object(id) => {
                if self.semantic_prototype_chain_contains(&prototype, target)? {
                    false
                } else {
                    self.objects.try_set_prototype_value(*id, &prototype)?
                }
            }
            Value::Function(id) => {
                if self.semantic_prototype_chain_contains(&prototype, target)? {
                    false
                } else {
                    self.try_set_function_static_parent(*id, prototype)?
                }
            }
            Value::NativeFunction(id) => {
                if self.semantic_prototype_chain_contains(&prototype, target)? {
                    false
                } else {
                    self.try_set_native_function_static_parent(*id, prototype)?
                }
            }
            Value::HostFunction(id) => {
                if self.semantic_prototype_chain_contains(&prototype, target)? {
                    false
                } else {
                    self.try_set_host_function_inheritance_prototype(*id, prototype)?
                }
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
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
            Value::Function(id) => self.function_is_extensible(*id)?,
            Value::NativeFunction(id) => self.native_function_is_extensible(*id)?,
            Value::HostFunction(id) => self.host_function_is_extensible(*id)?,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
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
                if self
                    .objects
                    .typed_array(*id)?
                    .is_some_and(|view| !view.can_prevent_extensions())
                {
                    return Ok(Some(false));
                }
                self.objects.prevent_extensions(*id)?;
                true
            }
            Value::Function(id) => {
                self.prevent_function_extensions(*id)?;
                true
            }
            Value::NativeFunction(id) => {
                self.prevent_native_function_extensions(*id)?;
                true
            }
            Value::HostFunction(id) => {
                self.prevent_host_function_extensions(*id)?;
                true
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
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
            Value::Object(id)
                if !self.objects.is_proxy(*id)
                    && self.objects.typed_array(*id)?.is_none()
                    && !self.objects.is_module_namespace(*id)? =>
            {
                match level {
                    SemanticIntegrityLevel::Sealed => self.objects.seal(*id)?,
                    SemanticIntegrityLevel::Frozen => self.objects.freeze(*id)?,
                }
                return Ok(Some(true));
            }
            Value::Object(_) => {}
            Value::Function(id) => {
                match level {
                    SemanticIntegrityLevel::Sealed => self.seal_function(*id)?,
                    SemanticIntegrityLevel::Frozen => self.freeze_function(*id)?,
                }
                return Ok(Some(true));
            }
            Value::NativeFunction(id) => {
                match level {
                    SemanticIntegrityLevel::Sealed => self.seal_native_function(*id)?,
                    SemanticIntegrityLevel::Frozen => self.freeze_native_function(*id)?,
                }
                return Ok(Some(true));
            }
            Value::HostFunction(id) => {
                match level {
                    SemanticIntegrityLevel::Sealed => self.seal_host_function(*id)?,
                    SemanticIntegrityLevel::Frozen => self.freeze_host_function(*id)?,
                }
                return Ok(Some(true));
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
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
            let update = Self::integrity_property_update(&descriptor, level);
            let descriptor_value = self.create_property_update_object(&update)?;
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
            Value::Object(id)
                if !self.objects.is_proxy(*id)
                    && self.objects.typed_array(*id)?.is_none()
                    && !self.objects.is_module_namespace(*id)? =>
            {
                return match level {
                    SemanticIntegrityLevel::Sealed => self.objects.is_sealed(*id).map(Some),
                    SemanticIntegrityLevel::Frozen => self.objects.is_frozen(*id).map(Some),
                };
            }
            Value::Object(_) => {}
            Value::Function(id) => {
                return match level {
                    SemanticIntegrityLevel::Sealed => self.function_is_sealed(*id).map(Some),
                    SemanticIntegrityLevel::Frozen => self.function_is_frozen(*id).map(Some),
                };
            }
            Value::NativeFunction(id) => {
                return match level {
                    SemanticIntegrityLevel::Sealed => self.native_function_is_sealed(*id).map(Some),
                    SemanticIntegrityLevel::Frozen => self.native_function_is_frozen(*id).map(Some),
                };
            }
            Value::HostFunction(id) => {
                return match level {
                    SemanticIntegrityLevel::Sealed => self.host_function_is_sealed(*id).map(Some),
                    SemanticIntegrityLevel::Frozen => self.host_function_is_frozen(*id).map(Some),
                };
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
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

    fn semantic_prototype_chain_contains(
        &mut self,
        prototype: &Value,
        target: &Value,
    ) -> Result<bool> {
        let mut current = prototype.clone();
        loop {
            if crate::runtime::abstract_operations::same_value(&current, target) {
                return Ok(true);
            }
            let Some(object_ref) = self.semantic_object_ref(&current)? else {
                return Ok(false);
            };
            let next = match object_ref.value {
                Value::Object(id) if self.objects.is_proxy(*id) => return Ok(false),
                Value::Object(id) => self.objects.prototype_value(*id)?,
                Value::Function(id) => self.function_inheritance_prototype_value(*id)?,
                Value::NativeFunction(id) => self.native_function_object_prototype_value(*id)?,
                Value::HostFunction(id) => self.host_function_inheritance_prototype_value(*id)?,
                Value::Undefined
                | Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::BigInt(_)
                | Value::String(_)
                | Value::Symbol(_) => return Ok(false),
            };
            if matches!(next, Value::Null) {
                return Ok(false);
            }
            self.step()?;
            current = next;
        }
    }

    fn integrity_property_update(
        descriptor: &OwnPropertyDescriptor,
        level: SemanticIntegrityLevel,
    ) -> PropertyUpdate {
        match descriptor {
            OwnPropertyDescriptor::Data(_) => PropertyUpdate::Data(DataPropertyUpdate::new(
                None,
                matches!(level, SemanticIntegrityLevel::Frozen).then_some(PropertyWritable::No),
                None,
                Some(PropertyConfigurable::No),
            )),
            OwnPropertyDescriptor::Accessor(_) => PropertyUpdate::Accessor(
                AccessorPropertyUpdate::new(None, None, None, Some(PropertyConfigurable::No)),
            ),
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
