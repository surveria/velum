#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    JsBigInt, JsString, RetainedValue, Value,
    api::{embedding::Vm, owned_value::OwnedValue},
    error::{Error, Result},
    runtime::object::{
        AccessorPropertyUpdate, DataPropertyUpdate, OwnPropertyDescriptor, PropertyConfigurable,
        PropertyEnumerable, PropertyUpdate, PropertyWritable,
    },
};

const ARGUMENT_LIST_CAPACITY_ERROR: &str = "embedding argument list capacity exceeded";
const PROPERTY_SYMBOL_TYPE_ERROR: &str = "property symbol handle must retain a Symbol";

/// A borrowed JavaScript input accepted by embedding operations.
///
/// Portable values can enter any VM. A retained value is accepted only by the
/// VM generation that owns it and is resolved before JavaScript dispatch.
#[derive(Clone, Copy, Debug)]
pub enum JsValueRef<'value> {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    BigInt(&'value JsBigInt),
    String(&'value str),
    ExactString(&'value JsString),
    Owned(&'value OwnedValue),
    Retained(&'value RetainedValue),
}

impl From<()> for JsValueRef<'_> {
    fn from((): ()) -> Self {
        Self::Undefined
    }
}

impl From<bool> for JsValueRef<'_> {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<f64> for JsValueRef<'_> {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl<'value> From<&'value str> for JsValueRef<'value> {
    fn from(value: &'value str) -> Self {
        Self::String(value)
    }
}

impl<'value> From<&'value String> for JsValueRef<'value> {
    fn from(value: &'value String) -> Self {
        Self::String(value.as_str())
    }
}

impl<'value> From<&'value JsBigInt> for JsValueRef<'value> {
    fn from(value: &'value JsBigInt) -> Self {
        Self::BigInt(value)
    }
}

impl<'value> From<&'value JsString> for JsValueRef<'value> {
    fn from(value: &'value JsString) -> Self {
        Self::ExactString(value)
    }
}

impl<'value> From<&'value OwnedValue> for JsValueRef<'value> {
    fn from(value: &'value OwnedValue) -> Self {
        Self::Owned(value)
    }
}

impl<'value> From<&'value RetainedValue> for JsValueRef<'value> {
    fn from(value: &'value RetainedValue) -> Self {
        Self::Retained(value)
    }
}

/// A property key accepted by embedding object operations.
#[derive(Clone, Copy, Debug)]
pub enum PropertyKeyRef<'key> {
    Name(&'key str),
    Symbol(&'key RetainedValue),
}

impl<'key> From<&'key str> for PropertyKeyRef<'key> {
    fn from(value: &'key str) -> Self {
        Self::Name(value)
    }
}

/// A partial data-property descriptor for an embedding define operation.
#[derive(Clone, Copy, Debug, Default)]
pub struct DataPropertyDefinition<'value> {
    value: Option<JsValueRef<'value>>,
    writable: Option<bool>,
    enumerable: Option<bool>,
    configurable: Option<bool>,
}

impl<'value> DataPropertyDefinition<'value> {
    #[must_use]
    pub const fn new(value: JsValueRef<'value>) -> Self {
        Self {
            value: Some(value),
            writable: None,
            enumerable: None,
            configurable: None,
        }
    }

    #[must_use]
    pub const fn with_value(mut self, value: JsValueRef<'value>) -> Self {
        self.value = Some(value);
        self
    }

    #[must_use]
    pub const fn with_writable(mut self, writable: bool) -> Self {
        self.writable = Some(writable);
        self
    }

    #[must_use]
    pub const fn with_enumerable(mut self, enumerable: bool) -> Self {
        self.enumerable = Some(enumerable);
        self
    }

    #[must_use]
    pub const fn with_configurable(mut self, configurable: bool) -> Self {
        self.configurable = Some(configurable);
        self
    }
}

/// A partial accessor-property descriptor for an embedding define operation.
#[derive(Clone, Copy, Debug, Default)]
pub struct AccessorPropertyDefinition<'value> {
    getter: Option<JsValueRef<'value>>,
    setter: Option<JsValueRef<'value>>,
    enumerable: Option<bool>,
    configurable: Option<bool>,
}

