use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

use super::NativeFunctionKind;

const PROXY_TARGET_NOT_OBJECT_ERROR: &str = "Proxy target must be an object";
const PROXY_HANDLER_NOT_OBJECT_ERROR: &str = "Proxy handler must be an object";
const PROXY_REQUIRES_NEW_ERROR: &str = "Constructor Proxy requires 'new'";

impl Context {
    pub(in crate::runtime) fn proxy_constructor_value(&mut self) -> Result<Value> {
        self.global_function_value(NativeFunctionKind::Proxy)
    }

    /// Spec `Proxy(target, handler)` / `ProxyCreate`. Both operands must be
    /// objects (ordinary objects or callables). Calling `Proxy` without `new`
    /// is a `TypeError`.
    pub(in crate::runtime) fn eval_proxy_call(&mut self, _args: RuntimeCallArgs<'_>) -> Result<Value> {
        Err(Error::type_error(PROXY_REQUIRES_NEW_ERROR))
    }

    pub(in crate::runtime) fn construct_proxy_object(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let slice = args.as_slice();
        let target = Self::argument_or_undefined(slice.first());
        let handler = Self::argument_or_undefined(slice.get(1));
        if !Self::is_object_like(&target) {
            return Err(Error::type_error(PROXY_TARGET_NOT_OBJECT_ERROR));
        }
        if !Self::is_object_like(&handler) {
            return Err(Error::type_error(PROXY_HANDLER_NOT_OBJECT_ERROR));
        }
        self.objects
            .create_proxy_object(target, handler, self.limits.max_objects)
    }

    /// A value counts as an object for Proxy internal slots when it is an
    /// ordinary object or any callable object.
    pub(in crate::runtime) const fn is_object_like(value: &Value) -> bool {
        matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
                | Value::Error(_)
        )
    }
}
