use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        object::{DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyWritable},
        object::{PropertyKey, PropertyUpdate},
        property::{delete_property, get_property, get_property_with_receiver, has_property},
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
        let key = self.object_property_key(slice.get(1))?;
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
        let Value::Object(_) = &target else {
            return Err(Error::type_error(REFLECT_TARGET_NOT_OBJECT_ERROR));
        };
        let mut dynamic = self.object_property_key(slice.get(1))?;
        let name = dynamic.name().to_owned();
        let property_key = self.intern_dynamic_property_key(&mut dynamic)?;
        let value = Self::argument_or_undefined(slice.get(2));
        self.set_property_value_with_accessors(&target, property_key, &name, value)?;
        Ok(Value::Bool(true))
    }

    pub(in crate::runtime) fn eval_reflect_has(
        &self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::require_reflect_object(slice.first())?;
        let key = self.object_property_key(slice.get(1))?;
        let present = has_property(&self.objects, &target, key.lookup())?;
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
        let key = self.object_property_key(slice.get(1))?;
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
        Self::require_reflect_object(args.as_slice().first())?;
        self.eval_object_set_prototype_of(args)?;
        Ok(Value::Bool(true))
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
        Self::require_reflect_object(args.as_slice().first())?;
        self.eval_object_prevent_extensions(args)?;
        Ok(Value::Bool(true))
    }

    pub(in crate::runtime) fn eval_reflect_get_own_property_descriptor(
        &mut self,
        args: RuntimeCallArgs<'_>,
        _this: &Value,
    ) -> Result<Value> {
        Self::require_reflect_object(args.as_slice().first())?;
        self.eval_object_get_own_property_descriptor(args)
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
        Self::require_reflect_object(args.as_slice().first())?;
        self.eval_object_get_own_property_names(args)
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
        if !matches!(target, Value::Function(_) | Value::NativeFunction(_)) {
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

    fn reflect_length_from_value(value: &Value) -> Result<usize> {
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
