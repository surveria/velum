use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::numeric::number_to_uint32,
    runtime::{
        object::{PropertyKey, PropertyLookup},
        property::DynamicPropertyKey,
    },
    value::{ErrorName, ObjectId, Value},
};

const ARRAY_LENGTH_RANGE_ERROR: &str = "Invalid array length";

impl Context {
    /// Invokes a getter function with the property read receiver as `this`.
    pub(in crate::runtime) fn call_accessor_getter(
        &mut self,
        getter: &Value,
        receiver: Value,
    ) -> Result<Value> {
        self.call_accessor_function(getter, receiver, &[])
    }

    /// Assigns `value` to `property` on `object`, routing the write through a
    /// setter when the receiver or its prototype chain defines an accessor
    /// property with that name. A getter-only accessor swallows the write
    /// (sloppy-mode semantics); otherwise ordinary data-write rules apply.
    pub(in crate::runtime) fn set_property_value_with_accessors(
        &mut self,
        object: &Value,
        key: PropertyKey,
        property_name: &str,
        value: Value,
    ) -> Result<()> {
        let lookup = PropertyLookup::from_key(property_name, key);
        let Some(write) = self.semantic_property_write(object, lookup, value.clone())? else {
            crate::runtime::property::set_property(
                &mut self.objects,
                object,
                key,
                property_name,
                value,
                self.limits.max_object_properties,
            )?;
            return Ok(());
        };
        self.finish_semantic_property_write(write, lookup, value)?;
        Ok(())
    }

    pub(in crate::runtime) fn write_ordinary_object_property_with_accessors(
        &mut self,
        object: ObjectId,
        key: PropertyKey,
        property_name: &str,
        value: Value,
    ) -> Result<()> {
        let receiver = Value::Object(object);
        let mut property = DynamicPropertyKey::new(property_name.to_owned(), Some(key));
        let Some(_updated) =
            self.semantic_reflect_property_write(&receiver, &mut property, value, &receiver)?
        else {
            return Err(Error::runtime("ordinary object write target disappeared"));
        };
        Ok(())
    }

    pub(in crate::runtime) fn array_length_from_value(&mut self, value: &Value) -> Result<usize> {
        let uint32_length = number_to_uint32(self.to_number(value)?, "array length")?;
        let number_length = self.to_number(value)?;
        if !matches!(
            number_length.partial_cmp(&f64::from(uint32_length)),
            Some(core::cmp::Ordering::Equal)
        ) {
            return Err(Error::exception(
                ErrorName::RangeError,
                ARRAY_LENGTH_RANGE_ERROR,
            ));
        }
        usize::try_from(uint32_length)
            .map_err(|_| Error::exception(ErrorName::RangeError, ARRAY_LENGTH_RANGE_ERROR))
    }

    /// Calls an accessor function while preserving any JavaScript thrown value
    /// across the native property boundary.
    pub(in crate::runtime) fn call_accessor_function(
        &mut self,
        function: &Value,
        this_value: Value,
        args: &[Value],
    ) -> Result<Value> {
        self.call(function, args, this_value)?.into_result()
    }
}