impl<'value> AccessorPropertyDefinition<'value> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            getter: None,
            setter: None,
            enumerable: None,
            configurable: None,
        }
    }

    #[must_use]
    pub const fn with_getter(mut self, getter: JsValueRef<'value>) -> Self {
        self.getter = Some(getter);
        self
    }

    #[must_use]
    pub const fn with_setter(mut self, setter: JsValueRef<'value>) -> Self {
        self.setter = Some(setter);
        self
    }

    #[must_use]
    pub const fn with_enumerable(mut self, enumerable: bool) -> Self {
        self.enumerable = Some(enumerable);
        self
    }

    #[must_use]
    pub const fn with_configurable(mut self, configurable: bool) -> Self {
        self.configurable = Some(configurable);
        self
    }
}

/// A partial property descriptor supplied by an embedder.
#[derive(Clone, Copy, Debug)]
pub enum PropertyDefinition<'value> {
    Data(DataPropertyDefinition<'value>),
    Accessor(AccessorPropertyDefinition<'value>),
}

impl<'value> From<DataPropertyDefinition<'value>> for PropertyDefinition<'value> {
    fn from(value: DataPropertyDefinition<'value>) -> Self {
        Self::Data(value)
    }
}

impl<'value> From<AccessorPropertyDefinition<'value>> for PropertyDefinition<'value> {
    fn from(value: AccessorPropertyDefinition<'value>) -> Self {
        Self::Accessor(value)
    }
}

/// A retained snapshot of one own property descriptor.
///
/// Every contained handle is an explicit VM root and can be released or
/// dropped independently.
#[derive(Debug)]
pub enum PropertyDescriptor {
    Data {
        value: RetainedValue,
        writable: bool,
        enumerable: bool,
        configurable: bool,
    },
    Accessor {
        getter: Option<RetainedValue>,
        setter: Option<RetainedValue>,
        enumerable: bool,
        configurable: bool,
    },
}

impl Vm {
    /// Returns whether a retained value implements JavaScript `[[Call]]`.
    ///
    /// # Errors
    /// Fails for a foreign or stale retained handle or invalid VM state.
    pub fn is_callable(&self, value: &RetainedValue) -> Result<bool> {
        let context = self.embedding_context_ref();
        let value = context.resolve_retained_value(value)?;
        context.embedding_callable_status(&value)
    }

    /// Returns whether a retained value implements JavaScript `[[Construct]]`.
    ///
    /// # Errors
    /// Fails for a foreign or stale retained handle or invalid VM state.
    pub fn is_constructor(&self, value: &RetainedValue) -> Result<bool> {
        let context = self.embedding_context_ref();
        let value = context.resolve_retained_value(value)?;
        context.embedding_constructor_status(&value)
    }

