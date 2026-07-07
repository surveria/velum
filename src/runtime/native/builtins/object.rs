use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call_args::RuntimeCallArgs,
    runtime::object::{
        DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyKey, PropertyWritable,
    },
    runtime::property::{DynamicPropertyKey, has_property},
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{
    NativeFunctionKind, OBJECT_DEFINE_PROPERTY_NAME, OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME,
    OBJECT_HAS_OWN_NAME, OBJECT_KEYS_NAME, OBJECT_NAME,
};
use crate::runtime::property::well_known::DescriptorPropertyKeys;

const DESCRIPTOR_CONFIGURABLE_PROPERTY: &str = "configurable";
const DESCRIPTOR_ENUMERABLE_PROPERTY: &str = "enumerable";
const DESCRIPTOR_GET_PROPERTY: &str = "get";
const DESCRIPTOR_SET_PROPERTY: &str = "set";
const DESCRIPTOR_VALUE_PROPERTY: &str = "value";
const DESCRIPTOR_WRITABLE_PROPERTY: &str = "writable";

impl Context {
    pub(in crate::runtime::native) fn object_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Object) {
            return Ok(Value::NativeFunction(id));
        }

        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.object_prototype_id_with_constructor(constructor.clone())?;
        let name = self.native_function_name_value(NativeFunctionKind::Object)?;
        self.push_native_function_with_id(id, NativeFunctionKind::Object, prototype, name)?;
        self.install_object_static_methods(id)?;
        self.insert_global_builtin(OBJECT_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_object_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
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
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => self.create_object_from_constructor(),
        }
    }

    pub(in crate::runtime::native) fn eval_object_define_property(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let target = Self::argument_or_undefined(values.first());
        let mut property = self.object_property_key(values.get(1))?;
        let key = self.intern_dynamic_property_key(&mut property)?;
        let descriptor_value = Self::argument_or_undefined(values.get(2));
        let descriptor = self.data_property_update_from_value(&descriptor_value)?;
        match &target {
            Value::Object(id) => {
                self.objects.define_property(
                    *id,
                    key,
                    property.name(),
                    descriptor,
                    self.limits.max_object_properties,
                )?;
            }
            Value::Function(id) => {
                self.define_function_property_key(*id, property.name(), key, descriptor)?;
            }
            Value::NativeFunction(id) => {
                self.define_native_function_property_key(*id, property.name(), key, descriptor)?;
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::HostFunction(_)
            | Value::Error(_) => {
                return Err(Error::runtime(
                    "Object.defineProperty target must be an object",
                ));
            }
        }
        Ok(target)
    }

    pub(in crate::runtime::native) fn eval_object_get_own_property_descriptor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let target = Self::argument_or_undefined(values.first());
        let property = self.object_property_key(values.get(1))?;
        let descriptor = match &target {
            Value::Object(id) => {
                if let Some(descriptor) =
                    self.string_object_own_property_descriptor(*id, &property)?
                {
                    Some(descriptor)
                } else {
                    self.objects
                        .own_property_descriptor(*id, property.lookup())?
                }
            }
            Value::Function(id) => {
                self.function_own_property_descriptor_lookup(*id, property.lookup())?
            }
            Value::NativeFunction(id) => {
                self.native_function_own_property_descriptor_lookup(*id, property.lookup())?
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
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

    fn string_object_own_property_descriptor(
        &mut self,
        id: ObjectId,
        property: &DynamicPropertyKey,
    ) -> Result<Option<DataPropertyDescriptor>> {
        let Some(ch) = self.objects.string_object_character(id, property.name())? else {
            return Ok(None);
        };
        let value = self.heap_string_char_value(ch)?;
        Ok(Some(DataPropertyDescriptor::new(
            value,
            PropertyWritable::No,
            PropertyEnumerable::Yes,
            PropertyConfigurable::No,
        )))
    }

    pub(in crate::runtime::native) fn eval_object_has_own(
        &self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let target = Self::argument_or_undefined(values.first());
        let property = self.object_property_key(values.get(1))?;
        self.has_own_property_value(&target, &property)
            .map(Value::Bool)
    }

    pub(in crate::runtime::native) fn eval_object_keys(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let target = Self::argument_or_undefined(values.first());
        let keys = self.own_enumerable_keys(&target)?;
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        let mut elements = Vec::with_capacity(keys.len());
        for key in keys {
            elements.push(self.heap_string_value(&key)?);
        }
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
        let function = self.create_native_function(kind, Value::Undefined)?;
        let key = self.intern_property_key(name)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, function, PropertyEnumerable::No);
        Ok(())
    }

    fn argument_or_undefined(value: Option<&Value>) -> Value {
        value.cloned().unwrap_or(Value::Undefined)
    }

    fn object_property_key(&self, value: Option<&Value>) -> Result<DynamicPropertyKey> {
        let value = value.cloned().unwrap_or(Value::Undefined);
        self.dynamic_property_key(&value)
    }

    fn data_property_update_from_value(&mut self, value: &Value) -> Result<DataPropertyUpdate> {
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

    fn reject_accessor_descriptor(&mut self, descriptor: &Value) -> Result<()> {
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
        &mut self,
        descriptor: &Value,
        property: &str,
    ) -> Result<Option<Value>> {
        if !has_property(&self.objects, descriptor, self.property_lookup(property))? {
            return Ok(None);
        }
        self.get_property_value(descriptor, property).map(Some)
    }

    fn optional_descriptor_writable(
        &mut self,
        descriptor: &Value,
    ) -> Result<Option<PropertyWritable>> {
        self.optional_descriptor_bool(descriptor, DESCRIPTOR_WRITABLE_PROPERTY)
            .map(|value| value.map(Self::property_writable))
    }

    fn optional_descriptor_enumerable(
        &mut self,
        descriptor: &Value,
    ) -> Result<Option<PropertyEnumerable>> {
        self.optional_descriptor_bool(descriptor, DESCRIPTOR_ENUMERABLE_PROPERTY)
            .map(|value| value.map(Self::property_enumerable))
    }

    fn optional_descriptor_configurable(
        &mut self,
        descriptor: &Value,
    ) -> Result<Option<PropertyConfigurable>> {
        self.optional_descriptor_bool(descriptor, DESCRIPTOR_CONFIGURABLE_PROPERTY)
            .map(|value| value.map(Self::property_configurable))
    }

    fn optional_descriptor_bool(
        &mut self,
        descriptor: &Value,
        property: &str,
    ) -> Result<Option<bool>> {
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
        let keys = self.descriptor_property_keys()?;
        let descriptor_value = self.runtime_value(descriptor.value())?;
        let properties = vec![
            Self::descriptor_object_property(
                keys.value(),
                DESCRIPTOR_VALUE_PROPERTY,
                descriptor_value,
            ),
            Self::descriptor_object_property(
                keys.writable(),
                DESCRIPTOR_WRITABLE_PROPERTY,
                Value::Bool(descriptor.writable().is_yes()),
            ),
            Self::descriptor_object_property(
                keys.enumerable(),
                DESCRIPTOR_ENUMERABLE_PROPERTY,
                Value::Bool(descriptor.enumerable().is_yes()),
            ),
            Self::descriptor_object_property(
                keys.configurable(),
                DESCRIPTOR_CONFIGURABLE_PROPERTY,
                Value::Bool(descriptor.configurable().is_yes()),
            ),
        ];
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            properties,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    const fn descriptor_object_property(
        key: PropertyKey,
        name: &'static str,
        value: Value,
    ) -> ObjectPropertyInit<'static> {
        ObjectPropertyInit::new(key, name, value, PropertyEnumerable::Yes)
    }

    fn descriptor_property_keys(&mut self) -> Result<DescriptorPropertyKeys> {
        if let Some(keys) = self.descriptor_property_keys {
            return Ok(keys);
        }
        let keys = DescriptorPropertyKeys::new(
            self.intern_property_key(DESCRIPTOR_VALUE_PROPERTY)?,
            self.intern_property_key(DESCRIPTOR_WRITABLE_PROPERTY)?,
            self.intern_property_key(DESCRIPTOR_ENUMERABLE_PROPERTY)?,
            self.intern_property_key(DESCRIPTOR_CONFIGURABLE_PROPERTY)?,
        );
        self.descriptor_property_keys = Some(keys);
        Ok(keys)
    }

    fn has_own_property_value(
        &self,
        target: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<bool> {
        match target {
            Value::Object(id) => self.objects.has_own(*id, property.lookup()),
            Value::Function(id) => self.has_function_property_lookup(*id, property.lookup()),
            Value::NativeFunction(id) => {
                self.has_native_function_property_lookup(*id, property.lookup())
            }
            Value::Error(_) | Value::String(_) | Value::HeapString(_) => {
                has_property(&self.objects, target, property.lookup())
            }
            Value::Bool(_) | Value::Number(_) | Value::Symbol(_) => Ok(false),
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
            Value::Error(_) | Value::String(_) | Value::HeapString(_) => {
                self.enumerable_keys(target)
            }
            Value::Bool(_) | Value::Number(_) | Value::Symbol(_) => Ok(Vec::new()),
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
