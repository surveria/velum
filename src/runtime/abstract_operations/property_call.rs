use crate::{
    error::{Error, Result},
    runtime::{Context, control::Completion, object::PropertyLookup, property::get_property},
    value::Value,
};

/// Selects the specification `Set` failure behavior without hiding a failed
/// `[[Set]]` result behind engine storage errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum SetFailureBehavior {
    ReturnFalse,
    Throw,
}

impl Context {
    /// ECMAScript `Get(O, P)` over the semantic `[[Get]]` boundary.
    ///
    /// Primitive member behavior remains here until the value representation
    /// can model temporary wrapper objects without splitting observable reads.
    pub(in crate::runtime) fn get(
        &mut self,
        object: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        if let Some(read) = self.semantic_property_read(object, property)? {
            return self.finish_semantic_property_read(read, object, property);
        }
        if let Value::String(value) = object {
            if property.key().is_some_and(|key| key.symbol_id().is_some()) {
                return self.get_string_prototype_symbol_property(object, property);
            }
            return self.get_string_property_value(object, value, property.name());
        }
        if let Value::HeapString(value) = object {
            if property.key().is_some_and(|key| key.symbol_id().is_some()) {
                return self.get_string_prototype_symbol_property(object, property);
            }
            return self.get_string_property_value(object, value.as_str(), property.name());
        }
        if let Some(value) = self.primitive_prototype_property_value(object, property.name())? {
            return Ok(value);
        }
        let value = get_property(&self.objects, object, property)?;
        self.runtime_property_value(value)
    }

    fn get_string_prototype_symbol_property(
        &mut self,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let prototype = Value::Object(self.string_constructor_prototype()?);
        let Some(read) =
            self.semantic_property_read_with_receiver(&prototype, receiver, property)?
        else {
            return Err(Error::runtime("String prototype is not an object"));
        };
        self.finish_semantic_property_read(read, receiver, property)
    }

    /// Named-key convenience entrypoint for the shared `Get` operation.
    pub(in crate::runtime) fn get_named(
        &mut self,
        object: &Value,
        property: &str,
    ) -> Result<Value> {
        let lookup = self.property_lookup(property);
        self.get(object, lookup)
    }

    /// ECMAScript `Set(O, P, V, Throw)` over the receiver-aware `[[Set]]`
    /// boundary.
    pub(in crate::runtime) fn set(
        &mut self,
        object: &Value,
        property: PropertyLookup<'_>,
        value: Value,
        receiver: &Value,
        failure: SetFailureBehavior,
    ) -> Result<bool> {
        let mut dynamic = crate::runtime::property::DynamicPropertyKey::new(
            property.name().to_owned(),
            property.key(),
        );
        let updated = self
            .semantic_reflect_property_write(object, &mut dynamic, value, receiver)?
            .ok_or_else(|| Error::type_error("Set target is not an object"))?;
        if !updated && matches!(failure, SetFailureBehavior::Throw) {
            return Err(Error::type_error(format!(
                "Cannot assign to property '{}'",
                property.name()
            )));
        }
        Ok(updated)
    }

    /// ECMAScript `Call(F, V, argumentsList)`, preserving abrupt completion.
    pub(in crate::runtime) fn call(
        &mut self,
        function: &Value,
        arguments: &[Value],
        this_value: Value,
    ) -> Result<Completion> {
        self.semantic_call(function, arguments, this_value)
    }

    /// Converts the shared `Call` completion at native-value boundaries.
    pub(in crate::runtime) fn call_value(
        &mut self,
        function: &Value,
        arguments: &[Value],
        this_value: Value,
    ) -> Result<Value> {
        self.call(function, arguments, this_value)?
            .into_native_value_result()
    }

    /// ECMAScript `GetMethod(V, P)`: nullish properties are absent, callable
    /// properties are returned, and every other value raises `TypeError`.
    pub(in crate::runtime) fn get_method(
        &mut self,
        value: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Option<Value>> {
        let method = self.get(value, property)?;
        if matches!(method, Value::Undefined | Value::Null) {
            return Ok(None);
        }
        if !self.semantic_is_callable(&method)? {
            return Err(Error::type_error(format!(
                "Property '{}' is not callable",
                property.name()
            )));
        }
        Ok(Some(method))
    }

    /// Named-key convenience entrypoint for the shared `GetMethod` operation.
    pub(in crate::runtime) fn get_named_method(
        &mut self,
        value: &Value,
        property: &str,
    ) -> Result<Option<Value>> {
        let lookup = self.property_lookup(property);
        self.get_method(value, lookup)
    }
}
