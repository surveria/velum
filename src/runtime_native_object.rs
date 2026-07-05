use crate::{
    ast::Expr,
    error::{Error, Result},
    runtime::Context,
    runtime_object::{
        DataPropertyDescriptor, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
        PropertyWritable,
    },
    runtime_property::{has_property, property_key},
    value::{NativeFunctionId, Value},
};

use super::{
    NativeFunction, NativeFunctionKind, OBJECT_DEFINE_PROPERTY_NAME,
    OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME, OBJECT_HAS_OWN_NAME, OBJECT_KEYS_NAME, OBJECT_NAME,
};

const DESCRIPTOR_CONFIGURABLE_PROPERTY: &str = "configurable";
const DESCRIPTOR_ENUMERABLE_PROPERTY: &str = "enumerable";
const DESCRIPTOR_GET_PROPERTY: &str = "get";
const DESCRIPTOR_SET_PROPERTY: &str = "set";
const DESCRIPTOR_VALUE_PROPERTY: &str = "value";
const DESCRIPTOR_WRITABLE_PROPERTY: &str = "writable";

impl Context {
    pub(super) fn object_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Object) {
            return Ok(Value::NativeFunction(id));
        }

        let id = NativeFunctionId::new(self.native_functions.len());
        let constructor = Value::NativeFunction(id);
        let prototype = self.object_prototype_id_with_constructor(constructor.clone())?;
        self.native_functions
            .push(NativeFunction::new(NativeFunctionKind::Object, prototype));
        self.install_object_static_methods(id)?;
        self.insert_global_builtin(OBJECT_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(super) fn eval_object_constructor(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_native_args(args)?;
        let Some(value) = values.first() else {
            return self.create_object_from_constructor();
        };

        match value {
            Value::Object(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => Ok(value.clone()),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_) => self.create_object_from_constructor(),
        }
    }

    pub(super) fn eval_object_define_property(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_native_args(args)?;
        let target = Self::argument_or_undefined(&values, 0);
        let property = self.object_property_key(&values, 1)?;
        let descriptor_value = Self::argument_or_undefined(&values, 2);
        let descriptor = self.data_property_update_from_value(&descriptor_value)?;
        match &target {
            Value::Object(id) => {
                let key = self.intern_property_key(&property)?;
                self.objects.define_property(
                    *id,
                    key,
                    &property,
                    descriptor,
                    self.limits.max_object_properties,
                )?;
            }
            Value::Function(id) => self.define_function_property(*id, property, descriptor)?,
            Value::NativeFunction(id) => {
                self.define_native_function_property(*id, property, descriptor)?;
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HostFunction(_)
            | Value::Error(_) => {
                return Err(Error::runtime(
                    "Object.defineProperty target must be an object",
                ));
            }
        }
        Ok(target)
    }

    pub(super) fn eval_object_get_own_property_descriptor(
        &mut self,
        args: &[Expr],
    ) -> Result<Value> {
        let values = self.eval_native_args(args)?;
        let target = Self::argument_or_undefined(&values, 0);
        let property = self.object_property_key(&values, 1)?;
        let descriptor = match target {
            Value::Object(id) => self
                .objects
                .own_property_descriptor(id, self.property_lookup(&property))?,
            Value::Function(id) => self.function_own_property_descriptor(id, &property)?,
            Value::NativeFunction(id) => {
                self.native_function_own_property_descriptor(id, &property)?
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HostFunction(_)
            | Value::Error(_) => {
                return Err(Error::runtime(
                    "Object.getOwnPropertyDescriptor target must be an object",
                ));
            }
        };
        let Some(descriptor) = descriptor else {
            return Ok(Value::Undefined);
        };
        self.create_property_descriptor_object(&descriptor)
    }

    pub(super) fn eval_object_has_own(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_native_args(args)?;
        let target = Self::argument_or_undefined(&values, 0);
        let property = self.object_property_key(&values, 1)?;
        self.has_own_property_value(&target, &property)
            .map(Value::Bool)
    }

    pub(super) fn eval_object_keys(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_native_args(args)?;
        let target = Self::argument_or_undefined(&values, 0);
        let keys = self.own_enumerable_keys(&target)?;
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        let elements = keys.into_iter().map(Value::String).collect();
        self.objects.create_array(
            elements,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn install_object_static_methods(&mut self, constructor: NativeFunctionId) -> Result<()> {
        self.define_object_static_method(
            constructor,
            OBJECT_DEFINE_PROPERTY_NAME,
            NativeFunctionKind::ObjectDefineProperty,
        )?;
        self.define_object_static_method(
            constructor,
            OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME,
            NativeFunctionKind::ObjectGetOwnPropertyDescriptor,
        )?;
        self.define_object_static_method(
            constructor,
            OBJECT_HAS_OWN_NAME,
            NativeFunctionKind::ObjectHasOwn,
        )?;
        self.define_object_static_method(
            constructor,
            OBJECT_KEYS_NAME,
            NativeFunctionKind::ObjectKeys,
        )
    }

    fn define_object_static_method(
        &mut self,
        constructor: NativeFunctionId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined);
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(name.to_owned(), function, PropertyEnumerable::No);
        Ok(())
    }

    fn eval_native_args(&mut self, args: &[Expr]) -> Result<Vec<Value>> {
        args.iter().map(|arg| self.eval_expr(arg)).collect()
    }

    fn argument_or_undefined(values: &[Value], index: usize) -> Value {
        values.get(index).cloned().unwrap_or(Value::Undefined)
    }

    fn object_property_key(&self, values: &[Value], index: usize) -> Result<String> {
        let key = property_key(&Self::argument_or_undefined(values, index));
        self.check_string_len(&key)?;
        Ok(key)
    }

    fn data_property_update_from_value(&self, value: &Value) -> Result<DataPropertyUpdate> {
        if !matches!(value, Value::Object(_)) {
            return Err(Error::runtime("property descriptor must be an object"));
        }
        self.reject_accessor_descriptor(value)?;
        Ok(DataPropertyUpdate::new(
            self.optional_descriptor_value(value, DESCRIPTOR_VALUE_PROPERTY)?,
            self.optional_descriptor_writable(value)?,
            self.optional_descriptor_enumerable(value)?,
            self.optional_descriptor_configurable(value)?,
        ))
    }

    fn reject_accessor_descriptor(&self, descriptor: &Value) -> Result<()> {
        if !matches!(
            self.get_property_value(descriptor, DESCRIPTOR_GET_PROPERTY)?,
            Value::Undefined
        ) {
            return Err(Error::runtime("accessor descriptors are not supported yet"));
        }
        if !matches!(
            self.get_property_value(descriptor, DESCRIPTOR_SET_PROPERTY)?,
            Value::Undefined
        ) {
            return Err(Error::runtime("accessor descriptors are not supported yet"));
        }
        Ok(())
    }

    fn optional_descriptor_value(
        &self,
        descriptor: &Value,
        property: &str,
    ) -> Result<Option<Value>> {
        if !has_property(&self.objects, descriptor, self.property_lookup(property))? {
            return Ok(None);
        }
        self.get_property_value(descriptor, property).map(Some)
    }

    fn optional_descriptor_writable(&self, descriptor: &Value) -> Result<Option<PropertyWritable>> {
        self.optional_descriptor_bool(descriptor, DESCRIPTOR_WRITABLE_PROPERTY)
            .map(|value| value.map(Self::property_writable))
    }

    fn optional_descriptor_enumerable(
        &self,
        descriptor: &Value,
    ) -> Result<Option<PropertyEnumerable>> {
        self.optional_descriptor_bool(descriptor, DESCRIPTOR_ENUMERABLE_PROPERTY)
            .map(|value| value.map(Self::property_enumerable))
    }

    fn optional_descriptor_configurable(
        &self,
        descriptor: &Value,
    ) -> Result<Option<PropertyConfigurable>> {
        self.optional_descriptor_bool(descriptor, DESCRIPTOR_CONFIGURABLE_PROPERTY)
            .map(|value| value.map(Self::property_configurable))
    }

    fn optional_descriptor_bool(&self, descriptor: &Value, property: &str) -> Result<Option<bool>> {
        if !has_property(&self.objects, descriptor, self.property_lookup(property))? {
            return Ok(None);
        }
        Ok(Some(
            self.get_property_value(descriptor, property)?.is_truthy(),
        ))
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

    fn create_property_descriptor_object(
        &mut self,
        descriptor: &DataPropertyDescriptor,
    ) -> Result<Value> {
        let properties = vec![
            self.descriptor_object_property(DESCRIPTOR_VALUE_PROPERTY, descriptor.value())?,
            self.descriptor_object_property(
                DESCRIPTOR_WRITABLE_PROPERTY,
                Value::Bool(descriptor.writable().is_yes()),
            )?,
            self.descriptor_object_property(
                DESCRIPTOR_ENUMERABLE_PROPERTY,
                Value::Bool(descriptor.enumerable().is_yes()),
            )?,
            self.descriptor_object_property(
                DESCRIPTOR_CONFIGURABLE_PROPERTY,
                Value::Bool(descriptor.configurable().is_yes()),
            )?,
        ];
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            properties,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn descriptor_object_property(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<(crate::runtime_object::PropertyKey, String, Value)> {
        let key = self.intern_property_key(name)?;
        Ok((key, name.to_owned(), value))
    }

    fn has_own_property_value(&self, target: &Value, property: &str) -> Result<bool> {
        match target {
            Value::Object(id) => self.objects.has_own(*id, self.property_lookup(property)),
            Value::Function(id) => self.has_function_property(*id, property),
            Value::NativeFunction(id) => self.has_native_function_property(*id, property),
            Value::Error(_) | Value::String(_) => {
                has_property(&self.objects, target, self.property_lookup(property))
            }
            Value::Bool(_) | Value::Number(_) => Ok(false),
            Value::Undefined | Value::Null | Value::HostFunction(_) => Err(Error::runtime(
                "Object.hasOwn target cannot be converted to an object",
            )),
        }
    }

    fn own_enumerable_keys(&self, target: &Value) -> Result<Vec<String>> {
        match target {
            Value::Object(id) => self.objects.own_keys(*id, &self.atoms),
            Value::Function(id) => self.function_enumerable_keys(*id),
            Value::NativeFunction(id) => self.native_function_enumerable_keys(*id),
            Value::Error(_) | Value::String(_) => self.enumerable_keys(target),
            Value::Bool(_) | Value::Number(_) => Ok(Vec::new()),
            Value::Undefined | Value::Null | Value::HostFunction(_) => Err(Error::runtime(
                "Object.keys target cannot be converted to an object",
            )),
        }
    }

    fn create_object_from_constructor(&mut self) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}
