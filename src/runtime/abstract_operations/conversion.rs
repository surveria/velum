use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{PropertyKey, PropertyLookup},
        property::DynamicPropertyKey,
    },
    value::{ErrorName, JsBigInt, Value, format_ecmascript_number},
};

const CANNOT_CONVERT_OBJECT_ERROR: &str = "Cannot convert object to primitive value";
const CANNOT_CONVERT_SYMBOL_ERROR: &str = "Cannot convert a Symbol value to a number";
const CANNOT_CONVERT_BIGINT_ERROR: &str = "Cannot convert a BigInt value to a number";
const CANNOT_CONVERT_TO_BIGINT_ERROR: &str = "Cannot convert value to a BigInt";
const CANNOT_CONVERT_SYMBOL_TO_STRING_ERROR: &str = "Cannot convert a Symbol value to a string";
const SYMBOL_TO_PRIMITIVE_NAME: &str = "[Symbol.toPrimitive]";
const SYMBOL_TO_PRIMITIVE_PROPERTY: &str = "toPrimitive";
const TO_STRING_PROPERTY: &str = "toString";
const VALUE_OF_PROPERTY: &str = "valueOf";
const STRING_NEGATIVE_INFINITY: &str = "-Infinity";
const STRING_PLUS_INFINITY: &str = "+Infinity";
const STRING_POSITIVE_INFINITY: &str = "Infinity";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_SAFE_INTEGER_NUMBER: f64 = 9_007_199_254_740_991.0;
const TO_INDEX_RANGE_ERROR: &str = "Index must be between 0 and Number.MAX_SAFE_INTEGER";

/// Preferred result type supplied to ECMAScript `ToPrimitive`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) enum PreferredType {
    Default,
    Number,
    String,
}

#[derive(Clone, Debug)]
pub(in crate::runtime) enum NumericValue {
    Number(f64),
    BigInt(JsBigInt),
}

impl PreferredType {
    const fn hint(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Number => "number",
            Self::String => "string",
        }
    }

    const fn ordinary_method_names(self) -> [&'static str; 2] {
        match self {
            Self::String => [TO_STRING_PROPERTY, VALUE_OF_PROPERTY],
            Self::Default | Self::Number => [VALUE_OF_PROPERTY, TO_STRING_PROPERTY],
        }
    }
}

