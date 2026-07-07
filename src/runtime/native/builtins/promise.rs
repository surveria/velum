use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        assertions::runtime_exception_value,
        call_args::RuntimeCallArgs,
        completion::Completion,
        object::{ObjectPropertyInit, PropertyEnumerable},
    },
    value::Value,
};

use super::{
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, PROMISE_CATCH_NAME, PROMISE_NAME,
    PROMISE_REJECT_NAME, PROMISE_RESOLVE_NAME, PROMISE_THEN_NAME,
};

impl Context {
    pub(in crate::runtime::native) fn promise_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Promise) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.promise_prototype_id_with_constructor(constructor.clone())?;
        let name = self.native_function_name_value(NativeFunctionKind::Promise)?;
        self.push_native_function_with_id(id, NativeFunctionKind::Promise, prototype, name)?;
        self.install_promise_static_methods(id)?;
        let Value::Object(prototype) = self.native_function(id)?.properties().prototype() else {
            return Err(Error::runtime("Promise prototype is not an object"));
        };
        self.install_promise_prototype_methods(prototype)?;
        self.insert_global_builtin(PROMISE_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn eval_promise_constructor(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_promise_constructor(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_promise_constructor(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let Some(executor) = args.first().cloned() else {
            return Err(Error::runtime("Promise constructor requires an executor"));
        };
        if !Self::is_callable(&executor) {
            return Err(Error::runtime("Promise executor must be callable"));
        }

        let (promise, object) = self.create_pending_promise()?;
        let resolve = self.create_promise_resolving_function(
            promise,
            crate::runtime::promise::PromiseResolverKind::Resolve,
        )?;
        let reject = self.create_promise_resolving_function(
            promise,
            crate::runtime::promise::PromiseResolverKind::Reject,
        )?;
        let call_result = match executor {
            Value::Function(id) => self.eval_function_completion_with_this(
                id,
                RuntimeCallArgs::values(&[resolve, reject]),
                Value::Undefined,
            )?,
            callee => {
                if let Err(error) =
                    self.eval_call_value(callee, &[resolve, reject], Value::Undefined)
                {
                    self.reject_promise(promise, promise_executor_error_value(&error))?;
                    return Ok(object);
                }
                Completion::Normal(Value::Undefined)
            }
        };
        if let Completion::Throw(value) = call_result {
            self.reject_promise(promise, value)?;
        }
        Ok(object)
    }

    pub(in crate::runtime::native) fn eval_promise_resolve(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_promise_resolve(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_promise_resolve(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        self.promise_constructor_value()?;
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        if self.promise_id_from_value(&value).is_ok() {
            return Ok(value);
        }
        self.create_fulfilled_promise(value)
    }

    pub(in crate::runtime::native) fn eval_promise_reject(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_promise_reject(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_promise_reject(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        self.promise_constructor_value()?;
        let reason = args.first().cloned().unwrap_or(Value::Undefined);
        self.create_rejected_promise(reason)
    }

    pub(in crate::runtime::native) fn eval_promise_then(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_promise_then(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_promise_then(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let promise = self.promise_id_from_value(this_value)?;
        let on_fulfilled = Self::promise_reaction_handler(args.first());
        let on_rejected = Self::promise_reaction_handler(args.get(1));
        self.promise_then(promise, on_fulfilled, on_rejected)
    }

    pub(in crate::runtime::native) fn eval_promise_catch(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_promise_catch(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_promise_catch(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let promise = self.promise_id_from_value(this_value)?;
        let on_rejected = Self::promise_reaction_handler(args.first());
        self.promise_then(promise, None, on_rejected)
    }

    pub(in crate::runtime) fn promise_constructor_prototype(
        &mut self,
    ) -> Result<crate::value::ObjectId> {
        let Value::NativeFunction(id) = self.promise_constructor_value()? else {
            return Err(Error::runtime("Promise constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("Promise prototype is not an object")),
        }
    }

    fn install_promise_static_methods(
        &mut self,
        constructor: crate::value::NativeFunctionId,
    ) -> Result<()> {
        self.define_promise_static_method(
            constructor,
            PROMISE_RESOLVE_NAME,
            NativeFunctionKind::PromiseResolve,
        )?;
        self.define_promise_static_method(
            constructor,
            PROMISE_REJECT_NAME,
            NativeFunctionKind::PromiseReject,
        )
    }

    fn define_promise_static_method(
        &mut self,
        constructor: crate::value::NativeFunctionId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        let key = self.intern_property_key(name)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, function, PropertyEnumerable::No);
        Ok(())
    }

    fn install_promise_prototype_methods(
        &mut self,
        prototype: crate::value::ObjectId,
    ) -> Result<()> {
        let then =
            self.create_native_function(NativeFunctionKind::PromiseThen, Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, PROMISE_THEN_NAME, then)?;
        let catch =
            self.create_native_function(NativeFunctionKind::PromiseCatch, Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, PROMISE_CATCH_NAME, catch)
    }

    fn promise_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<Value> {
        if let Some(prototype) = self.promise_prototype {
            self.define_non_enumerable_object_property(
                prototype,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor,
            )?;
            return Ok(Value::Object(prototype));
        }

        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_property(
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
        )?;
        self.promise_prototype = Some(prototype);
        Ok(Value::Object(prototype))
    }
}

pub(in crate::runtime::native) fn promise_executor_error_value(error: &Error) -> Value {
    runtime_exception_value(error).unwrap_or_else(|| {
        Value::Error(crate::value::ErrorObject::new(
            crate::value::ErrorName::Base,
            error.to_string(),
        ))
    })
}
