use std::{fmt, rc::Rc};

use crate::{
    ast::DeclKind,
    error::{Error, Result},
    runtime::Context,
    runtime_call_args::RuntimeCallArgs,
    value::{HostFunctionId, Value},
};

const EMPTY_HOST_FUNCTION_NAME_ERROR: &str = "host function name must not be empty";
const HOST_FUNCTION_HANDLE_RETURN_ERROR: &str =
    "host functions cannot return VM-owned handles in the skeleton API";

type HostCallback = dyn for<'call> Fn(HostCall<'call>) -> Result<Value>;

pub trait IntoJsValue {
    /// # Errors
    /// Fails when conversion cannot produce a JavaScript value.
    fn into_js_value(self) -> Result<Value>;
}

impl IntoJsValue for Value {
    fn into_js_value(self) -> Result<Value> {
        Ok(self)
    }
}

impl IntoJsValue for () {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::Undefined)
    }
}

impl IntoJsValue for bool {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::Bool(self))
    }
}

impl IntoJsValue for f64 {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::Number(self))
    }
}

impl IntoJsValue for String {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::String(self))
    }
}

impl IntoJsValue for &str {
    fn into_js_value(self) -> Result<Value> {
        Ok(Value::String(self.to_owned()))
    }
}

pub trait FromJsValue<'value>: Sized {
    const EXPECTED_TYPE: &'static str;

    fn from_js_value(value: &'value Value) -> Option<Self>;
}

impl FromJsValue<'_> for bool {
    const EXPECTED_TYPE: &'static str = "boolean";

    fn from_js_value(value: &Value) -> Option<Self> {
        match value {
            Value::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

impl FromJsValue<'_> for f64 {
    const EXPECTED_TYPE: &'static str = "number";

    fn from_js_value(value: &Value) -> Option<Self> {
        match value {
            Value::Number(value) => Some(*value),
            _ => None,
        }
    }
}

impl<'value> FromJsValue<'value> for &'value str {
    const EXPECTED_TYPE: &'static str = "string";

    fn from_js_value(value: &'value Value) -> Option<Self> {
        match value {
            Value::String(value) => Some(value.as_str()),
            Value::HeapString(value) => Some(value.as_str()),
            _ => None,
        }
    }
}

impl FromJsValue<'_> for String {
    const EXPECTED_TYPE: &'static str = "string";

    fn from_js_value(value: &Value) -> Option<Self> {
        match value {
            Value::String(value) => Some(value.clone()),
            Value::HeapString(value) => Some(value.as_str().to_owned()),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct HostFunction {
    name: String,
    callback: Rc<HostCallback>,
}

impl HostFunction {
    fn new<F>(name: String, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Value> + 'static,
    {
        Self {
            name,
            callback: Rc::new(callback),
        }
    }

    fn new_typed<F, R>(name: String, callback: F) -> Self
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        Self::new(name, move |call| callback(call)?.into_js_value())
    }

    fn call(&self, args: &[Value]) -> Result<Value> {
        let call = HostCall {
            function_name: self.name.as_str(),
            args,
        };
        (self.callback)(call).map_err(|error| error.with_context(self.context_message()))
    }

    fn context_message(&self) -> String {
        format!("host function '{}'", self.name)
    }
}

impl fmt::Debug for HostFunction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostFunction")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HostCall<'call> {
    function_name: &'call str,
    args: &'call [Value],
}

impl<'call> HostCall<'call> {
    #[must_use]
    pub const fn function_name(self) -> &'call str {
        self.function_name
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.args.len()
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.args.is_empty()
    }

    #[must_use]
    pub fn value(self, index: usize) -> Option<&'call Value> {
        self.args.get(index)
    }

    /// # Errors
    /// Fails when the argument is missing.
    pub fn required_value(self, index: usize, label: &str) -> Result<&'call Value> {
        let Some(value) = self.value(index) else {
            return Err(Self::missing_argument(index, label));
        };
        Ok(value)
    }

    /// # Errors
    /// Fails when the argument is missing or is not a JavaScript number.
    pub fn number(self, index: usize, label: &str) -> Result<f64> {
        self.argument(index, label)
    }

    /// # Errors
    /// Fails when the argument is missing or is not a JavaScript string.
    pub fn string(self, index: usize, label: &str) -> Result<&'call str> {
        self.argument(index, label)
    }

    /// # Errors
    /// Fails when the argument is missing or is not a JavaScript boolean.
    pub fn boolean(self, index: usize, label: &str) -> Result<bool> {
        self.argument(index, label)
    }

    /// # Errors
    /// Fails when the argument is missing or cannot be converted into `T`.
    pub fn argument<T>(self, index: usize, label: &str) -> Result<T>
    where
        T: FromJsValue<'call>,
    {
        let value = self.required_value(index, label)?;
        let Some(converted) = T::from_js_value(value) else {
            return Err(Self::type_error(index, label, T::EXPECTED_TYPE, value));
        };
        Ok(converted)
    }

    fn missing_argument(index: usize, label: &str) -> Error {
        Error::runtime(format!("missing argument '{label}' at index {index}"))
    }

    fn type_error(index: usize, label: &str, expected: &str, actual: &Value) -> Error {
        Error::runtime(format!(
            "argument '{label}' at index {index} expected {expected}, got {}",
            actual.type_name()
        ))
    }
}