impl Context {
    /// ECMAScript `ToPrimitive`, including `@@toPrimitive` lookup and calls.
    // The specification name is intentional; conversion can invoke JavaScript.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_primitive(
        &mut self,
        value: &Value,
        preferred_type: PreferredType,
    ) -> Result<Value> {
        if is_primitive(value) {
            return Ok(value.clone());
        }

        if let Some(exotic) = self.get_to_primitive_method(value)? {
            let hint = self.heap_string_value(preferred_type.hint())?;
            let result = self.call_value(&exotic, &[hint], value.clone())?;
            if is_primitive(&result) {
                return Ok(result);
            }
            return Err(Error::type_error(CANNOT_CONVERT_OBJECT_ERROR));
        }

        self.ordinary_to_primitive(value, preferred_type)
    }

    /// ECMAScript `OrdinaryToPrimitive` with observable method ordering.
    pub(in crate::runtime) fn ordinary_to_primitive(
        &mut self,
        value: &Value,
        preferred_type: PreferredType,
    ) -> Result<Value> {
        if is_primitive(value) {
            return Err(Error::type_error(
                "OrdinaryToPrimitive requires an object value",
            ));
        }
        for method_name in preferred_type.ordinary_method_names() {
            let method = self.get_named(value, method_name)?;
            if !self.semantic_is_callable(&method)? {
                continue;
            }
            let result = self.call_value(&method, &[], value.clone())?;
            if is_primitive(&result) {
                return Ok(result);
            }
        }
        Err(Error::type_error(CANNOT_CONVERT_OBJECT_ERROR))
    }

    /// ECMAScript `ToNumber` for the runtime's current value domain.
    // The specification name is intentional; conversion can invoke JavaScript.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_number(&mut self, value: &Value) -> Result<f64> {
        let primitive = self.to_primitive(value, PreferredType::Number)?;
        to_number_primitive(&primitive)
    }

    /// ECMAScript `ToNumeric`, preserving `BigInt` instead of routing it through
    /// the binary64 Number domain.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_numeric(&mut self, value: &Value) -> Result<NumericValue> {
        let primitive = self.to_primitive(value, PreferredType::Number)?;
        if let Value::BigInt(value) = primitive {
            return Ok(NumericValue::BigInt(value));
        }
        to_number_primitive(&primitive).map(NumericValue::Number)
    }

    /// ECMAScript `ToBigInt` used by BigInt-only operators and APIs.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_bigint(&mut self, value: &Value) -> Result<JsBigInt> {
        let primitive = self.to_primitive(value, PreferredType::Number)?;
        to_bigint_primitive(&primitive)
    }

    /// ECMAScript `ToIntegerOrInfinity`, including observable number conversion.
    // The specification name is intentional; conversion can invoke JavaScript.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_integer_or_infinity(&mut self, value: &Value) -> Result<f64> {
        self.to_number(value).map(integer_or_infinity_from_number)
    }

    /// ECMAScript `ToLength` in its full specification range.
    // The specification name is intentional; conversion can invoke JavaScript.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_length(&mut self, value: &Value) -> Result<u64> {
        let integer = self.to_integer_or_infinity(value)?;
        length_from_integer(integer)
    }

    /// ECMAScript `ToIndex`, with `undefined` defaulting to zero.
    // The specification name is intentional; conversion can invoke JavaScript.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_index(&mut self, value: Option<&Value>) -> Result<u64> {
        let Some(value) = value else {
            return Ok(0);
        };
        if matches!(value, Value::Undefined) {
            return Ok(0);
        }
        let integer = self.to_integer_or_infinity(value)?;
        if !(0.0..=MAX_SAFE_INTEGER_NUMBER).contains(&integer) {
            return Err(Error::exception(
                ErrorName::RangeError,
                TO_INDEX_RANGE_ERROR,
            ));
        }
        length_from_integer(integer)
    }

    pub(in crate::runtime) fn length_to_usize(length: u64, error: &str) -> Result<usize> {
        usize::try_from(length).map_err(|_| Error::limit(error))
    }

    pub(in crate::runtime) fn finite_nonnegative_integer_to_usize(
        integer: f64,
        error: &str,
    ) -> Result<usize> {
        if integer == 0.0 {
            return Ok(0);
        }
        if !integer.is_finite() || integer < 0.0 || integer.fract() != 0.0 {
            return Err(Error::limit(error));
        }
        format!("{integer:.0}")
            .parse::<usize>()
            .map_err(|_| Error::limit(error))
    }

    pub(in crate::runtime) fn usize_to_number(value: usize, error: &str) -> Result<f64> {
        let value = u64::try_from(value).map_err(|_| Error::limit(error))?;
        if value > MAX_SAFE_INTEGER {
            return Err(Error::limit(error));
        }
        value
            .to_string()
            .parse::<f64>()
            .map_err(|_| Error::limit(error))
    }

    /// ECMAScript `ToString`, including observable object conversion.
    // The specification name is intentional; conversion can invoke JavaScript.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_string(&mut self, value: &Value) -> Result<String> {
        let primitive = self.to_primitive(value, PreferredType::String)?;
        let text = to_string_primitive(&primitive)?;
        self.check_string_len(&text)?;
        Ok(text)
    }

    /// ECMAScript `ToString`, retaining the exact UTF-16 code-unit sequence.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_utf16_string(&mut self, value: &Value) -> Result<Vec<u16>> {
        let primitive = self.to_primitive(value, PreferredType::String)?;
        let units = if let Some(units) = primitive.string_units() {
            units.into_owned()
        } else {
            to_string_primitive(&primitive)?
                .encode_utf16()
                .collect::<Vec<_>>()
        };
        self.check_utf16_string_len(&units)?;
        Ok(units)
    }

    /// ECMAScript `ToPropertyKey`, preserving Symbol identity.
    // The specification name is intentional; conversion can invoke JavaScript.
    #[allow(clippy::wrong_self_convention)]
    pub(in crate::runtime) fn to_property_key(
        &mut self,
        value: &Value,
    ) -> Result<DynamicPropertyKey> {
        let primitive = self.to_primitive(value, PreferredType::String)?;
        if let Value::Symbol(symbol) = primitive {
            let name = symbol.display_name();
            self.check_string_len(&name)?;
            return Ok(DynamicPropertyKey::new(
                name,
                Some(PropertyKey::symbol(symbol.id())),
            ));
        }
        let name = to_string_primitive(&primitive)?;
        self.check_string_len(&name)?;
        let key = self.known_property_key(&name);
        Ok(DynamicPropertyKey::new(name, key))
    }

    fn get_to_primitive_method(&mut self, value: &Value) -> Result<Option<Value>> {
        let constructor = self.symbol_constructor_value()?;
        let symbol = self.get_named(&constructor, SYMBOL_TO_PRIMITIVE_PROPERTY)?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("Symbol.toPrimitive is not a symbol"));
        };
        let lookup =
            PropertyLookup::from_key(SYMBOL_TO_PRIMITIVE_NAME, PropertyKey::symbol(symbol.id()));
        self.get_method(value, lookup)
    }
}

