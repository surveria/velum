use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::PreferredType,
        call::RuntimeCallArgs,
        object::{
            DataPropertyUpdate, ObjectPrimitiveValue, ObjectPropertyInit, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyLookup, PropertyUpdate, PropertyWritable,
        },
    },
    value::{ErrorName, JsBigInt, NativeFunctionId, ObjectId, Value},
};

use super::{
    BIGINT_AS_INT_N_NAME, BIGINT_AS_UINT_N_NAME, BIGINT_NAME,
    BIGINT_PROTOTYPE_TO_LOCALE_STRING_NAME, BIGINT_PROTOTYPE_TO_STRING_NAME,
    BIGINT_PROTOTYPE_VALUE_OF_NAME, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY,
};

const BIGINT_RECEIVER_ERROR: &str = "BigInt.prototype method requires a BigInt value";
const BIGINT_NUMBER_RANGE_ERROR: &str =
    "The number cannot be converted to a BigInt because it is not an integer";
const BIGINT_RADIX_RANGE_ERROR: &str = "BigInt.prototype.toString radix must be between 2 and 36";
const BIGINT_BIT_LIMIT_ERROR: &str = "BigInt result exceeded the configured bit limit";
const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const SYMBOL_TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";

impl Context {
    pub(in crate::runtime::native) fn bigint_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::BigInt) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.bigint_prototype_id_with_constructor(constructor.clone())?;
        let name = self.native_function_name_value(NativeFunctionKind::BigInt)?;
        self.push_native_function_with_id(
            id,
            NativeFunctionKind::BigInt,
            Value::Object(prototype_id),
            name,
        )?;
        self.install_bigint_static_methods(id)?;
        self.install_bigint_prototype_methods(prototype_id)?;
        self.insert_global_builtin(BIGINT_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_bigint_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = Self::argument_or_undefined(args.as_slice().first());
        let primitive = self.to_primitive(&value, PreferredType::Number)?;
        if let Value::Number(number) = primitive {
            let value = JsBigInt::from_f64_integer(number).ok_or_else(|| {
                Error::exception(ErrorName::RangeError, BIGINT_NUMBER_RANGE_ERROR)
            })?;
            return self.bigint_value(value);
        }
        let value = self.to_bigint(&primitive)?;
        self.bigint_value(value)
    }

    pub(in crate::runtime::native) fn eval_bigint_as_int_n(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_bigint_width_conversion(args.as_slice(), true)
    }

    pub(in crate::runtime::native) fn eval_bigint_as_uint_n(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_bigint_width_conversion(args.as_slice(), false)
    }

    fn eval_bigint_width_conversion(&mut self, args: &[Value], signed: bool) -> Result<Value> {
        let bits = self.to_index(args.first())?;
        let bits = usize::try_from(bits)
            .map_err(|_| Error::limit("BigInt width exceeded supported resource range"))?;
        let value = Self::argument_or_undefined(args.get(1));
        let value = self.to_bigint(&value)?;
        let unchanged = if signed {
            value.unchanged_by_as_int_n(bits)
        } else {
            value.unchanged_by_as_uint_n(bits)
        };
        if bits > self.limits.max_bigint_bits && !unchanged {
            return Err(Error::exception(
                ErrorName::RangeError,
                BIGINT_BIT_LIMIT_ERROR,
            ));
        }
        self.bigint_value(if signed {
            value.as_int_n(bits)
        } else {
            value.as_uint_n(bits)
        })
    }

    pub(in crate::runtime::native) fn eval_bigint_prototype_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.bigint_receiver_value(this_value)?;
        let radix = self.bigint_radix(args.as_slice().first())?;
        self.heap_string_value(&value.to_string_radix(radix))
    }

    pub(in crate::runtime::native) fn eval_bigint_prototype_to_locale_string(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let value = self.bigint_receiver_value(this_value)?;
        self.heap_string_value(&value.to_string())
    }

    pub(in crate::runtime::native) fn eval_bigint_prototype_value_of(
        &self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.bigint_receiver_value(this_value).map(Value::BigInt)
    }

    pub(in crate::runtime::native) fn create_bigint_object_from_value(
        &mut self,
        value: JsBigInt,
    ) -> Result<Value> {
        let prototype = self.bigint_constructor_prototype()?;
        self.objects.create_boxed_primitive(
            ObjectPrimitiveValue::BigInt(value),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(in crate::runtime) fn bigint_prototype_property_value(
        &mut self,
        receiver: &Value,
        property: &str,
    ) -> Result<Value> {
        let prototype = self.bigint_constructor_prototype()?;
        self.get_prototype_property_value_with_receiver(prototype, receiver, property)
    }

    pub(in crate::runtime) fn bigint_prototype_property_value_with_lookup(
        &mut self,
        receiver: &Value,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let prototype = self.bigint_constructor_prototype()?;
        self.get_prototype_property_value_with_lookup(prototype, receiver, property)
    }

    fn bigint_receiver_value(&self, value: &Value) -> Result<JsBigInt> {
        match value {
            Value::BigInt(value) => Ok(value.clone()),
            Value::Object(id) => match self.objects.primitive_value(*id)? {
                Some(ObjectPrimitiveValue::BigInt(value)) => Ok(value.clone()),
                Some(
                    ObjectPrimitiveValue::Bool(_)
                    | ObjectPrimitiveValue::Number(_)
                    | ObjectPrimitiveValue::Symbol(_),
                )
                | None => Err(Error::type_error(BIGINT_RECEIVER_ERROR)),
            },
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => Err(Error::type_error(BIGINT_RECEIVER_ERROR)),
        }
    }

    fn bigint_radix(&mut self, value: Option<&Value>) -> Result<u32> {
        let Some(value) = value else {
            return Ok(10);
        };
        if matches!(value, Value::Undefined) {
            return Ok(10);
        }
        let radix = self.to_integer_or_infinity(value)?;
        if !radix.is_finite() || !(2.0..=36.0).contains(&radix) {
            return Err(Error::exception(
                ErrorName::RangeError,
                BIGINT_RADIX_RANGE_ERROR,
            ));
        }
        format!("{radix:.0}")
            .parse::<u32>()
            .map_err(|_| Error::limit("BigInt radix exceeded supported range"))
    }

    fn bigint_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype_property(
            None,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor,
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime) fn bigint_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.bigint_constructor_value()? else {
            return Err(Error::runtime("BigInt constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("BigInt prototype is not an object")),
        }
    }

    fn install_bigint_static_methods(&mut self, constructor: NativeFunctionId) -> Result<()> {
        self.define_bigint_static_method(
            constructor,
            BIGINT_AS_INT_N_NAME,
            NativeFunctionKind::BigIntAsIntN,
        )?;
        self.define_bigint_static_method(
            constructor,
            BIGINT_AS_UINT_N_NAME,
            NativeFunctionKind::BigIntAsUintN,
        )
    }

    fn define_bigint_static_method(
        &mut self,
        constructor: NativeFunctionId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        let key = self.intern_property_key(name)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, function, PropertyEnumerable::No)?;
        Ok(())
    }

    fn install_bigint_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in [
            (
                BIGINT_PROTOTYPE_TO_LOCALE_STRING_NAME,
                NativeFunctionKind::BigIntPrototypeToLocaleString,
            ),
            (
                BIGINT_PROTOTYPE_TO_STRING_NAME,
                NativeFunctionKind::BigIntPrototypeToString,
            ),
            (
                BIGINT_PROTOTYPE_VALUE_OF_NAME,
                NativeFunctionKind::BigIntPrototypeValueOf,
            ),
        ] {
            let function = self.create_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, function)?;
        }
        self.install_bigint_to_string_tag(prototype)
    }

    fn install_bigint_to_string_tag(&mut self, prototype: ObjectId) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag = self.get_named(&symbol_constructor, SYMBOL_TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(tag) = tag else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(BIGINT_NAME)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(tag.id()),
            SYMBOL_TO_STRING_TAG_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}
