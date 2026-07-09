use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::object::{ObjectPrimitiveValue, ObjectPropertyInit, PropertyEnumerable},
    value::{ErrorName, NativeFunctionId, ObjectId, Value},
};

use super::{
    GLOBAL_PARSE_FLOAT_NAME, GLOBAL_PARSE_INT_NAME, NUMBER_IS_FINITE_NAME, NUMBER_IS_INTEGER_NAME,
    NUMBER_IS_NAN_NAME, NUMBER_IS_SAFE_INTEGER_NAME, NUMBER_NAME,
    NUMBER_PROTOTYPE_TO_EXPONENTIAL_NAME, NUMBER_PROTOTYPE_TO_FIXED_NAME,
    NUMBER_PROTOTYPE_TO_LOCALE_STRING_NAME, NUMBER_PROTOTYPE_TO_PRECISION_NAME,
    NUMBER_PROTOTYPE_TO_STRING_NAME, NUMBER_PROTOTYPE_VALUE_OF_NAME, NativeFunctionKind,
    OBJECT_CONSTRUCTOR_PROPERTY,
};

const NUMBER_EPSILON_PROPERTY: &str = "EPSILON";
const NUMBER_MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
const NUMBER_MAX_SAFE_INTEGER_PROPERTY: &str = "MAX_SAFE_INTEGER";
const NUMBER_MAX_VALUE_PROPERTY: &str = "MAX_VALUE";
const NUMBER_MIN_SAFE_INTEGER: f64 = -9_007_199_254_740_991.0;
const NUMBER_MIN_SAFE_INTEGER_PROPERTY: &str = "MIN_SAFE_INTEGER";
const NUMBER_MIN_VALUE_PROPERTY: &str = "MIN_VALUE";
const NUMBER_NAN_PROPERTY: &str = "NaN";
const NUMBER_NEGATIVE_INFINITY_PROPERTY: &str = "NEGATIVE_INFINITY";
const NUMBER_POSITIVE_INFINITY_PROPERTY: &str = "POSITIVE_INFINITY";
const NUMBER_RADIX_MAX: u32 = 36;
const NUMBER_RADIX_MIN: u32 = 2;
const NUMBER_RADIX_RANGE_ERROR: &str = "Number.prototype.toString radix must be between 2 and 36";
const NUMBER_VALUE_RECEIVER_ERROR: &str =
    "Number.prototype value method requires a number or Number object";
const STRING_NEGATIVE_INFINITY: &str = "-Infinity";
const STRING_POSITIVE_INFINITY: &str = "Infinity";

pub(in crate::runtime::native) fn number_intrinsic_property(property: &str) -> Option<Value> {
    match property {
        NUMBER_EPSILON_PROPERTY => Some(Value::Number(f64::EPSILON)),
        NUMBER_MAX_SAFE_INTEGER_PROPERTY => Some(Value::Number(NUMBER_MAX_SAFE_INTEGER)),
        NUMBER_MAX_VALUE_PROPERTY => Some(Value::Number(f64::MAX)),
        NUMBER_MIN_SAFE_INTEGER_PROPERTY => Some(Value::Number(NUMBER_MIN_SAFE_INTEGER)),
        // Number.MIN_VALUE is the smallest positive subnormal double (5e-324),
        // not the smallest normal one exposed by f64::MIN_POSITIVE.
        NUMBER_MIN_VALUE_PROPERTY => Some(Value::Number(f64::from_bits(1))),
        NUMBER_NAN_PROPERTY => Some(Value::Number(f64::NAN)),
        NUMBER_NEGATIVE_INFINITY_PROPERTY => Some(Value::Number(f64::NEG_INFINITY)),
        NUMBER_POSITIVE_INFINITY_PROPERTY => Some(Value::Number(f64::INFINITY)),
        _ => None,
    }
}