    /// Calls a retained JavaScript callable with `undefined` as its receiver.
    ///
    /// The raw result is call-local and is not a durable root.
    ///
    /// # Errors
    /// Fails for invalid handles, conversion or resource errors, a non-callable
    /// target, or a JavaScript exception.
    pub fn call(&mut self, callable: &RetainedValue, args: &[JsValueRef<'_>]) -> Result<Value> {
        self.call_with_receiver(callable, JsValueRef::Undefined, args)
    }

    /// Calls a retained JavaScript callable and copies a primitive result.
    ///
    /// # Errors
    /// Fails when [`Self::call`] fails or the result is VM-local.
    pub fn call_owned(
        &mut self,
        callable: &RetainedValue,
        args: &[JsValueRef<'_>],
    ) -> Result<OwnedValue> {
        self.call(callable, args).and_then(OwnedValue::try_from)
    }

    /// Calls a retained JavaScript callable and roots its result.
    ///
    /// # Errors
    /// Fails when [`Self::call`] fails or retained-root allocation fails.
    pub fn call_retained(
        &mut self,
        callable: &RetainedValue,
        args: &[JsValueRef<'_>],
    ) -> Result<RetainedValue> {
        let value = self.call(callable, args)?;
        self.embedding_context_ref().retain_embedder_value(value)
    }

    /// Calls a retained JavaScript callable with an explicit receiver.
    ///
    /// The raw result is call-local and is not a durable root.
    ///
    /// # Errors
    /// Fails for invalid handles, conversion or resource errors, a non-callable
    /// target, or a JavaScript exception.
    pub fn call_with_receiver(
        &mut self,
        callable: &RetainedValue,
        receiver: JsValueRef<'_>,
        args: &[JsValueRef<'_>],
    ) -> Result<Value> {
        let callable = self
            .embedding_context_ref()
            .resolve_retained_value(callable)?;
        let receiver = self.resolve_js_value(receiver)?;
        let args = self.resolve_arguments(args)?;
        self.embedding_context_mut()
            .embedding_call(&callable, &args, receiver)
    }

    /// Calls with an explicit receiver and copies a primitive result.
    ///
    /// # Errors
    /// Fails when [`Self::call_with_receiver`] fails or the result is VM-local.
    pub fn call_with_receiver_owned(
        &mut self,
        callable: &RetainedValue,
        receiver: JsValueRef<'_>,
        args: &[JsValueRef<'_>],
    ) -> Result<OwnedValue> {
        self.call_with_receiver(callable, receiver, args)
            .and_then(OwnedValue::try_from)
    }

    /// Calls with an explicit receiver and roots the result.
    ///
    /// # Errors
    /// Fails when [`Self::call_with_receiver`] fails or root allocation fails.
    pub fn call_with_receiver_retained(
        &mut self,
        callable: &RetainedValue,
        receiver: JsValueRef<'_>,
        args: &[JsValueRef<'_>],
    ) -> Result<RetainedValue> {
        let value = self.call_with_receiver(callable, receiver, args)?;
        self.embedding_context_ref().retain_embedder_value(value)
    }

    /// Looks up and calls a method with the original target as its receiver.
    ///
    /// The raw result is call-local and is not a durable root.
    ///
    /// # Errors
    /// Fails for invalid inputs, property access errors, a non-callable method,
    /// or a JavaScript exception.
    pub fn call_method(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        args: &[JsValueRef<'_>],
    ) -> Result<Value> {
        let target = self.resolve_js_value(target)?;
        let property = self.resolve_property_key(property)?;
        let args = self.resolve_arguments(args)?;
        self.embedding_context_mut()
            .embedding_call_method(&target, &property, &args)
    }

    /// Calls a method and copies a primitive result.
    ///
    /// # Errors
    /// Fails when [`Self::call_method`] fails or the result is VM-local.
    pub fn call_method_owned(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        args: &[JsValueRef<'_>],
    ) -> Result<OwnedValue> {
        self.call_method(target, property, args)
            .and_then(OwnedValue::try_from)
    }

    /// Calls a method and roots its result.
    ///
    /// # Errors
    /// Fails when [`Self::call_method`] fails or root allocation fails.
    pub fn call_method_retained(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        args: &[JsValueRef<'_>],
    ) -> Result<RetainedValue> {
        let value = self.call_method(target, property, args)?;
        self.embedding_context_ref().retain_embedder_value(value)
    }

    /// Constructs an object with the constructor itself as `newTarget`.
    ///
    /// The raw result is call-local and is not a durable root.
    ///
    /// # Errors
    /// Fails for invalid inputs, a non-constructor, resource errors, or a
    /// JavaScript exception.
    pub fn construct(
        &mut self,
        constructor: &RetainedValue,
        args: &[JsValueRef<'_>],
    ) -> Result<Value> {
        let constructor = self
            .embedding_context_ref()
            .resolve_retained_value(constructor)?;
        let args = self.resolve_arguments(args)?;
        self.embedding_context_mut()
            .embedding_construct(&constructor, &args)
    }

    /// Constructs an object and retains the new instance.
    ///
    /// # Errors
    /// Fails when [`Self::construct`] fails or root allocation fails.
    pub fn construct_retained(
        &mut self,
        constructor: &RetainedValue,
        args: &[JsValueRef<'_>],
    ) -> Result<RetainedValue> {
        let value = self.construct(constructor, args)?;
        self.embedding_context_ref().retain_embedder_value(value)
    }

    /// Reads a property through JavaScript `[[Get]]` semantics.
    ///
    /// The raw result is call-local and is not a durable root.
    ///
    /// # Errors
    /// Fails for invalid inputs, property access errors, resource errors, or a
    /// JavaScript exception raised by an accessor or Proxy.
    pub fn get_property(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
    ) -> Result<Value> {
        let target = self.resolve_js_value(target)?;
        let property = self.resolve_property_key(property)?;
        self.embedding_context_mut()
            .embedding_get_property(&target, &property)
    }

    /// Reads a property and copies a primitive result.
    ///
    /// # Errors
    /// Fails when [`Self::get_property`] fails or the result is VM-local.
    pub fn get_property_owned(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
    ) -> Result<OwnedValue> {
        self.get_property(target, property)
            .and_then(OwnedValue::try_from)
    }

    /// Reads a property and roots its result.
    ///
    /// # Errors
    /// Fails when [`Self::get_property`] fails or root allocation fails.
    pub fn get_property_retained(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
    ) -> Result<RetainedValue> {
        let value = self.get_property(target, property)?;
        self.embedding_context_ref().retain_embedder_value(value)
    }

    /// Writes a property with `Reflect.set`-style boolean failure reporting.
    ///
    /// # Errors
    /// Fails for invalid inputs, resource errors, or a JavaScript exception
    /// raised by an accessor or Proxy.
    pub fn set_property(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        value: JsValueRef<'_>,
    ) -> Result<bool> {
        self.set_property_internal(target, property, value, false)
    }

    /// Writes a property and raises JavaScript `TypeError` when `[[Set]]`
    /// reports false, matching strict assignment behavior.
    ///
    /// # Errors
    /// Fails for invalid inputs, failed assignment, resource errors, or a
    /// JavaScript exception raised by an accessor or Proxy.
    pub fn set_property_or_throw(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        value: JsValueRef<'_>,
    ) -> Result<()> {
        self.set_property_internal(target, property, value, true)
            .map(drop)
    }

    /// Defines or updates an own property with boolean failure reporting.
    ///
    /// # Errors
    /// Fails for invalid inputs, invalid descriptors, resource errors, or a
    /// JavaScript exception raised by a Proxy.
    pub fn define_property(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        definition: PropertyDefinition<'_>,
    ) -> Result<bool> {
        self.define_property_internal(target, property, definition, false)
    }

    /// Defines an own property and raises JavaScript `TypeError` on rejection.
    ///
    /// # Errors
    /// Fails for invalid inputs, an invalid or rejected descriptor, resource
    /// errors, or a JavaScript exception raised by a Proxy.
    pub fn define_property_or_throw(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        definition: PropertyDefinition<'_>,
    ) -> Result<()> {
        self.define_property_internal(target, property, definition, true)
            .map(drop)
    }

    /// Deletes a property with `Reflect.deleteProperty`-style reporting.
    ///
    /// # Errors
    /// Fails for invalid inputs, resource errors, or a JavaScript exception
    /// raised by a Proxy.
    pub fn delete_property(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
    ) -> Result<bool> {
        self.delete_property_internal(target, property, false)
    }

    /// Deletes a property and raises JavaScript `TypeError` when rejected.
    ///
    /// # Errors
    /// Fails for invalid inputs, rejected deletion, resource errors, or a
    /// JavaScript exception raised by a Proxy.
    pub fn delete_property_or_throw(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
    ) -> Result<()> {
        self.delete_property_internal(target, property, true)
            .map(drop)
    }

    /// Returns a retained own-property descriptor snapshot.
    ///
    /// # Errors
    /// Fails for invalid inputs, resource errors, root allocation failures, or
    /// a JavaScript exception raised by a Proxy.
    pub fn get_own_property_descriptor(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
    ) -> Result<Option<PropertyDescriptor>> {
        let target = self.resolve_js_value(target)?;
        let property = self.resolve_property_key(property)?;
        let descriptor = self
            .embedding_context_mut()
            .embedding_own_property_descriptor(&target, &property)?;
        descriptor
            .map(|descriptor| self.retain_property_descriptor(descriptor))
            .transpose()
    }

    fn set_property_internal(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        value: JsValueRef<'_>,
        throw_on_failure: bool,
    ) -> Result<bool> {
        let target = self.resolve_js_value(target)?;
        let property = self.resolve_property_key(property)?;
        let value = self.resolve_js_value(value)?;
        self.embedding_context_mut().embedding_set_property(
            &target,
            &property,
            value,
            throw_on_failure,
        )
    }

    fn define_property_internal(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        definition: PropertyDefinition<'_>,
        throw_on_failure: bool,
    ) -> Result<bool> {
        let target = self.resolve_js_value(target)?;
        let property = self.resolve_property_key(property)?;
        let update = self.resolve_property_definition(definition)?;
        self.embedding_context_mut().embedding_define_property(
            &target,
            &property,
            update,
            throw_on_failure,
        )
    }

    fn delete_property_internal(
        &mut self,
        target: JsValueRef<'_>,
        property: PropertyKeyRef<'_>,
        throw_on_failure: bool,
    ) -> Result<bool> {
        let target = self.resolve_js_value(target)?;
        let property = self.resolve_property_key(property)?;
        self.embedding_context_mut()
            .embedding_delete_property(&target, &property, throw_on_failure)
    }

    fn resolve_arguments(&mut self, args: &[JsValueRef<'_>]) -> Result<Vec<Value>> {
        let mut values = Vec::new();
        values
            .try_reserve(args.len())
            .map_err(|_| Error::limit(ARGUMENT_LIST_CAPACITY_ERROR))?;
        for argument in args {
            values.push(self.resolve_js_value(*argument)?);
        }
        Ok(values)
    }

    fn resolve_js_value(&mut self, value: JsValueRef<'_>) -> Result<Value> {
        match value {
            JsValueRef::Undefined => Ok(Value::Undefined),
            JsValueRef::Null => Ok(Value::Null),
            JsValueRef::Bool(value) => Ok(Value::Bool(value)),
            JsValueRef::Number(value) => Ok(Value::Number(value)),
            JsValueRef::BigInt(value) => self
                .embedding_context_mut()
                .runtime_value(Value::BigInt(value.clone())),
            JsValueRef::String(value) => self.embedding_context_mut().heap_string_value(value),
            JsValueRef::ExactString(value) => {
                self.embedding_context_mut().heap_js_string_value(value)
            }
            JsValueRef::Owned(value) => self
                .embedding_context_mut()
                .runtime_value(value.clone().into()),
            JsValueRef::Retained(value) => {
                self.embedding_context_ref().resolve_retained_value(value)
            }
        }
    }

    fn resolve_property_key(&mut self, property: PropertyKeyRef<'_>) -> Result<Value> {
        match property {
            PropertyKeyRef::Name(name) => self.embedding_context_mut().heap_string_value(name),
            PropertyKeyRef::Symbol(handle) => {
                let value = self
                    .embedding_context_ref()
                    .resolve_retained_value(handle)?;
                if matches!(value, Value::Symbol(_)) {
                    return Ok(value);
                }
                Err(Error::runtime(PROPERTY_SYMBOL_TYPE_ERROR))
            }
        }
    }

    fn resolve_property_definition(
        &mut self,
        definition: PropertyDefinition<'_>,
    ) -> Result<PropertyUpdate> {
        match definition {
            PropertyDefinition::Data(definition) => {
                Ok(PropertyUpdate::Data(DataPropertyUpdate::new(
                    self.resolve_optional_value(definition.value)?,
                    definition.writable.map(property_writable),
                    definition.enumerable.map(property_enumerable),
                    definition.configurable.map(property_configurable),
                )))
            }
            PropertyDefinition::Accessor(definition) => {
                Ok(PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    self.resolve_optional_value(definition.getter)?,
                    self.resolve_optional_value(definition.setter)?,
                    definition.enumerable.map(property_enumerable),
                    definition.configurable.map(property_configurable),
                )))
            }
        }
    }

    fn resolve_optional_value(&mut self, value: Option<JsValueRef<'_>>) -> Result<Option<Value>> {
        value.map(|value| self.resolve_js_value(value)).transpose()
    }

    fn retain_property_descriptor(
        &self,
        descriptor: OwnPropertyDescriptor,
    ) -> Result<PropertyDescriptor> {
        match descriptor {
            OwnPropertyDescriptor::Data(descriptor) => Ok(PropertyDescriptor::Data {
                value: self
                    .embedding_context_ref()
                    .retain_embedder_value(descriptor.value())?,
                writable: descriptor.writable().is_yes(),
                enumerable: descriptor.enumerable().is_yes(),
                configurable: descriptor.configurable().is_yes(),
            }),
            OwnPropertyDescriptor::Accessor(descriptor) => Ok(PropertyDescriptor::Accessor {
                getter: self.retain_optional_accessor(descriptor.get())?,
                setter: self.retain_optional_accessor(descriptor.set())?,
                enumerable: descriptor.enumerable().is_yes(),
                configurable: descriptor.configurable().is_yes(),
            }),
        }
    }

    fn retain_optional_accessor(&self, value: Value) -> Result<Option<RetainedValue>> {
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        self.embedding_context_ref()
            .retain_embedder_value(value)
            .map(Some)
    }
}

const fn property_writable(value: bool) -> PropertyWritable {
    if value {
        PropertyWritable::Yes
    } else {
        PropertyWritable::No
    }
}

const fn property_enumerable(value: bool) -> PropertyEnumerable {
    if value {
        PropertyEnumerable::Yes
    } else {
        PropertyEnumerable::No
    }
}

const fn property_configurable(value: bool) -> PropertyConfigurable {
    if value {
        PropertyConfigurable::Yes
    } else {
        PropertyConfigurable::No
    }
}
