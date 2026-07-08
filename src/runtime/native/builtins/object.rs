use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call_args::RuntimeCallArgs,
    runtime::object::{
        AccessorPropertyUpdate, DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit,
        OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable, PropertyKey,
        PropertyUpdate, PropertyWritable,
    },
    runtime::property::{DynamicPropertyKey, has_property},
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{
    NativeFunctionKind, OBJECT_DEFINE_PROPERTY_NAME, OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME,
    OBJECT_GET_OWN_PROPERTY_NAMES_NAME, OBJECT_GET_PROTOTYPE_OF_NAME, OBJECT_HAS_OWN_NAME,
    OBJECT_KEYS_NAME, OBJECT_NAME, OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME,
    OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME,
};
use crate::runtime::property::well_known::DescriptorPropertyKeys;

const DESCRIPTOR_CONFIGURABLE_PROPERTY: &str = "configurable";
const DESCRIPTOR_ENUMERABLE_PROPERTY: &str = "enumerable";
const DESCRIPTOR_GET_PROPERTY: &str = "get";
const DESCRIPTOR_SET_PROPERTY: &str = "set";
const DESCRIPTOR_VALUE_PROPERTY: &str = "value";
const DESCRIPTOR_WRITABLE_PROPERTY: &str = "writable";
const OBJECT_PROTOTYPE_PROPERTY: &str = "prototype";

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
        self.install_object_prototype_methods(&constructor)?;
        self.insert_global_builtin(OBJECT_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_object_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_object_constructor(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_object_constructor(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let Some(value) = args.first() else {
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
        let descriptor = self.property_update_from_value(&descriptor_value)?;
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
                let PropertyUpdate::Data(descriptor) = descriptor else {
                    return Err(Error::runtime(
                        "accessor properties are not supported on function objects",
                    ));
                };
                self.define_function_property_key(*id, property.name(), key, descriptor)?;
            }
            Value::NativeFunction(id) => {
                let PropertyUpdate::Data(descriptor) = descriptor else {
                    return Err(Error::runtime(
                        "accessor properties are not supported on function objects",
                    ));
                };
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
                    Some(OwnPropertyDescriptor::Data(descriptor))
                } else {
                    self.objects
                        .own_property_descriptor(*id, property.lookup())?
                }
            }
            Value::Function(id) => self
                .function_own_property_descriptor_lookup(*id, property.lookup())?
                .map(OwnPropertyDescriptor::Data),
            Value::NativeFunction(id) => self
                .native_function_own_property_descriptor_lookup(*id, property.lookup())?
                .map(OwnPropertyDescriptor::Data),
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

    pub(in crate::runtime::native) fn eval_object_get_prototype_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let target = Self::argument_or_undefined(values.first());
        match target {
            Value::Object(id) => self.objects.prototype_value(id),
            Value::Function(id) => self.function_object_prototype_value(id),
            Value::NativeFunction(id) => self.native_function_object_prototype_value(id),
            Value::Undefined | Value::Null => Err(Error::runtime(
                "Object.getPrototypeOf target cannot be converted to an object",
            )),
            Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::HostFunction(_)
            | Value::Error(_) => Err(Error::runtime(
                "Object.getPrototypeOf target must be an object",
            )),
        }
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

    pub(in crate::runtime::native) fn eval_object_get_own_property_names(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let target = Self::argument_or_undefined(values.first());
        let keys = self.own_property_names(&target)?;
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

    pub(in crate::runtime::native) fn eval_object_prototype_has_own_property(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let property = self.object_property_key(args.as_slice().first())?;
        self.has_own_property_value(this_value, &property)
            .map(Value::Bool)
    }

    pub(in crate::runtime::native) fn eval_object_prototype_property_is_enumerable(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let property = self.object_property_key(args.as_slice().first())?;
        self.property_is_enumerable_value(this_value, &property)
            .map(Value::Bool)
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
            OBJECT_GET_OWN_PROPERTY_NAMES_NAME,
            NativeFunctionKind::ObjectGetOwnPropertyNames,
        )?;
        self.define_object_static_method(
            constructor,
            OBJECT_GET_PROTOTYPE_OF_NAME,
            NativeFunctionKind::ObjectGetPrototypeOf,
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

    fn install_object_prototype_methods(&mut self, constructor: &Value) -> Result<()> {
        let Value::Object(prototype) =
            self.get_property_value(constructor, OBJECT_PROTOTYPE_PROPERTY)?
        else {
            return Err(Error::runtime("Object prototype is not an object"));
        };
        self.define_object_prototype_method(
            prototype,
            OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME,
            NativeFunctionKind::ObjectPrototypeHasOwnProperty,
        )?;
        self.define_object_prototype_method(
            prototype,
            OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME,
            NativeFunctionKind::ObjectPrototypePropertyIsEnumerable,
        )
    }

    fn define_object_prototype_method(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_ephemeral_native_function(kind, Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, name, function)
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

    fn property_update_from_value(&mut self, value: &Value) -> Result<PropertyUpdate> {
        if !matches!(value, Value::Object(_)) {
            return Err(Error::runtime("property descriptor must be an object"));
        }
        let get = self.optional_descriptor_accessor(value, DESCRIPTOR_GET_PROPERTY)?;
        let set = self.optional_descriptor_accessor(value, DESCRIPTOR_SET_PROPERTY)?;
        let data_value = self.optional_descriptor_value(value, DESCRIPTOR_VALUE_PROPERTY)?;
        let writable = self.optional_descriptor_writable(value)?;
        let enumerable = self.optional_descriptor_enumerable(value)?;
        let configurable = self.optional_descriptor_configurable(value)?;
        if get.is_some() || set.is_some() {
            if data_value.is_some() || writable.is_some() {
                return Err(Error::type_error(
                    "property descriptor cannot mix accessor and data attributes",
                ));
            }
            return Ok(PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                get,
                set,
                enumerable,
                configurable,
            )));
        }
        Ok(PropertyUpdate::Data(DataPropertyUpdate::new(
            data_value,
            writable,
            enumerable,
            configurable,
        )))
    }

    /// Reads a `get`/`set` descriptor field. Present fields must be callable
    /// or `undefined`; a missing field returns `None`.
    fn optional_descriptor_accessor(
        &mut self,
        descriptor: &Value,
        property: &str,
    ) -> Result<Option<Value>> {
        let Some(function) = self.optional_descriptor_value(descriptor, property)? else {
            return Ok(None);
        };
        if !matches!(
            function,
            Value::Undefined | Value::Function(_) | Value::NativeFunction(_)
        ) {
            return Err(Error::type_error(format!(
                "property descriptor field '{property}' must be callable or undefined"
            )));
        }
        Ok(Some(function))
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
        descriptor: &OwnPropertyDescriptor,
    ) -> Result<Value> {
        let keys = self.descriptor_property_keys()?;
        let properties = match descriptor {
            OwnPropertyDescriptor::Data(descriptor) => {
                let descriptor_value = self.runtime_value(descriptor.value())?;
                vec![
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
                ]
            }
            OwnPropertyDescriptor::Accessor(descriptor) => vec![
                Self::descriptor_object_property(
                    keys.get(),
                    DESCRIPTOR_GET_PROPERTY,
                    descriptor.get(),
                ),
                Self::descriptor_object_property(
                    keys.set(),
                    DESCRIPTOR_SET_PROPERTY,
                    descriptor.set(),
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
            ],
        };
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
            self.intern_property_key(DESCRIPTOR_GET_PROPERTY)?,
            self.intern_property_key(DESCRIPTOR_SET_PROPERTY)?,
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

    fn property_is_enumerable_value(
        &mut self,
        target: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<bool> {
        let Some(descriptor) = self.own_property_descriptor_value(target, property)? else {
            return Ok(false);
        };
        let enumerable = match descriptor {
            OwnPropertyDescriptor::Data(descriptor) => descriptor.enumerable(),
            OwnPropertyDescriptor::Accessor(descriptor) => descriptor.enumerable(),
        };
        Ok(enumerable.is_yes())
    }

    fn own_property_descriptor_value(
        &mut self,
        target: &Value,
        property: &DynamicPropertyKey,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        match target {
            Value::Object(id) => {
                if let Some(descriptor) =
                    self.string_object_own_property_descriptor(*id, property)?
                {
                    return Ok(Some(OwnPropertyDescriptor::Data(descriptor)));
                }
                self.objects.own_property_descriptor(*id, property.lookup())
            }
            Value::Function(id) => Ok(self
                .function_own_property_descriptor_lookup(*id, property.lookup())?
                .map(OwnPropertyDescriptor::Data)),
            Value::NativeFunction(id) => Ok(self
                .native_function_own_property_descriptor_lookup(*id, property.lookup())?
                .map(OwnPropertyDescriptor::Data)),
            Value::Error(_) | Value::String(_) | Value::HeapString(_) => {
                if self.has_own_property_value(target, property)? {
                    Ok(Some(OwnPropertyDescriptor::Data(
                        DataPropertyDescriptor::new(
                            self.get_property_value(target, property.name())?,
                            PropertyWritable::Yes,
                            PropertyEnumerable::Yes,
                            PropertyConfigurable::Yes,
                        ),
                    )))
                } else {
                    Ok(None)
                }
            }
            Value::Bool(_) | Value::Number(_) | Value::Symbol(_) => Ok(None),
            Value::Undefined | Value::Null | Value::HostFunction(_) => Err(Error::runtime(
                "Object.prototype.propertyIsEnumerable target cannot be converted to an object",
            )),
        }
    }

    pub(in crate::runtime) fn own_enumerable_keys(&self, target: &Value) -> Result<Vec<String>> {
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

    fn own_property_names(&self, target: &Value) -> Result<Vec<String>> {
        match target {
            Value::Object(id) => self.objects.own_property_names(*id, &self.atoms),
            Value::Function(id) => self.function_enumerable_keys(*id),
            Value::NativeFunction(id) => self.native_function_enumerable_keys(*id),
            Value::Error(_) | Value::String(_) | Value::HeapString(_) => {
                self.enumerable_keys(target)
            }
            Value::Bool(_) | Value::Number(_) | Value::Symbol(_) => Ok(Vec::new()),
            Value::Undefined | Value::Null | Value::HostFunction(_) => Err(Error::runtime(
                "Object.getOwnPropertyNames target cannot be converted to an object",
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
