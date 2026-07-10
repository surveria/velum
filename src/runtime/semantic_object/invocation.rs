use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, control::Completion, native::NativeFunctionKind},
    value::Value,
};

impl Context {
    pub(in crate::runtime) fn semantic_type_name(&self, value: &Value) -> Result<&'static str> {
        if self.semantic_is_callable(value)? {
            return Ok("function");
        }
        Ok(value.type_name())
    }

    /// Spec-style `IsCallable` over the checked semantic object boundary.
    pub(in crate::runtime) fn semantic_is_callable(&self, value: &Value) -> Result<bool> {
        let Some(object) = self.semantic_object_ref(value)? else {
            return Ok(false);
        };
        match object.value {
            Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_) => Ok(true),
            Value::Object(id) => self.objects.proxy_callability(*id),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => Ok(false),
        }
    }

    /// Optional semantic `[[Call]]`, preserving JavaScript completion values.
    pub(in crate::runtime) fn semantic_call(
        &mut self,
        callee: &Value,
        args: &[Value],
        this_value: Value,
    ) -> Result<Completion> {
        let Some(object) = self.semantic_object_ref(callee)? else {
            return Err(Self::not_callable_error(callee));
        };
        match object.value {
            Value::Function(id) => self.eval_function_call_completion_with_this(
                *id,
                RuntimeCallArgs::values(args),
                this_value,
            ),
            Value::NativeFunction(id) => {
                let kind = self.native_function(*id)?.kind();
                self.eval_direct_or_generic_native_function_kind(kind, args, &this_value)
                    .map(Completion::Normal)
            }
            Value::HostFunction(id) => self
                .eval_host_function(*id, RuntimeCallArgs::values(args))
                .map(Completion::Normal),
            Value::Object(id) if self.objects.proxy_callability(*id)? => self
                .proxy_apply(*id, args, this_value)
                .map(Completion::Normal),
            Value::Object(_)
            | Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => Err(Self::not_callable_error(callee)),
        }
    }

    /// Spec-style `IsConstructor` over functions, native payloads, bound
    /// functions, and Proxy exotic objects.
    pub(in crate::runtime) fn semantic_is_constructor(&self, value: &Value) -> Result<bool> {
        let Some(object) = self.semantic_object_ref(value)? else {
            return Ok(false);
        };
        match object.value {
            Value::Function(id) => self.is_function_constructable(*id),
            Value::NativeFunction(id) => {
                let kind = self.native_function(*id)?.kind();
                if let NativeFunctionKind::BoundFunction(bound) = kind {
                    let target = self.bound_function_target(bound)?;
                    return self.semantic_is_constructor(&target);
                }
                Ok(kind.is_constructable())
            }
            Value::Object(id) => self.objects.proxy_constructability(*id),
            Value::HostFunction(_)
            | Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => Ok(false),
        }
    }

    /// Optional semantic `[[Construct]]` with an explicit `newTarget`.
    pub(in crate::runtime) fn semantic_construct(
        &mut self,
        constructor: &Value,
        args: &[Value],
        new_target: Value,
    ) -> Result<Value> {
        if !self.semantic_is_constructor(&new_target)? {
            return Err(Self::not_constructor_error(&new_target));
        }
        let Some(object) = self.semantic_object_ref(constructor)? else {
            return Err(Self::not_constructor_error(constructor));
        };
        match object.value {
            Value::Function(id) if self.is_function_constructable(*id)? => {
                self.eval_function_constructor_value(*id, RuntimeCallArgs::values(args), new_target)
            }
            Value::NativeFunction(id) => {
                let kind = self.native_function(*id)?.kind();
                if let NativeFunctionKind::BoundFunction(bound) = kind {
                    return self.eval_bound_function_construct(
                        bound,
                        args,
                        constructor,
                        new_target,
                    );
                }
                if let NativeFunctionKind::ErrorConstructor(name) = kind {
                    let prototype = self.constructor_instance_prototype(&new_target)?;
                    return self
                        .eval_direct_error_constructor_with_prototype(name, args, prototype);
                }
                self.construct_native_function_kind(kind, RuntimeCallArgs::values(args))
            }
            Value::Object(id) if self.objects.proxy_constructability(*id)? => {
                self.proxy_construct(*id, args, new_target)
            }
            Value::Function(_)
            | Value::Object(_)
            | Value::HostFunction(_)
            | Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => Err(Self::not_constructor_error(constructor)),
        }
    }

    fn not_callable_error(value: &Value) -> Error {
        Error::type_error(format!("'{value}' is not callable"))
    }

    fn not_constructor_error(value: &Value) -> Error {
        Error::type_error(format!("'{value}' is not a constructor"))
    }
}
