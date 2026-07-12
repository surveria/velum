use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            DataPropertyDescriptor, OwnPropertyDescriptor, PropertyConfigurable,
            PropertyEnumerable, PropertyUpdate, PropertyWritable, TypedArrayPropertyIndex,
        },
        property::{DynamicPropertyKey, has_property},
    },
    value::Value,
};

const ARRAY_LENGTH_PROPERTY: &str = "length";

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
        if let Value::Object(id) = object_ref.value
            && let Some(index) = self
                .objects
                .typed_array_property_index(*id, property.name())?
        {
            return self.semantic_define_typed_array_index(*id, index, update);
        }
        if let Value::Object(id) = object_ref.value
            && property.name() == ARRAY_LENGTH_PROPERTY
            && self.objects.array_len_if_array(*id)?.is_some()
        {
            let PropertyUpdate::Data(mut update) = update else {
                return Err(Error::type_error(
                    "array length cannot be defined as an accessor property",
                ));
            };
            let new_length = if let Some(value) = update.value() {
                let length = self.array_length_from_value(&value)?;
                let normalized = u32::try_from(length)
                    .map_err(|_| Error::limit("array length exceeded supported range"))?;
                update.replace_value(Value::Number(f64::from(normalized)));
                Some(length)
            } else {
                None
            };
            self.objects
                .define_array_length_property(*id, update, new_length)?;
            return Ok(true);
        }
        if let Value::Object(id) = object_ref.value
            && self.is_global_object_id(*id)
            && self
                .objects
                .own_property_descriptor(*id, property.lookup())?
                .is_none()
        {
            let _materialized_descriptor =
                self.global_object_property_descriptor(*id, property.lookup())?;
        }
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
                if self.is_global_object_id(*id)
                    && property.key().is_none_or(|key| key.symbol_id().is_none())
                {
                    self.mark_global_object_property_authoritative(*id, property.name())?;
                }
            }
            Value::Function(id) => {
                self.define_function_property_key(*id, property.name(), key, update)?;
            }
            Value::NativeFunction(id) => match update {
                PropertyUpdate::Data(update) => {
                    self.define_native_function_property_key(*id, property.name(), key, update)?;
                }
                PropertyUpdate::Accessor(update) => {
                    self.define_native_function_accessor_property_key(
                        *id,
                        property.name(),
                        key,
                        update,
                    )?;
                }
            },
            Value::HostFunction(_) => {
                return Err(Error::runtime(
                    "property definition target is not supported",
                ));
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
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

    fn semantic_define_typed_array_index(
        &mut self,
        id: crate::value::ObjectId,
        index: TypedArrayPropertyIndex,
        update: PropertyUpdate,
    ) -> Result<bool> {
        let TypedArrayPropertyIndex::Valid(index) = index else {
            return Ok(false);
        };
        let PropertyUpdate::Data(update) = update else {
            return Ok(false);
        };
        if update.configurable().is_some_and(|value| !value.is_yes())
            || update.enumerable().is_some_and(|value| !value.is_yes())
            || update.writable().is_some_and(|value| !value.is_yes())
        {
            return Ok(false);
        }
        if let Some(value) = update.value() {
            let Some(view) = self.objects.typed_array(id)? else {
                return Err(Error::runtime("typed array view is not available"));
            };
            let element = self.convert_typed_array_element_value(view.element_kind(), &value)?;
            if !self.objects.set_typed_array_value(id, index, &element)? {
                return Ok(false);
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
                | Value::BigInt(_)
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
            | Value::BigInt(_)
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
        if !has_property(&self.objects, target, property.lookup())? {
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
