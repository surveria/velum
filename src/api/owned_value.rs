use crate::{
    compiled_script::CompiledScript,
    error::{Error, Result},
    runtime::Context,
    value::{JsBigInt, Value},
};

const VM_LOCAL_VALUE_ERROR: &str = "VM-local value cannot be converted to OwnedValue";
const ILL_FORMED_STRING_ERROR: &str =
    "JavaScript string containing lone surrogates cannot be converted to UTF-8 OwnedValue";

/// A JavaScript primitive that owns all of its data and can cross VM
/// boundaries without retaining a VM identity or root.
#[derive(Clone, Debug, PartialEq)]
pub enum OwnedValue {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    BigInt(JsBigInt),
    String(String),
}

impl OwnedValue {
    /// Returns the corresponding ECMAScript type name.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Undefined => "undefined",
            Self::Null => "object",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::BigInt(_) => "bigint",
            Self::String(_) => "string",
        }
    }
}

impl TryFrom<&Value> for OwnedValue {
    type Error = Error;

    fn try_from(value: &Value) -> Result<Self> {
        match value {
            Value::Undefined => Ok(Self::Undefined),
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(*value)),
            Value::Number(value) => Ok(Self::Number(*value)),
            Value::BigInt(value) => Ok(Self::BigInt(value.clone())),
            Value::String(value) => value
                .as_utf8()
                .map(str::to_owned)
                .map(Self::String)
                .ok_or_else(|| Error::runtime(ILL_FORMED_STRING_ERROR)),
            Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_) => Err(Error::runtime(VM_LOCAL_VALUE_ERROR)),
        }
    }
}

impl TryFrom<Value> for OwnedValue {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self> {
        match value {
            Value::Undefined => Ok(Self::Undefined),
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(value)),
            Value::Number(value) => Ok(Self::Number(value)),
            Value::BigInt(value) => Ok(Self::BigInt(value)),
            Value::String(value) => value
                .into_utf8()
                .map(Self::String)
                .ok_or_else(|| Error::runtime(ILL_FORMED_STRING_ERROR)),
            Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_) => Err(Error::runtime(VM_LOCAL_VALUE_ERROR)),
        }
    }
}

impl From<OwnedValue> for Value {
    fn from(value: OwnedValue) -> Self {
        match value {
            OwnedValue::Undefined => Self::Undefined,
            OwnedValue::Null => Self::Null,
            OwnedValue::Bool(value) => Self::Bool(value),
            OwnedValue::Number(value) => Self::Number(value),
            OwnedValue::BigInt(value) => Self::BigInt(value),
            OwnedValue::String(value) => Self::from(value),
        }
    }
}

impl Context {
    /// Evaluates source and copies its result into a VM-independent primitive.
    ///
    /// # Errors
    /// Fails when evaluation fails or the result is a Symbol, object, or
    /// function that requires a VM-local handle.
    pub fn eval_owned(&mut self, source: &str) -> Result<OwnedValue> {
        self.eval(source).and_then(OwnedValue::try_from)
    }

    /// Evaluates compiled source and copies its result into a VM-independent
    /// primitive.
    ///
    /// # Errors
    /// Fails when evaluation fails or the result is a Symbol, object, or
    /// function that requires a VM-local handle.
    pub fn eval_compiled_owned(&mut self, script: &CompiledScript) -> Result<OwnedValue> {
        self.eval_compiled(script).and_then(OwnedValue::try_from)
    }
}
