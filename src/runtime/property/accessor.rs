use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::object::{AccessorWriteDisposition, PropertyKey, PropertyLookup},
    value::{ErrorName, ObjectId, Value},
};

const ARRAY_LENGTH_PROPERTY: &str = "length";
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
        let lookup = PropertyLookup::from_key(property_name, key);
        match self.objects.accessor_write_target(object, lookup)? {
            AccessorWriteDisposition::Setter(setter) => {
                self.call_accessor_function(&setter, Value::Object(object), &[value])?;
                return Ok(());
            }
            AccessorWriteDisposition::NoSetter => return Ok(()),
            AccessorWriteDisposition::None => {}
        }
        if let Some(index) = self
            .objects
            .typed_array_property_index(object, property_name)?
        {
            let crate::runtime::object::TypedArrayPropertyIndex::Valid(index) = index else {
                return Ok(());
            };
            let Some(view) = self.objects.typed_array(object)? else {
                return Err(Error::runtime("typed array view is not available"));
            };
            let element = self.convert_typed_array_element_value(view.element_kind(), &value)?;
            self.objects
                .set_typed_array_value(object, index, &element)?;
            return Ok(());
        }
        if property_name == ARRAY_LENGTH_PROPERTY
            && self.objects.array_len_if_array(object)?.is_some()
        {
            let length = self.array_length_from_value(&value)?;
            return self.objects.set_array_length(object, length).map(|_| ());
        }
        crate::runtime::property::set_property(
            &mut self.objects,
            &Value::Object(object),
            key,
            property_name,
            value,
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime) fn array_length_from_value(&mut self, value: &Value) -> Result<usize> {
        let number = self.to_number(value)?;
        if !number.is_finite()
            || number < 0.0
            || number.fract() != 0.0
            || number > f64::from(u32::MAX)
        {
            return Err(Error::exception(
                ErrorName::RangeError,
                ARRAY_LENGTH_RANGE_ERROR,
            ));
        }
        if number == 0.0 {
            return Ok(0);
        }
        let length = format!("{number:.0}")
            .parse::<u32>()
            .map_err(|_| Error::exception(ErrorName::RangeError, ARRAY_LENGTH_RANGE_ERROR))?;
        usize::try_from(length)
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
