use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        object::{
            DataPropertyUpdate, OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable,
            PropertyLookup, PropertyWritable,
        },
        object::{PropertyKey, PropertyUpdate},
        property::{delete_property, get_property, get_property_with_receiver},
    },
    value::{ObjectId, Value},
};

use super::{NativeFunctionKind, REFLECT_NAME};

const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const REFLECT_TARGET_NOT_OBJECT_ERROR: &str = "Reflect target must be an object";
const REFLECT_APPLY_NOT_CALLABLE_ERROR: &str = "Reflect.apply target is not callable";
const REFLECT_CONSTRUCT_NOT_CONSTRUCTOR_ERROR: &str =
    "Reflect.construct target is not a constructor";
const REFLECT_ARGUMENTS_NOT_LIST_ERROR: &str = "Reflect argument list must be an array-like object";
const REFLECT_PROTOTYPE_NOT_OBJECT_ERROR: &str =
    "Reflect.setPrototypeOf prototype must be an object or null";
const ARRAY_LIKE_LENGTH_PROPERTY: &str = "length";

const REFLECT_METHODS: [(&str, NativeFunctionKind); 13] = [
    ("apply", NativeFunctionKind::ReflectApply),
    ("construct", NativeFunctionKind::ReflectConstruct),
    ("defineProperty", NativeFunctionKind::ReflectDefineProperty),
    ("deleteProperty", NativeFunctionKind::ReflectDeleteProperty),
    ("get", NativeFunctionKind::ReflectGet),
    (
        "getOwnPropertyDescriptor",
        NativeFunctionKind::ReflectGetOwnPropertyDescriptor,
    ),
    ("getPrototypeOf", NativeFunctionKind::ReflectGetPrototypeOf),
    ("has", NativeFunctionKind::ReflectHas),
    ("isExtensible", NativeFunctionKind::ReflectIsExtensible),
    ("ownKeys", NativeFunctionKind::ReflectOwnKeys),
    (
        "preventExtensions",
        NativeFunctionKind::ReflectPreventExtensions,
    ),
    ("set", NativeFunctionKind::ReflectSet),
    ("setPrototypeOf", NativeFunctionKind::ReflectSetPrototypeOf),
];