impl Context {
    pub(in crate::runtime::native) fn number_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Number) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.number_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        let name = self.native_function_name_value(NativeFunctionKind::Number)?;
        self.push_native_function_with_id(id, NativeFunctionKind::Number, prototype, name)?;
        self.install_number_static_methods(id)?;
        self.install_number_prototype_methods(prototype_id)?;
        self.insert_global_builtin(NUMBER_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_number_constructor(
        &self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_number_constructor(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_number_constructor(
        &self,
        args: &[Value],
    ) -> Result<Value> {
        self.checked_value(Value::Number(Self::number_argument_value(args.first())))
    }

    pub(in crate::runtime::native) fn construct_number_object(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = Self::eval_native_unary_argument_value(args);
        let number_value = Self::number_argument_value(value);
        let prototype = self.number_constructor_prototype()?;
        self.objects.create_boxed_primitive(
            ObjectPrimitiveValue::Number(number_value),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(in crate::runtime::native) fn create_number_object_from_value(
        &mut self,
        value: f64,
    ) -> Result<Value> {
        let prototype = self.number_constructor_prototype()?;
        self.objects.create_boxed_primitive(
            ObjectPrimitiveValue::Number(value),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(in crate::runtime::native) fn eval_number_prototype_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_number_prototype_to_string(args.as_slice(), this_value)
    }

    pub(in crate::runtime) fn eval_direct_number_prototype_to_string(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let number = self.number_receiver_value(this_value)?;
        let radix = Self::number_to_string_radix_arg(args.first())?;
        let text = Self::number_to_radix_string(number, radix)?;
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn eval_number_prototype_value_of(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        Self::discard_number_extra_args(args.as_slice());
        self.eval_direct_number_prototype_value_of(this_value)
    }

    pub(in crate::runtime) fn eval_direct_number_prototype_value_of(
        &self,
        this_value: &Value,
    ) -> Result<Value> {
        self.number_receiver_value(this_value).map(Value::Number)
    }

    pub(in crate::runtime) fn number_prototype_property_value(
        &mut self,
        receiver: &Value,
        property: &str,
    ) -> Result<Value> {
        let prototype = self.number_constructor_prototype()?;
        self.get_prototype_property_value_with_receiver(prototype, receiver, property)
    }

    fn number_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
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

    fn number_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.number_constructor_value()? else {
            return Err(crate::error::Error::runtime(
                "Number constructor value is not native",
            ));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(crate::error::Error::runtime(
                "Number prototype is not an object",
            )),
        }
    }

    fn install_number_static_methods(&mut self, constructor: NativeFunctionId) -> Result<()> {
        self.define_number_static_method(
            constructor,
            NUMBER_IS_FINITE_NAME,
            NativeFunctionKind::NumberIsFinite,
        )?;
        self.define_number_static_method(
            constructor,
            NUMBER_IS_INTEGER_NAME,
            NativeFunctionKind::NumberIsInteger,
        )?;
        self.define_number_static_method(
            constructor,
            NUMBER_IS_NAN_NAME,
            NativeFunctionKind::NumberIsNan,
        )?;
        self.define_number_static_method(
            constructor,
            NUMBER_IS_SAFE_INTEGER_NAME,
            NativeFunctionKind::NumberIsSafeInteger,
        )?;
        let parse_float = self.global_function_value(NativeFunctionKind::GlobalParseFloat)?;
        self.define_number_static_function(constructor, GLOBAL_PARSE_FLOAT_NAME, parse_float)?;
        let parse_int = self.global_function_value(NativeFunctionKind::GlobalParseInt)?;
        self.define_number_static_function(constructor, GLOBAL_PARSE_INT_NAME, parse_int)
    }

    fn define_number_static_method(
        &mut self,
        constructor: NativeFunctionId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        self.define_number_static_function(constructor, name, function)
    }

    fn define_number_static_function(
        &mut self,
        constructor: NativeFunctionId,
        name: &str,
        function: Value,
    ) -> Result<()> {
        let key = self.intern_property_key(name)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, function, PropertyEnumerable::No);
        Ok(())
    }

    fn install_number_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        self.define_number_prototype_method(
            prototype,
            NUMBER_PROTOTYPE_TO_LOCALE_STRING_NAME,
            NativeFunctionKind::NumberPrototypeToLocaleString,
        )?;
        self.define_number_prototype_method(
            prototype,
            NUMBER_PROTOTYPE_TO_STRING_NAME,
            NativeFunctionKind::NumberPrototypeToString,
        )?;
        self.define_number_prototype_method(
            prototype,
            NUMBER_PROTOTYPE_VALUE_OF_NAME,
            NativeFunctionKind::NumberPrototypeValueOf,
        )?;
        self.define_number_prototype_method(
            prototype,
            NUMBER_PROTOTYPE_TO_FIXED_NAME,
            NativeFunctionKind::NumberPrototypeToFixed,
        )?;
        self.define_number_prototype_method(
            prototype,
            NUMBER_PROTOTYPE_TO_EXPONENTIAL_NAME,
            NativeFunctionKind::NumberPrototypeToExponential,
        )?;
        self.define_number_prototype_method(
            prototype,
            NUMBER_PROTOTYPE_TO_PRECISION_NAME,
            NativeFunctionKind::NumberPrototypeToPrecision,
        )
    }

    fn define_number_prototype_method(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, name, function)
    }

    fn number_argument_value(value: Option<&Value>) -> f64 {
        let Some(value) = value else {
            return 0.0;
        };
        Self::value_to_number(value)
    }

    pub(super) fn number_receiver_value(&self, value: &Value) -> Result<f64> {
        match value {
            Value::Number(value) => Ok(*value),
            Value::Object(id) => match self.objects.primitive_value(*id)? {
                Some(ObjectPrimitiveValue::Number(value)) => Ok(*value),
                Some(ObjectPrimitiveValue::Bool(_) | ObjectPrimitiveValue::Symbol(_)) | None => {
                    Err(Error::type_error(NUMBER_VALUE_RECEIVER_ERROR))
                }
            },
            _ => Err(Error::type_error(NUMBER_VALUE_RECEIVER_ERROR)),
        }
    }

    pub(in crate::runtime::native) fn eval_number_is_integer(args: RuntimeCallArgs<'_>) -> Value {
        Self::eval_direct_number_is_integer(args.as_slice())
    }

    pub(in crate::runtime) fn eval_direct_number_is_integer(args: &[Value]) -> Value {
        let is_integer = args
            .first()
            .and_then(Value::as_number)
            .is_some_and(Self::is_integer_number);
        Value::Bool(is_integer)
    }

    pub(in crate::runtime::native) fn eval_number_is_safe_integer(
        args: RuntimeCallArgs<'_>,
    ) -> Value {
        Self::eval_direct_number_is_safe_integer(args.as_slice())
    }

    pub(in crate::runtime) fn eval_direct_number_is_safe_integer(args: &[Value]) -> Value {
        let is_safe_integer = args
            .first()
            .and_then(Value::as_number)
            .is_some_and(Self::is_safe_integer_number);
        Value::Bool(is_safe_integer)
    }

    pub(in crate::runtime) fn value_to_number(value: &Value) -> f64 {
        match value {
            Value::Null => 0.0,
            Value::Bool(value) => f64::from(u8::from(*value)),
            Value::Number(value) => *value,
            Value::String(value) => Self::string_to_number(value),
            Value::HeapString(value) => Self::string_to_number(value.as_str()),
            Value::Undefined
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_)
            | Value::Symbol(_)
            | Value::Error(_) => f64::NAN,
        }
    }

    pub(in crate::runtime) fn string_to_number(value: &str) -> f64 {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return 0.0;
        }
        if trimmed == STRING_POSITIVE_INFINITY {
            return f64::INFINITY;
        }
        if trimmed == STRING_NEGATIVE_INFINITY {
            return f64::NEG_INFINITY;
        }
        if let Some(value) = Self::prefixed_integer_to_number(trimmed) {
            return value;
        }
        trimmed.parse::<f64>().map_or(f64::NAN, |number| {
            if number.is_infinite() {
                return f64::NAN;
            }
            number
        })
    }

    fn prefixed_integer_to_number(value: &str) -> Option<f64> {
        let (digits, radix) = if let Some(digits) = value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
        {
            (digits, 16)
        } else if let Some(digits) = value
            .strip_prefix("0b")
            .or_else(|| value.strip_prefix("0B"))
        {
            (digits, 2)
        } else if let Some(digits) = value
            .strip_prefix("0o")
            .or_else(|| value.strip_prefix("0O"))
        {
            (digits, 8)
        } else {
            return None;
        };

        u32::from_str_radix(digits, radix).map(f64::from).ok()
    }

    fn number_to_string_radix_arg(value: Option<&Value>) -> Result<u32> {
        let Some(value) = value else {
            return Ok(10);
        };
        if matches!(value, Value::Undefined) {
            return Ok(10);
        }
        let number = Self::value_to_number(value);
        let Some(radix) = Self::number_finite_integer(number) else {
            return Err(Error::exception(
                ErrorName::RangeError,
                NUMBER_RADIX_RANGE_ERROR,
            ));
        };
        if radix < i64::from(NUMBER_RADIX_MIN) || radix > i64::from(NUMBER_RADIX_MAX) {
            return Err(Error::exception(
                ErrorName::RangeError,
                NUMBER_RADIX_RANGE_ERROR,
            ));
        }
        u32::try_from(radix).map_err(|_| Error::limit("number radix exceeded supported range"))
    }

    fn number_to_radix_string(number: f64, radix: u32) -> Result<String> {
        if radix == 10 || !number.is_finite() || number.fract() != 0.0 {
            return Ok(Value::Number(number).to_string());
        }
        if number == 0.0 {
            return Ok("0".to_owned());
        }
        let integer = Self::number_finite_integer(number)
            .ok_or_else(|| Error::limit("number radix conversion requires a finite integer"))?;
        let magnitude = integer.unsigned_abs();
        let mut output = Self::unsigned_integer_to_radix_string(magnitude, radix)?;
        if integer.is_negative() {
            output.insert(0, '-');
        }
        Ok(output)
    }

    fn unsigned_integer_to_radix_string(mut value: u64, radix: u32) -> Result<String> {
        let mut digits = Vec::new();
        let radix = u64::from(radix);
        while value > 0 {
            let digit = value % radix;
            let digit = u32::try_from(digit)
                .map_err(|_| Error::limit("number radix digit exceeded supported range"))?;
            let ch = char::from_digit(digit, NUMBER_RADIX_MAX)
                .ok_or_else(|| Error::limit("number radix digit is not representable"))?;
            digits.push(ch);
            value /= radix;
        }
        Ok(digits.into_iter().rev().collect())
    }

    fn number_finite_integer(number: f64) -> Option<i64> {
        if !number.is_finite() {
            return None;
        }
        let integer = if number.is_sign_negative() {
            number.ceil()
        } else {
            number.floor()
        };
        format!("{integer:.0}").parse::<i64>().ok()
    }

    const fn is_integer_number(number: f64) -> bool {
        number.is_finite() && number.fract() == 0.0
    }

    const fn is_safe_integer_number(number: f64) -> bool {
        Self::is_integer_number(number)
            && number >= NUMBER_MIN_SAFE_INTEGER
            && number <= NUMBER_MAX_SAFE_INTEGER
    }

    const fn discard_number_extra_args(_args: &[Value]) {}
}
