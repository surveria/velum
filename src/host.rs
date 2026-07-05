use std::{fmt, rc::Rc};

use crate::{
    ast::{DeclKind, Expr},
    error::{Error, Result},
    runtime::Context,
    value::{HostFunctionId, Value},
};

const EMPTY_HOST_FUNCTION_NAME_ERROR: &str = "host function name must not be empty";
const HOST_FUNCTION_HANDLE_RETURN_ERROR: &str =
    "host functions cannot return VM-owned handles in the skeleton API";

type HostCallback = dyn for<'call> Fn(HostCall<'call>) -> Result<Value>;

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
        match self.required_value(index, label)? {
            Value::Number(value) => Ok(*value),
            value => Err(Self::type_error(index, label, "number", value)),
        }
    }

    /// # Errors
    /// Fails when the argument is missing or is not a JavaScript string.
    pub fn string(self, index: usize, label: &str) -> Result<&'call str> {
        match self.required_value(index, label)? {
            Value::String(value) => Ok(value.as_str()),
            value => Err(Self::type_error(index, label, "string", value)),
        }
    }

    /// # Errors
    /// Fails when the argument is missing or is not a JavaScript boolean.
    pub fn boolean(self, index: usize, label: &str) -> Result<bool> {
        match self.required_value(index, label)? {
            Value::Bool(value) => Ok(*value),
            value => Err(Self::type_error(index, label, "boolean", value)),
        }
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
        let name = name.into();
        if name.is_empty() {
            return Err(Error::runtime(EMPTY_HOST_FUNCTION_NAME_ERROR));
        }
        self.check_string_len(&name)?;

        let id = HostFunctionId::new(self.host_functions.len());
        self.host_functions
            .push(HostFunction::new(name.clone(), callback));
        let result = self.define(&name, Value::HostFunction(id), DeclKind::Const);
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
        args: &[Expr],
    ) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let function = self.host_function(id)?.clone();
        self.checked_host_return_value(function.call(&values)?)
    }

    fn host_function(&self, id: HostFunctionId) -> Result<&HostFunction> {
        self.host_functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("host function id is not defined"))
    }

    fn checked_host_return_value(&self, value: Value) -> Result<Value> {
        match value {
            Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Object(_) => Err(Error::runtime(HOST_FUNCTION_HANDLE_RETURN_ERROR)),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Error(_) => self.checked_value(value),
        }
    }
}
