use crate::{
    ast::Expr,
    error::Result,
    runtime::Context,
    runtime_object::PropertyEnumerable,
    value::{NativeFunctionId, ObjectId, Value},
};

use super::{NUMBER_NAME, NativeFunction, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY};

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
        let id = NativeFunctionId::new(self.native_functions.len());
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.number_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        self.native_functions
            .push(NativeFunction::new(NativeFunctionKind::Number, prototype));
        self.insert_global_builtin(NUMBER_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(super) fn eval_number_constructor(&mut self, args: &[Expr]) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        Ok(Value::Number(Self::number_argument_value(values.first())))
    }

    pub(super) fn construct_number_object(&mut self, args: &[Expr]) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let _number_value = Self::number_argument_value(values.first());
        let prototype = self.number_constructor_prototype()?;
        self.objects.create_with_prototype(
            Some(prototype),
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn number_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        self.objects.create_with_prototype_property(
            None,
            OBJECT_CONSTRUCTOR_PROPERTY.to_owned(),
            constructor,
            PropertyEnumerable::No,
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
        Self::to_number(value)
    }

    fn to_number(value: &Value) -> f64 {
        match value {
            Value::Null => 0.0,
            Value::Bool(value) => f64::from(u8::from(*value)),
            Value::Number(value) => *value,
            Value::String(value) => Self::string_to_number(value),
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