impl Context {
    /// # Errors
    /// Fails when the name is empty, exceeds string limits, duplicates an
    /// existing binding, or would exceed the binding limit.
    pub fn register_host_function<F>(&mut self, name: impl Into<String>, callback: F) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<Value> + 'static,
    {
        self.register_host_callback(name.into(), HostFunction::new, callback)
    }

    /// # Errors
    /// Fails when the name is empty, exceeds string limits, duplicates an
    /// existing binding, or would exceed the binding limit.
    pub fn register_host_function_typed<F, R>(
        &mut self,
        name: impl Into<String>,
        callback: F,
    ) -> Result<()>
    where
        F: for<'call> Fn(HostCall<'call>) -> Result<R> + 'static,
        R: IntoJsValue + 'static,
    {
        self.register_host_callback(name.into(), HostFunction::new_typed, callback)
    }

    fn register_host_callback<F, C>(
        &mut self,
        name: String,
        create_host_function: C,
        callback: F,
    ) -> Result<()>
    where
        C: FnOnce(String, F) -> HostFunction,
    {
        if name.is_empty() {
            return Err(Error::runtime(EMPTY_HOST_FUNCTION_NAME_ERROR));
        }
        self.check_string_len(&name)?;

        let id = HostFunctionId::new(self.host_functions.len());
        let binding_name = name.clone();
        self.host_functions
            .push(create_host_function(name, callback));
        let result = self.define(&binding_name, Value::HostFunction(id), DeclKind::Const);
        if let Err(error) = result {
            let removed = self.host_functions.pop();
            if removed.is_none() {
                return Err(Error::runtime("host function rollback failed"));
            }
            return Err(error);
        }
        Ok(())
    }

    pub(crate) fn eval_host_function(
        &mut self,
        id: HostFunctionId,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.evaluate();
        let function = self.host_function(id)?.clone();
        self.checked_host_return_value(function.call(&values)?)
    }

    fn host_function(&self, id: HostFunctionId) -> Result<&HostFunction> {
        self.host_functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("host function id is not defined"))
    }

    fn checked_host_return_value(&mut self, value: Value) -> Result<Value> {
        match value {
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_) => Err(Error::runtime(HOST_FUNCTION_HANDLE_RETURN_ERROR)),
            Value::String(value) => self.heap_string_value(&value),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::HeapString(_)
            | Value::Error(_) => self.runtime_value(value),
        }
    }
}
