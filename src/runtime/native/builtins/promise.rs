use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::IteratorStep,
        call::RuntimeCallArgs,
        control::Completion,
        control::runtime_exception_value,
        object::{ObjectPropertyInit, PropertyEnumerable},
    },
    value::Value,
};

use super::{
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, PROMISE_ALL_NAME, PROMISE_CATCH_NAME,
    PROMISE_NAME, PROMISE_REJECT_NAME, PROMISE_RESOLVE_NAME, PROMISE_THEN_NAME,
};

const PROMISE_ALL_VALUES_PROPERTY: &str = "[[PromiseAllValues]]";
const PROMISE_ALL_REMAINING_PROPERTY: &str = "[[PromiseAllRemaining]]";
const PROMISE_ALL_RESULT_PROPERTY: &str = "[[PromiseAllResult]]";

impl Context {
    pub(in crate::runtime) fn promise_constructor_value(&mut self) -> Result<Value> {
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
            return Err(Error::type_error(
                "Promise constructor requires an executor",
            ));
        };
        if !self.semantic_is_callable(&executor)? {
            return Err(Error::type_error("Promise executor must be callable"));
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
                if let Err(error) = self.call_value(&callee, &[resolve, reject], Value::Undefined) {
                    let Some(reason) = runtime_exception_value(self, &error)? else {
                        return Err(error);
                    };
                    self.reject_promise(promise, reason)?;
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
        let on_fulfilled = self.promise_reaction_handler(args.first())?;
        let on_rejected = self.promise_reaction_handler(args.get(1))?;
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
        let on_rejected = self.promise_reaction_handler(args.first())?;
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
            PROMISE_ALL_NAME,
            NativeFunctionKind::PromiseAll,
        )?;
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

    pub(in crate::runtime::native) fn eval_promise_all(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let intrinsic = self.promise_constructor_value()?;
        if this_value != &intrinsic {
            return Err(Error::type_error(
                "Promise.all requires the intrinsic Promise constructor",
            ));
        }
        let (result_promise, result_object) = self.create_pending_promise()?;
        let iterable = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let setup = self.setup_promise_all(result_promise, result_object.clone(), iterable);
        if let Err(error) = setup {
            let Some(reason) = runtime_exception_value(self, &error)? else {
                return Err(error);
            };
            self.reject_promise(result_promise, reason)?;
        }
        Ok(result_object)
    }

    fn setup_promise_all(
        &mut self,
        result_promise: crate::runtime::promise::PromiseId,
        result_object: Value,
        iterable: Value,
    ) -> Result<()> {
        let mut iterator = self.get_iterator(iterable)?;
        let mut inputs = Vec::new();
        loop {
            self.step()?;
            match self.iterator_step(&mut iterator)? {
                IteratorStep::Value(value) => inputs.push(value),
                IteratorStep::Done => break,
                IteratorStep::Abrupt(completion) => return completion.into_result().map(|_| ()),
            }
        }

        let values = self.create_array_from_elements(vec![Value::Undefined; inputs.len()])?;
        if inputs.is_empty() {
            return self.resolve_promise(result_promise, values);
        }

        let constructor_key = self.object_constructor_property_key()?;
        let state = self.objects.create_empty_data_object(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.define_non_enumerable_object_property(state, PROMISE_ALL_VALUES_PROPERTY, values)?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_REMAINING_PROPERTY,
            Value::Number(Self::usize_to_number(
                inputs.len(),
                "Promise.all input count exceeds numeric range",
            )?),
        )?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_RESULT_PROPERTY,
            result_object,
        )?;

        for (index, input) in inputs.into_iter().enumerate() {
            let input_promise = self.promise_resolve_for_await(input)?;
            let on_fulfilled = self.create_ephemeral_native_function(
                NativeFunctionKind::PromiseAllResolveElement { state, index },
                Value::Undefined,
            )?;
            let on_rejected = self.create_promise_resolving_function(
                result_promise,
                crate::runtime::promise::PromiseResolverKind::Reject,
            )?;
            let chained =
                self.promise_then(input_promise, Some(on_fulfilled), Some(on_rejected))?;
            drop(chained);
        }
        Ok(())
    }

    pub(in crate::runtime::native) fn eval_promise_all_resolve_element(
        &mut self,
        state: crate::value::ObjectId,
        index: usize,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let state_value = Value::Object(state);
        let values = self.get_named(&state_value, PROMISE_ALL_VALUES_PROPERTY)?;
        let Value::Object(values_id) = values.clone() else {
            return Err(Error::runtime("Promise.all values state is not an array"));
        };
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !self.objects.set_array_index_if_array(
            values_id,
            index,
            value,
            self.limits.max_object_properties,
        )? {
            return Err(Error::runtime("Promise.all values state is not an array"));
        }

        let remaining = self.get_named(&state_value, PROMISE_ALL_REMAINING_PROPERTY)?;
        let Value::Number(remaining) = remaining else {
            return Err(Error::runtime("Promise.all remaining state is not numeric"));
        };
        let next = remaining - 1.0;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_REMAINING_PROPERTY,
            Value::Number(next),
        )?;
        if next == 0.0 {
            let result = self.get_named(&state_value, PROMISE_ALL_RESULT_PROPERTY)?;
            let result_promise = self.promise_id_from_value(&result)?;
            self.resolve_promise(result_promise, values)?;
        }
        Ok(Value::Undefined)
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
            .define_builtin(key, function, PropertyEnumerable::No)?;
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
        self.storage_ledger
            .grow_count(crate::runtime::VmStorageKind::Association, 1)?;
        self.promise_prototype = Some(prototype);
        Ok(Value::Object(prototype))
    }
}
