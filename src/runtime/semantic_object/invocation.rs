use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::same_value,
        call::RuntimeCallArgs,
        control::Completion,
        native::{
            DataViewFunctionKind, IntlFunctionKind, IteratorFunctionKind, NativeFunctionKind,
        },
        roots::VmRootKind,
    },
    value::Value,
};

impl Context {
    pub(in crate::runtime) fn semantic_is_array(&self, value: &Value) -> Result<bool> {
        let mut current = value.clone();
        let mut depth = 0_usize;
        loop {
            let Value::Object(id) = current else {
                return Ok(false);
            };
            if !self.objects.is_proxy(id) {
                return self
                    .objects
                    .array_len_if_array(id)
                    .map(|length| length.is_some());
            }
            depth = depth
                .checked_add(1)
                .ok_or_else(|| Error::limit("proxy array test depth overflowed"))?;
            if depth > self.objects.object_count() {
                return Err(Error::runtime("proxy array target chain is cyclic"));
            }
            let (target, _handler) = self.proxy_target_handler(id)?;
            current = target;
        }
    }

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
            | Value::BigInt(_)
            | Value::String(_)
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
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientCall,
            std::iter::once(callee)
                .chain(std::iter::once(&this_value))
                .chain(args.iter()),
        )?;
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
                self.eval_native_function_in_realm(*id, kind, args, &this_value)
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
            | Value::BigInt(_)
            | Value::String(_)
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
                    if self.bound_function_is_shadow_realm(bound)? {
                        return Ok(false);
                    }
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
            | Value::BigInt(_)
            | Value::String(_)
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
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientCall,
            std::iter::once(constructor)
                .chain(std::iter::once(&new_target))
                .chain(args.iter()),
        )?;
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
                let function = self.native_function(*id)?;
                let kind = function.kind();
                let realm = function.realm();
                self.with_realm(realm, |context| {
                    context.construct_native_function_in_active_realm(
                        kind,
                        args,
                        constructor,
                        new_target,
                    )
                })
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
            | Value::BigInt(_)
            | Value::String(_)
            | Value::Symbol(_) => Err(Self::not_constructor_error(constructor)),
        }
    }

    fn construct_native_function_in_active_realm(
        &mut self,
        kind: NativeFunctionKind,
        args: &[Value],
        constructor: &Value,
        new_target: Value,
    ) -> Result<Value> {
        if let NativeFunctionKind::BoundFunction(bound) = kind {
            return self.eval_bound_function_construct(bound, args, constructor, new_target);
        }
        if let NativeFunctionKind::ErrorConstructor(name) = kind {
            let prototype = self.constructor_instance_prototype_with_default(
                &new_target,
                NativeFunctionKind::ErrorConstructor(name),
            )?;
            return self.eval_direct_error_constructor_with_prototype(name, args, Some(prototype));
        }
        if kind == NativeFunctionKind::Promise {
            return self.construct_promise_with_new_target(args, &new_target);
        }
        if kind == NativeFunctionKind::Iterator(IteratorFunctionKind::Constructor) {
            return self.construct_iterator_object(constructor, &new_target);
        }
        if kind
            == NativeFunctionKind::DisposableStack(
                crate::runtime::native::DisposableStackFunctionKind::Constructor,
            )
        {
            return self.construct_disposable_stack_with_new_target(&new_target);
        }
        if kind
            == NativeFunctionKind::AsyncDisposableStack(
                crate::runtime::native::AsyncDisposableStackFunctionKind::Constructor,
            )
        {
            return self.construct_async_disposable_stack_with_new_target(&new_target);
        }
        if kind == NativeFunctionKind::DataView(DataViewFunctionKind::Constructor) {
            return self.construct_data_view_with_new_target(args, &new_target);
        }
        self.construct_native_with_new_target(kind, args, constructor, &new_target)
    }

    fn construct_native_with_new_target(
        &mut self,
        kind: NativeFunctionKind,
        args: &[Value],
        constructor: &Value,
        new_target: &Value,
    ) -> Result<Value> {
        if kind == NativeFunctionKind::Object && !same_value(constructor, new_target) {
            let prototype = self.constructor_instance_prototype_with_default(
                new_target,
                NativeFunctionKind::Object,
            )?;
            let constructor_key = self.object_constructor_property_key()?;
            return self.objects.create_with_prototype(
                Some(prototype),
                constructor_key,
                self.limits.max_objects,
                self.limits.max_object_properties,
            );
        }

        let eager_prototype = if matches!(
            kind,
            NativeFunctionKind::Intl(IntlFunctionKind::DisplayNamesConstructor)
        ) && !same_value(constructor, new_target)
        {
            Some(self.constructor_instance_prototype_with_default(new_target, kind)?)
        } else {
            None
        };
        let value = self.construct_native_function_kind(kind, RuntimeCallArgs::values(args))?;
        if same_value(constructor, new_target) || kind == NativeFunctionKind::Proxy {
            return Ok(value);
        }
        let prototype = if let Some(prototype) = eager_prototype {
            prototype
        } else {
            self.constructor_instance_prototype_with_default(new_target, kind)?
        };
        match self.semantic_try_set_prototype(&value, Value::Object(prototype))? {
            Some(true) => Ok(value),
            Some(false) | None => Err(Error::runtime(
                "native construction could not apply the new.target prototype",
            )),
        }
    }

    fn not_callable_error(value: &Value) -> Error {
        Error::type_error(format!("'{value}' is not callable"))
    }

    fn not_constructor_error(value: &Value) -> Error {
        Error::type_error(format!("'{value}' is not a constructor"))
    }
}
