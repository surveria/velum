use crate::{
    ast::Expr,
    error::Result,
    runtime::Context,
    runtime_object::{ObjectPropertyInit, PropertyEnumerable},
    value::{ObjectId, Value},
};

use super::{NUMBER_NAME, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY};

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
const STRING_NEGATIVE_INFINITY: &str = "-Infinity";
const STRING_POSITIVE_INFINITY: &str = "Infinity";

pub(super) fn number_intrinsic_property(property: &str) -> Option<Value> {
    match property {
        NUMBER_EPSILON_PROPERTY => Some(Value::Number(f64::EPSILON)),
        NUMBER_MAX_SAFE_INTEGER_PROPERTY => Some(Value::Number(NUMBER_MAX_SAFE_INTEGER)),
        NUMBER_MAX_VALUE_PROPERTY => Some(Value::Number(f64::MAX)),
        NUMBER_MIN_SAFE_INTEGER_PROPERTY => Some(Value::Number(NUMBER_MIN_SAFE_INTEGER)),
        NUMBER_MIN_VALUE_PROPERTY => Some(Value::Number(f64::MIN_POSITIVE)),
        NUMBER_NAN_PROPERTY => Some(Value::Number(f64::NAN)),
        NUMBER_NEGATIVE_INFINITY_PROPERTY => Some(Value::Number(f64::NEG_INFINITY)),
        NUMBER_POSITIVE_INFINITY_PROPERTY => Some(Value::Number(f64::INFINITY)),
        _ => None,
    }
}

impl Context {
    pub(super) fn number_constructor_value(&mut self) -> Result<Value> {
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
        self.insert_global_builtin(NUMBER_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(super) fn eval_number_constructor(&mut self, args: &[Expr]) -> Result<Value> {
        let value = self.eval_native_unary_argument_value(args)?;
        Ok(Value::Number(Self::number_argument_value(value.as_ref())))
    }

    pub(super) fn construct_number_object(&mut self, args: &[Expr]) -> Result<Value> {
        let value = self.eval_native_unary_argument_value(args)?;
        let _number_value = Self::number_argument_value(value.as_ref());
        let prototype = self.number_constructor_prototype()?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
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

    fn number_argument_value(value: Option<&Value>) -> f64 {
        let Some(value) = value else {
            return 0.0;
        };
        Self::value_to_number(value)
    }

    pub(super) fn value_to_number(value: &Value) -> f64 {
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
            | Value::Error(_) => f64::NAN,
        }
    }

    fn string_to_number(value: &str) -> f64 {
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
}