impl Context {
    pub(in crate::runtime) fn reflect_object_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(REFLECT_NAME) {
            return binding.value(REFLECT_NAME);
        }
        self.object_constructor_value()?;
        let constructor_key = self.object_constructor_property_key()?;
        let id = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        for (name, kind) in REFLECT_METHODS {
            let function = self.create_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(id, name, function)?;
        }
        self.define_reflect_to_string_tag(id)?;
        let object = Value::Object(id);
        self.insert_global_builtin(REFLECT_NAME, object.clone())?;
        Ok(object)
    }

    fn define_reflect_to_string_tag(&mut self, id: ObjectId) -> Result<()> {
        let tag = self.heap_string_value(REFLECT_NAME)?;
        let key = self.reflect_well_known_symbol_key(SYMBOL_TO_STRING_TAG_PROPERTY)?;
        self.objects.define_property(
            id,
            key,
            SYMBOL_TO_STRING_TAG_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(tag),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn reflect_well_known_symbol_key(&mut self, property: &str) -> Result<PropertyKey> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_property_value(&constructor, property)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime("well-known Symbol property is not a symbol"));
        };
        Ok(PropertyKey::symbol(symbol.id()))
    }

    pub(in crate::runtime) fn eval_reflect_get(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::require_reflect_object(slice.first())?;
        let key = self.reflect_property_key(slice.get(1))?;
        let raw = match slice.get(2) {
            Some(receiver) if !matches!(target, Value::Function(_) | Value::NativeFunction(_)) => {
                let Value::Object(id) = &target else {
                    return Err(Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR));
                };
                get_property_with_receiver(&self.objects, *id, receiver, key.lookup())?
            }
            _ => get_property(&self.objects, &target, key.lookup())?,
        };
        self.runtime_property_value(raw)
    }

    pub(in crate::runtime) fn eval_reflect_set(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::require_reflect_object(slice.first())?;
        let Value::Object(id) = target else {
            return Err(Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR));
        };
        let mut dynamic = self.reflect_property_key(slice.get(1))?;
        let name = dynamic.name().to_owned();
        let property_key = self.intern_dynamic_property_key(&mut dynamic)?;
        let value = Self::argument_or_undefined(slice.get(2));
        let target_value = Value::Object(id);
        let receiver = slice.get(3).unwrap_or(&target_value).clone();
        let updated = self.reflect_set_ordinary(id, property_key, &name, value, &receiver)?;
        Ok(Value::Bool(updated))
    }

    pub(in crate::runtime) fn eval_reflect_has(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::require_reflect_object(slice.first())?;
        let key = self.reflect_property_key(slice.get(1))?;
        let present = self.has_dynamic_property_value(&target, &key)?;
        Ok(Value::Bool(present))
    }

    pub(in crate::runtime) fn eval_reflect_delete_property(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::require_reflect_object(slice.first())?;
        let Value::Object(_) = &target else {
            return Err(Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR));
        };
        let key = self.reflect_property_key(slice.get(1))?;
        let deleted = delete_property(&mut self.objects, &target, key.lookup())?;
        Ok(Value::Bool(deleted))
    }

    pub(in crate::runtime) fn eval_reflect_get_prototype_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        Self::require_reflect_object(args.as_slice().first())?;
        self.eval_object_get_prototype_of(args)
    }

    pub(in crate::runtime) fn eval_reflect_set_prototype_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::require_reflect_object(slice.first())?;
        let prototype = Self::argument_or_undefined(slice.get(1));
        Self::validate_reflect_prototype_value(&prototype)?;
        let updated = match target {
            Value::Object(id) if self.objects.is_proxy(id) => {
                self.proxy_set_prototype_of(id, prototype)?
            }
            Value::Object(id) => self.objects.try_set_prototype_value(id, &prototype)?,
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => false,
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Err(Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR)),
        };
        Ok(Value::Bool(updated))
    }

    pub(in crate::runtime) fn eval_reflect_is_extensible(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        Self::require_reflect_object(args.as_slice().first())?;
        self.eval_object_is_extensible(args)
    }

    pub(in crate::runtime) fn eval_reflect_prevent_extensions(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let target = Self::require_reflect_object(args.as_slice().first())?;
        let Value::Object(id) = target else {
            return Ok(Value::Bool(true));
        };
        if self.objects.is_proxy(id) {
            return self.proxy_prevent_extensions(id).map(Value::Bool);
        }
        self.objects.prevent_extensions(id)?;
        Ok(Value::Bool(true))
    }

    pub(in crate::runtime) fn eval_reflect_get_own_property_descriptor(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::require_reflect_object(slice.first())?;
        let property = self.reflect_property_key(slice.get(1))?;
        let Some(descriptor) = self.own_property_descriptor_value(&target, &property)? else {
            return Ok(Value::Undefined);
        };
        self.create_property_descriptor_object(&descriptor)
    }

    pub(in crate::runtime) fn eval_reflect_define_property(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        Self::require_reflect_object(args.as_slice().first())?;
        self.eval_object_define_property(args)?;
        Ok(Value::Bool(true))
    }

    pub(in crate::runtime) fn eval_reflect_own_keys(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let target = Self::require_reflect_object(args.as_slice().first())?;
        let mut elements = Vec::new();
        match target {
            Value::Object(id) if self.objects.is_proxy(id) => {
                for key in self.proxy_own_keys(id)? {
                    elements.push(self.heap_string_value(&key)?);
                }
            }
            Value::Object(id) => {
                let names = self.objects.own_property_names(id, &self.atoms)?;
                elements.reserve(names.len());
                for key in names {
                    elements.push(self.heap_string_value(&key)?);
                }
                for symbol in self.objects.own_property_symbols(id, &self.symbols)? {
                    elements.push(Value::Symbol(symbol));
                }
            }
            Value::Function(_) | Value::NativeFunction(_) | Value::Error(_) => {
                for key in self.own_property_names(&target)? {
                    elements.push(self.heap_string_value(&key)?);
                }
            }
            Value::HostFunction(_)
            | Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => return Err(Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR)),
        }
        self.create_array_from_elements(elements)
    }

    pub(in crate::runtime) fn eval_reflect_apply(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::argument_or_undefined(slice.first());
        if !Self::is_callable(&target) {
            return Err(Error::type_error(REFLECT_APPLY_NOT_CALLABLE_ERROR));
        }
        let this_arg = Self::argument_or_undefined(slice.get(1));
        let args_list = Self::argument_or_undefined(slice.get(2));
        let call_args = self.reflect_argument_list(&args_list)?;
        self.eval_call_value(target, &call_args, this_arg)
    }

    pub(in crate::runtime) fn eval_reflect_construct(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::argument_or_undefined(slice.first());
        if !self.is_constructor_value(&target)? {
            return Err(Error::type_error(REFLECT_CONSTRUCT_NOT_CONSTRUCTOR_ERROR));
        }
        let new_target = slice.get(2).map_or(&target, |value| value);
        if !self.is_constructor_value(new_target)? {
            return Err(Error::type_error(REFLECT_CONSTRUCT_NOT_CONSTRUCTOR_ERROR));
        }
        let args_list = Self::argument_or_undefined(slice.get(1));
        let construct_args = self.reflect_argument_list(&args_list)?;
        self.eval_new_value(target, &construct_args)
    }

    fn require_reflect_object(value: Option<&Value>) -> Result<Value> {
        let value = value.cloned().unwrap_or(Value::Undefined);
        if matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
                | Value::Error(_)
        ) {
            return Ok(value);
        }
        Err(Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR))
    }

    fn validate_reflect_prototype_value(value: &Value) -> Result<()> {
        match value {
            Value::Object(_) | Value::Null => Ok(()),
            Value::Undefined
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => Err(Error::type_error(REFLECT_PROTOTYPE_NOT_OBJECT_ERROR)),
        }
    }

    fn reflect_property_key(
        &mut self,
        value: Option<&Value>,
    ) -> Result<crate::runtime::property::DynamicPropertyKey> {
        let value = Self::argument_or_undefined(value);
        if matches!(value, Value::Object(_)) {
            let to_string = self.get_property_value(&value, "toString")?;
            if Self::is_callable(&to_string) {
                let key = self
                    .eval_call_completion(to_string, &[], value.clone())?
                    .into_native_value_result()?;
                return self.dynamic_property_key(&key);
            }
        }
        self.dynamic_property_key(&value)
    }

    fn reflect_set_ordinary(
        &mut self,
        target: ObjectId,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        receiver: &Value,
    ) -> Result<bool> {
        let lookup = PropertyLookup::from_key(property_name, property);
        let descriptor = self.objects.own_property_descriptor(target, lookup)?;
        if let Some(descriptor) = descriptor {
            return self.reflect_set_with_descriptor(
                property,
                property_name,
                value,
                receiver,
                descriptor,
            );
        }
        let prototype = self.objects.prototype_value(target)?;
        if let Value::Object(prototype) = prototype {
            return self.reflect_set_ordinary(prototype, property, property_name, value, receiver);
        }
        self.reflect_set_data_property(property, property_name, value, receiver)
    }

    fn reflect_set_with_descriptor(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        receiver: &Value,
        descriptor: OwnPropertyDescriptor,
    ) -> Result<bool> {
        match descriptor {
            OwnPropertyDescriptor::Data(descriptor) => {
                if !descriptor.writable().is_yes() {
                    return Ok(false);
                }
                self.reflect_set_data_property(property, property_name, value, receiver)
            }
            OwnPropertyDescriptor::Accessor(descriptor) => {
                if !descriptor.has_setter() {
                    return Ok(false);
                }
                self.call_accessor_function(descriptor.set(), receiver.clone(), &[value])?;
                Ok(true)
            }
        }
    }

    fn reflect_set_data_property(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        receiver: &Value,
    ) -> Result<bool> {
        let Value::Object(receiver) = receiver else {
            return Ok(false);
        };
        let lookup = PropertyLookup::from_key(property_name, property);
        let mut new_property = true;
        if let Some(descriptor) = self.objects.own_property_descriptor(*receiver, lookup)? {
            new_property = false;
            match descriptor {
                OwnPropertyDescriptor::Accessor(_) => return Ok(false),
                OwnPropertyDescriptor::Data(descriptor) if !descriptor.writable().is_yes() => {
                    return Ok(false);
                }
                OwnPropertyDescriptor::Data(_) => {}
            }
        }
        self.objects.define_property(
            *receiver,
            property,
            property_name,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                new_property.then_some(PropertyWritable::Yes),
                new_property.then_some(PropertyEnumerable::Yes),
                new_property.then_some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        Ok(true)
    }

    /// Spec `CreateListFromArrayLike` for the default element types.
    fn reflect_argument_list(&mut self, value: &Value) -> Result<Vec<Value>> {
        if !matches!(value, Value::Object(_)) {
            return Err(Error::type_error(REFLECT_ARGUMENTS_NOT_LIST_ERROR));
        }
        let length_value = self.get_property_value(value, ARRAY_LIKE_LENGTH_PROPERTY)?;
        let length = Self::reflect_length_from_value(&length_value)?;
        let mut list = Vec::new();
        for index in 0..length {
            self.step()?;
            list.push(self.get_property_value(value, &index.to_string())?);
        }
        Ok(list)
    }

    pub(in crate::runtime::native) fn reflect_length_from_value(value: &Value) -> Result<usize> {
        let number = Self::value_to_number(value);
        if number.is_nan() || number <= 0.0 {
            return Ok(0);
        }
        let capped = number.floor().min(f64::from(u32::MAX));
        format!("{capped:.0}")
            .parse::<usize>()
            .map_err(|_| Error::limit("Reflect argument list length exceeded supported range"))
    }
}
