use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::control::Completion,
    runtime::object::{AccessorWriteDisposition, PropertyKey, PropertyLookup},
    value::{ObjectId, Value},
};

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
        crate::runtime::property::set_property(
            &mut self.objects,
            &Value::Object(object),
            key,
            property_name,
            value,
            self.limits.max_object_properties,
        )
    }

    /// Calls an accessor function and rethrows JS `Error` throw completions
    /// as engine exceptions so surrounding `try`/`catch` blocks can observe
    /// them; other abrupt completions surface as runtime errors.
    pub(in crate::runtime) fn call_accessor_function(
        &mut self,
        function: &Value,
        this_value: Value,
        args: &[Value],
    ) -> Result<Value> {
        let completion = self.eval_call_completion(function, args, this_value)?;
        match completion {
            Completion::Throw(Value::Error(error)) => {
                Err(Error::exception(error.name(), error.message().to_owned()))
            }
            completion => completion.into_result(),
        }
    }
}
