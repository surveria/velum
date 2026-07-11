use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            DataPropertyDescriptor, OwnPropertyDescriptor, PropertyConfigurable,
            PropertyEnumerable, PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
    },
    value::Value,
};

impl Context {
    /// Applies `ToPropertyDescriptor` and dispatches object-like
    /// `[[DefineOwnProperty]]` through one owner.
    pub(in crate::runtime) fn semantic_define_own_property_from_value(
        &mut self,
        target: &Value,
        property: &mut DynamicPropertyKey,
        descriptor_value: &Value,
    ) -> Result<bool> {
        let update = self.property_update_from_value(descriptor_value)?;
        self.semantic_define_own_property_update_with_descriptor(
            target,
            property,
            update,
            descriptor_value,
        )
    }

    pub(in crate::runtime) fn semantic_define_own_property_update_with_descriptor(
        &mut self,
        target: &Value,
        property: &mut DynamicPropertyKey,
        update: PropertyUpdate,
        descriptor_value: &Value,
    ) -> Result<bool> {
        if let Value::Object(id) = target
            && self.objects.is_proxy(*id)
        {
            return self.proxy_define_property(
                *id,
                property.lookup(),
                update,
                descriptor_value.clone(),
            );
        }
        self.semantic_define_own_property_update(target, property, update)
    }

    /// Dispatches an already parsed property descriptor to the physical owner.
    pub(in crate::runtime) fn semantic_define_own_property_update(
        &mut self,
        target: &Value,
        property: &mut DynamicPropertyKey,
        update: PropertyUpdate,
    ) -> Result<bool> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return Err(Error::runtime(
                "property definition target must be an object",
            ));
        };
        let key = self.intern_dynamic_property_key(property)?;
        match object_ref.value {
            Value::Object(id) => {
                self.objects.define_property(
                    *id,
                    key,
                    property.name(),
                    update,
                    self.limits.max_object_properties,
                )?;
            }
            Value::Function(id) => {
                self.define_function_property_key(*id, property.name(), key, update)?;
            }
            Value::NativeFunction(id) => {
                let PropertyUpdate::Data(update) = update else {
                    return Err(Error::runtime(
                        "accessor properties are not supported on native function objects",
                    ));
                };
                self.define_native_function_property_key(*id, property.name(), key, update)?;
            }
            Value::HostFunction(_) => {
                return Err(Error::runtime(
                    "property definition target is not supported",
                ));
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => {
                return Err(Error::runtime(
                    "property definition target must be an object",
                ));
            }
        }
        Ok(true)
    }

    /// Shared object-like `[[GetOwnProperty]]` dispatch.
    pub(in crate::runtime) fn semantic_own_property_descriptor(
        &mut self,
        target: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        let Some(object_ref) = self.semantic_object_ref(target)? else {
            return match target {
                Value::String(_) | Value::HeapString(_) => {
                    self.primitive_own_property_descriptor(target, property)
                }
                Value::Undefined | Value::Null => Err(Error::runtime(
                    "property descriptor target cannot be converted to an object",
                )),
                Value::Bool(_)
                | Value::Number(_)
                | Value::Symbol(_)
                | Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_) => Ok(None),
            };
        };
        match object_ref.value {
            Value::Object(id) if self.objects.is_proxy(*id) => {
                self.proxy_get_own_property_descriptor(*id, property.lookup())
            }
            Value::Object(id) => {
                if let Some(descriptor) =
                    self.string_object_own_property_descriptor(*id, property)?
                {
                    return Ok(Some(OwnPropertyDescriptor::Data(descriptor)));
                }
                if let Some(descriptor) = self
                    .objects
                    .own_property_descriptor(*id, property.lookup())?
                {
                    return Ok(Some(descriptor));
                }
                self.global_object_property_descriptor(*id, property.lookup())
            }
            Value::Function(id) => {
                self.function_own_property_descriptor_lookup(*id, property.lookup())
            }
            Value::NativeFunction(id) => {
                self.native_function_own_property_descriptor_lookup(*id, property.lookup())
            }
            Value::HostFunction(_) => Err(Error::runtime(
                "property descriptor target cannot be converted to an object",
            )),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => Ok(None),
        }
    }

    fn primitive_own_property_descriptor(
        &mut self,
        target: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        if !self.has_own_property_value(target, property)? {
            return Ok(None);
        }
        Ok(Some(OwnPropertyDescriptor::Data(
            DataPropertyDescriptor::new(
                self.get_named(target, property.name())?,
                PropertyWritable::Yes,
                PropertyEnumerable::Yes,
                PropertyConfigurable::Yes,
            ),
        )))
    }
}