pub(in crate::runtime) fn integer_or_infinity_from_number(number: f64) -> f64 {
    if number.is_nan() || number == 0.0 {
        return 0.0;
    }
    if number.is_infinite() {
        return number;
    }
    number.trunc()
}

fn length_from_integer(integer: f64) -> Result<u64> {
    if integer <= 0.0 {
        return Ok(0);
    }
    if integer >= MAX_SAFE_INTEGER_NUMBER {
        return Ok(MAX_SAFE_INTEGER);
    }
    format!("{integer:.0}")
        .parse::<u64>()
        .map_err(|_| Error::limit("length conversion exceeded supported range"))
}

pub(in crate::runtime) const fn is_primitive(value: &Value) -> bool {
    matches!(
        value,
        Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
    )
}

pub(in crate::runtime) fn to_number_primitive(value: &Value) -> Result<f64> {
    match value {
        Value::Undefined => Ok(f64::NAN),
        Value::Null => Ok(0.0),
        Value::Bool(value) => Ok(f64::from(u8::from(*value))),
        Value::Number(value) => Ok(*value),
        Value::BigInt(_) => Err(Error::type_error(CANNOT_CONVERT_BIGINT_ERROR)),
        Value::String(_) | Value::HeapString(_) => value
            .string_text()
            .map(string_to_number)
            .ok_or_else(|| Error::runtime("string value lost its text")),
        Value::Symbol(_) => Err(Error::type_error(CANNOT_CONVERT_SYMBOL_ERROR)),
        Value::Function(_)
        | Value::NativeFunction(_)
        | Value::HostFunction(_)
        | Value::Object(_) => Err(Error::runtime(
            "ToNumber received a non-primitive after ToPrimitive",
        )),
    }
}

pub(in crate::runtime) fn to_bigint_primitive(value: &Value) -> Result<JsBigInt> {
    match value {
        Value::BigInt(value) => Ok(value.clone()),
        Value::Bool(value) => Ok(JsBigInt::from_u64(u64::from(*value))),
        Value::String(_) | Value::HeapString(_) => value
            .string_text()
            .and_then(JsBigInt::parse_string)
            .ok_or_else(|| {
                Error::exception(ErrorName::SyntaxError, CANNOT_CONVERT_TO_BIGINT_ERROR)
            }),
        Value::Undefined
        | Value::Null
        | Value::Number(_)
        | Value::Symbol(_)
        | Value::Function(_)
        | Value::NativeFunction(_)
        | Value::HostFunction(_)
        | Value::Object(_) => Err(Error::type_error(CANNOT_CONVERT_TO_BIGINT_ERROR)),
    }
}

/// ECMAScript `ToBoolean` for the runtime's complete value domain.
pub(in crate::runtime) fn to_boolean(value: &Value) -> bool {
    match value {
        Value::Undefined | Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => *value != 0.0 && !value.is_nan(),
        Value::BigInt(value) => !value.is_zero(),
        Value::String(_) | Value::HeapString(_) => {
            value.string_text().is_some_and(|value| !value.is_empty())
        }
        Value::Symbol(_)
        | Value::Function(_)
        | Value::NativeFunction(_)
        | Value::HostFunction(_)
        | Value::Object(_) => true,
    }
}

pub(in crate::runtime) fn to_string_primitive(value: &Value) -> Result<String> {
    match value {
        Value::Undefined => Ok("undefined".to_owned()),
        Value::Null => Ok("null".to_owned()),
        Value::Bool(value) => Ok(if *value { "true" } else { "false" }.to_owned()),
        Value::Number(value) => Ok(format_ecmascript_number(*value)),
        Value::BigInt(value) => Ok(value.to_string()),
        Value::String(_) | Value::HeapString(_) => value
            .string_text()
            .map(str::to_owned)
            .ok_or_else(|| Error::runtime("string value lost its text")),
        Value::Symbol(_) => Err(Error::type_error(CANNOT_CONVERT_SYMBOL_TO_STRING_ERROR)),
        Value::Function(_)
        | Value::NativeFunction(_)
        | Value::HostFunction(_)
        | Value::Object(_) => Err(Error::runtime(
            "ToString received a non-primitive after ToPrimitive",
        )),
    }
}

pub(in crate::runtime) fn string_to_number(value: &str) -> f64 {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return 0.0;
    }
    if matches!(trimmed, STRING_POSITIVE_INFINITY | STRING_PLUS_INFINITY) {
        return f64::INFINITY;
    }
    if trimmed == STRING_NEGATIVE_INFINITY {
        return f64::NEG_INFINITY;
    }
    if let Some(value) = prefixed_integer_to_number(trimmed) {
        return value;
    }
    trimmed.parse::<f64>().map_or(f64::NAN, |number| {
        if number.is_infinite() && !trimmed.bytes().any(|byte| byte.is_ascii_digit()) {
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
