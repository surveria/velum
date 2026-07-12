use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        object::{DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyWritable},
        object::{PropertyKey, PropertyUpdate},
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
        let value = self.get_named(&constructor, property)?;
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
        let receiver = slice.get(2).unwrap_or(&target);
        let Some(read) =
            self.semantic_property_read_with_receiver(&target, receiver, key.lookup())?
        else {
            return Err(Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR));
        };
        self.finish_semantic_property_read(read, receiver, key.lookup())
    }

    pub(in crate::runtime) fn eval_reflect_set(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::require_reflect_object(slice.first())?;
        let dynamic = self.reflect_property_key(slice.get(1))?;
        let value = Self::argument_or_undefined(slice.get(2));
        let receiver = slice.get(3).unwrap_or(&target).clone();
        let updated = self.set(
            &target,
            dynamic.lookup(),
            value,
            &receiver,
            crate::runtime::abstract_operations::SetFailureBehavior::ReturnFalse,
        )?;
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
        let deleted = self.delete_property_value_with_lookup(&target, key.lookup())?;
        Ok(Value::Bool(deleted))
    }

    pub(in crate::runtime) fn eval_reflect_get_prototype_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let target = Self::require_reflect_object(args.as_slice().first())?;
        self.semantic_get_prototype(&target)?
            .ok_or_else(|| Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR))
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
        let updated = self
            .semantic_try_set_prototype(&target, prototype)?
            .ok_or_else(|| Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR))?;
        Ok(Value::Bool(updated))
    }

    pub(in crate::runtime) fn eval_reflect_is_extensible(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let target = Self::require_reflect_object(args.as_slice().first())?;
        self.semantic_is_extensible(&target)?
            .map(Value::Bool)
            .ok_or_else(|| Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR))
    }

    pub(in crate::runtime) fn eval_reflect_prevent_extensions(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let target = Self::require_reflect_object(args.as_slice().first())?;
        self.semantic_prevent_extensions(&target)?
            .map(Value::Bool)
            .ok_or_else(|| Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR))
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
        let values = args.as_slice();
        let target = Self::require_reflect_object(values.first())?;
        let mut property = self.reflect_property_key(values.get(1))?;
        let descriptor = Self::argument_or_undefined(values.get(2));
        self.semantic_define_own_property_from_value(&target, &mut property, &descriptor)
            .map(Value::Bool)
    }

    pub(in crate::runtime) fn eval_reflect_own_keys(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let target = Self::require_reflect_object(args.as_slice().first())?;
        let elements = self.semantic_own_property_keys(&target)?;
        self.create_array_from_elements(elements)
    }

    pub(in crate::runtime) fn eval_reflect_apply(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::argument_or_undefined(slice.first());
        if !self.semantic_is_callable(&target)? {
            return Err(Error::type_error(REFLECT_APPLY_NOT_CALLABLE_ERROR));
        }
        let this_arg = Self::argument_or_undefined(slice.get(1));
        let args_list = Self::argument_or_undefined(slice.get(2));
        let call_args = self.reflect_argument_list(&args_list)?;
        self.call_value(&target, &call_args, this_arg)
    }

    pub(in crate::runtime) fn eval_reflect_construct(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::argument_or_undefined(slice.first());
        if !self.semantic_is_constructor(&target)? {
            return Err(Error::type_error(REFLECT_CONSTRUCT_NOT_CONSTRUCTOR_ERROR));
        }
        let new_target = slice.get(2).cloned().unwrap_or_else(|| target.clone());
        if !self.semantic_is_constructor(&new_target)? {
            return Err(Error::type_error(REFLECT_CONSTRUCT_NOT_CONSTRUCTOR_ERROR));
        }
        let args_list = Self::argument_or_undefined(slice.get(1));
        let construct_args = self.reflect_argument_list(&args_list)?;
        self.semantic_construct(&target, &construct_args, new_target)
    }

    fn require_reflect_object(value: Option<&Value>) -> Result<Value> {
        let value = value.cloned().unwrap_or(Value::Undefined);
        if matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        ) {
            return Ok(value);
        }
        Err(Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR))
    }

    fn validate_reflect_prototype_value(value: &Value) -> Result<()> {
        match value {
            Value::Object(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Null => Ok(()),
            Value::Undefined
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::Symbol(_) => Err(Error::type_error(REFLECT_PROTOTYPE_NOT_OBJECT_ERROR)),
        }
    }

    fn reflect_property_key(
        &mut self,
        value: Option<&Value>,
    ) -> Result<crate::runtime::property::DynamicPropertyKey> {
        let value = Self::argument_or_undefined(value);
        self.dynamic_property_key(&value)
    }

    /// Spec `CreateListFromArrayLike` for the default element types.
    fn reflect_argument_list(&mut self, value: &Value) -> Result<Vec<Value>> {
        if !matches!(value, Value::Object(_)) {
            return Err(Error::type_error(REFLECT_ARGUMENTS_NOT_LIST_ERROR));
        }
        let length_value = self.get_named(value, ARRAY_LIKE_LENGTH_PROPERTY)?;
        let length = self.reflect_length_from_value(&length_value)?;
        let mut list = Vec::new();
        for index in 0..length {
            self.step()?;
            list.push(self.get_named(value, &index.to_string())?);
        }
        Ok(list)
    }

    pub(in crate::runtime::native) fn reflect_length_from_value(
        &mut self,
        value: &Value,
    ) -> Result<usize> {
        let length = self.to_length(value)?;
        Self::length_to_usize(
            length,
            "Reflect argument list length exceeded supported range",
        )
    }
}
