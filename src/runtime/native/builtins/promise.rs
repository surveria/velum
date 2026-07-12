use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{IteratorSource, IteratorStep},
        call::RuntimeCallArgs,
        control::Completion,
        control::runtime_exception_value,
        native::NativeFunctionKind,
        object::{
            DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
            PropertyUpdate, PropertyWritable,
        },
        promise::{PromiseCombinatorElementKind, PromiseCombinatorKind},
        roots::VmRootKind,
    },
    value::{ObjectId, Value},
};

use super::{
    OBJECT_CONSTRUCTOR_PROPERTY, PROMISE_CATCH_NAME, PROMISE_NAME, PROMISE_REJECT_NAME,
    PROMISE_RESOLVE_NAME, PROMISE_THEN_NAME,
};

mod combinators;

const PROMISE_CAPABILITY_REJECT_PROPERTY: &str = "[[PromiseCapabilityReject]]";
const PROMISE_CAPABILITY_RESOLVE_PROPERTY: &str = "[[PromiseCapabilityResolve]]";
const PROMISE_ALL_ALREADY_CALLED_PROPERTY: &str = "[[PromiseAllAlreadyCalled]]";
const PROMISE_ALL_SHARED_STATE_PROPERTY: &str = "[[PromiseAllSharedState]]";
const PROMISE_ALL_VALUES_PROPERTY: &str = "[[PromiseAllValues]]";
const PROMISE_ALL_REMAINING_PROPERTY: &str = "[[PromiseAllRemaining]]";
const PROMISE_ALL_RESOLVE_PROPERTY: &str = "[[PromiseAllResolve]]";
const PROMISE_COMBINATOR_COUNT_ERROR: &str = "Promise combinator input count exceeds numeric range";

struct PromiseCapability {
    promise: Value,
    resolve: Value,
    reject: Value,
}

impl PromiseCapability {
    const fn root_values(&self) -> [&Value; 3] {
        [&self.promise, &self.resolve, &self.reject]
    }
}

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
        self.install_species_accessor(id)?;
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
        let prototype = self.promise_constructor_prototype()?;
        self.eval_promise_constructor_with_prototype(args, prototype)
    }

    pub(in crate::runtime) fn construct_promise_with_new_target(
        &mut self,
        args: &[Value],
        new_target: &Value,
    ) -> Result<Value> {
        let prototype = match self.constructor_instance_prototype(new_target)? {
            Some(prototype) => prototype,
            None => self.promise_constructor_prototype()?,
        };
        self.eval_promise_constructor_with_prototype(args, prototype)
    }

    fn eval_promise_constructor_with_prototype(
        &mut self,
        args: &[Value],
        prototype: ObjectId,
    ) -> Result<Value> {
        let executor = self.promise_executor_argument(args)?;
        let (promise, object) = self.create_pending_promise_with_prototype(prototype)?;
        self.run_promise_executor(promise, object, executor)
    }

    pub(in crate::runtime) fn initialize_promise_super_instance(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<()> {
        let executor = self.promise_executor_argument(args)?;
        let Value::Object(object) = this_value else {
            return Err(Error::runtime("Promise super receiver is not an object"));
        };
        if self.promise_id_from_value(this_value).is_ok() {
            return Err(Error::type_error(
                "Promise super constructor was called twice",
            ));
        }
        let promise = self.create_pending_promise_for_object(*object)?;
        self.run_promise_executor(promise, this_value.clone(), executor)
            .map(|_object| ())
    }

    fn promise_executor_argument(&self, args: &[Value]) -> Result<Value> {
        let Some(executor) = args.first().cloned() else {
            return Err(Error::type_error(
                "Promise constructor requires an executor",
            ));
        };
        if !self.semantic_is_callable(&executor)? {
            return Err(Error::type_error("Promise executor must be callable"));
        }
        Ok(executor)
    }

    fn run_promise_executor(
        &mut self,
        promise: crate::runtime::promise::PromiseId,
        object: Value,
        executor: Value,
    ) -> Result<Value> {
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
        constructor: &Value,
    ) -> Result<Value> {
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_constructor(constructor)? {
            return Err(Error::type_error(
                "Promise.resolve receiver must be a constructor",
            ));
        }
        if self.promise_id_from_value(&value).is_ok()
            && self.get_named(&value, OBJECT_CONSTRUCTOR_PROPERTY)? == *constructor
        {
            return Ok(value);
        }
        let intrinsic = self.promise_constructor_value()?;
        if constructor == &intrinsic {
            return self.eval_direct_promise_resolve(std::slice::from_ref(&value));
        }
        let capability = self.new_promise_capability(constructor)?;
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            capability
                .root_values()
                .into_iter()
                .chain(std::iter::once(&value)),
        )?;
        self.call_value(&capability.resolve, &[value], Value::Undefined)?;
        Ok(capability.promise)
    }

    pub(in crate::runtime::native) fn eval_direct_promise_resolve(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let intrinsic = self.promise_constructor_value()?;
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        if self.promise_id_from_value(&value).is_ok()
            && self.get_named(&value, OBJECT_CONSTRUCTOR_PROPERTY)? == intrinsic
        {
            return Ok(value);
        }
        let (promise, object) = self.create_pending_promise()?;
        self.resolve_promise(promise, value)?;
        Ok(object)
    }

    pub(in crate::runtime::native) fn eval_promise_reject(
        &mut self,
        args: RuntimeCallArgs<'_>,
        constructor: &Value,
    ) -> Result<Value> {
        let reason = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let intrinsic = self.promise_constructor_value()?;
        if constructor == &intrinsic {
            return self.eval_direct_promise_reject(std::slice::from_ref(&reason));
        }
        let capability = self.new_promise_capability(constructor)?;
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            capability
                .root_values()
                .into_iter()
                .chain(std::iter::once(&reason)),
        )?;
        self.call_value(&capability.reject, &[reason], Value::Undefined)?;
        Ok(capability.promise)
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

    pub(in crate::runtime::native) fn eval_promise_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::Promise => Some(self.eval_promise_constructor(args)),
            NativeFunctionKind::PromiseCombinator(kind) => {
                Some(self.eval_promise_combinator(kind, args, this_value))
            }
            NativeFunctionKind::PromiseCombinatorElement { state, index, kind } => {
                Some(self.eval_promise_combinator_element(state, index, kind, args))
            }
            NativeFunctionKind::PromiseCapabilityExecutor { capability_state } => {
                Some(self.eval_promise_capability_executor(capability_state, args))
            }
            NativeFunctionKind::PromiseResolve => Some(self.eval_promise_resolve(args, this_value)),
            NativeFunctionKind::PromiseReject => Some(self.eval_promise_reject(args, this_value)),
            NativeFunctionKind::PromiseThen => Some(self.eval_promise_then(args, this_value)),
            NativeFunctionKind::PromiseCatch => Some(self.eval_promise_catch(args, this_value)),
            NativeFunctionKind::PromiseResolver { promise, kind } => {
                Some(self.eval_promise_resolver(promise, kind, args))
            }
            _ => None,
        }
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
        for kind in PromiseCombinatorKind::ALL {
            self.define_promise_static_method(
                constructor,
                kind.name(),
                NativeFunctionKind::PromiseCombinator(kind),
            )?;
        }
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
        let iterable = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let capability = self.new_promise_capability(this_value)?;
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            capability
                .root_values()
                .into_iter()
                .chain(std::iter::once(&iterable)),
        )?;
        let setup = self.setup_promise_all(&capability, this_value, &iterable);
        if let Err(error) = setup {
            self.reject_promise_combinator_capability(&capability, error)?;
        }
        Ok(capability.promise)
    }

    fn new_promise_capability(&mut self, constructor: &Value) -> Result<PromiseCapability> {
        if !self.semantic_is_constructor(constructor)? {
            return Err(Error::type_error(
                "Promise combinator receiver must be a constructor",
            ));
        }
        let intrinsic = self.promise_constructor_value()?;
        if constructor == &intrinsic {
            return self.create_intrinsic_promise_capability();
        }
        let state = self.create_promise_internal_state()?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_CAPABILITY_RESOLVE_PROPERTY,
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_CAPABILITY_REJECT_PROPERTY,
            Value::Undefined,
        )?;
        let executor = self.create_ephemeral_native_function(
            NativeFunctionKind::PromiseCapabilityExecutor {
                capability_state: state,
            },
            Value::Undefined,
        )?;
        let promise = self.semantic_construct(
            constructor,
            std::slice::from_ref(&executor),
            constructor.clone(),
        )?;
        let state = Value::Object(state);
        let resolve = self.get_named(&state, PROMISE_CAPABILITY_RESOLVE_PROPERTY)?;
        let reject = self.get_named(&state, PROMISE_CAPABILITY_REJECT_PROPERTY)?;
        if !self.semantic_is_callable(&resolve)? || !self.semantic_is_callable(&reject)? {
            return Err(Error::type_error(
                "Promise capability resolve and reject must be callable",
            ));
        }
        Ok(PromiseCapability {
            promise,
            resolve,
            reject,
        })
    }

    fn create_intrinsic_promise_capability(&mut self) -> Result<PromiseCapability> {
        let (promise, object) = self.create_pending_promise()?;
        let resolve = self.create_promise_resolving_function(
            promise,
            crate::runtime::promise::PromiseResolverKind::Resolve,
        )?;
        let reject = self.create_promise_resolving_function(
            promise,
            crate::runtime::promise::PromiseResolverKind::Reject,
        )?;
        Ok(PromiseCapability {
            promise: object,
            resolve,
            reject,
        })
    }

    pub(in crate::runtime::native) fn eval_promise_capability_executor(
        &mut self,
        state: ObjectId,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let state_value = Value::Object(state);
        let resolve = self.get_named(&state_value, PROMISE_CAPABILITY_RESOLVE_PROPERTY)?;
        let reject = self.get_named(&state_value, PROMISE_CAPABILITY_REJECT_PROPERTY)?;
        if !matches!(resolve, Value::Undefined) || !matches!(reject, Value::Undefined) {
            return Err(Error::type_error(
                "Promise capability executor was already initialized",
            ));
        }
        let values = args.as_slice();
        self.define_non_enumerable_object_property(
            state,
            PROMISE_CAPABILITY_RESOLVE_PROPERTY,
            values.first().cloned().unwrap_or(Value::Undefined),
        )?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_CAPABILITY_REJECT_PROPERTY,
            values.get(1).cloned().unwrap_or(Value::Undefined),
        )?;
        Ok(Value::Undefined)
    }

    fn reject_promise_combinator_capability(
        &mut self,
        capability: &PromiseCapability,
        error: Error,
    ) -> Result<()> {
        let Some(reason) = runtime_exception_value(self, &error)? else {
            return Err(error);
        };
        self.call_value(&capability.reject, &[reason], Value::Undefined)
            .map(|_value| ())
    }

    fn setup_promise_all(
        &mut self,
        capability: &PromiseCapability,
        constructor: &Value,
        iterable: &Value,
    ) -> Result<()> {
        let promise_resolve = self.get_named(constructor, PROMISE_RESOLVE_NAME)?;
        if !self.semantic_is_callable(&promise_resolve)? {
            return Err(Error::type_error("Promise resolve method must be callable"));
        }
        let mut iterator = self.get_iterator(iterable)?;
        let result =
            self.perform_promise_all(capability, constructor, &promise_resolve, &mut iterator);
        match result {
            Ok(()) => Ok(()),
            Err(error) => Err(self.iterator_close_on_error(&mut iterator, error)),
        }
    }

    fn perform_promise_all(
        &mut self,
        capability: &PromiseCapability,
        constructor: &Value,
        promise_resolve: &Value,
        iterator: &mut IteratorSource,
    ) -> Result<()> {
        let _iterator_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, iterator.root_values())?;
        let values = self.create_array_from_elements(Vec::new())?;
        let state = self.create_promise_internal_state()?;
        let state_value = Value::Object(state);
        let _state_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            std::iter::once(&state_value),
        )?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_VALUES_PROPERTY,
            values.clone(),
        )?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_REMAINING_PROPERTY,
            Value::Number(1.0),
        )?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_RESOLVE_PROPERTY,
            capability.resolve.clone(),
        )?;
        let mut index = 0_usize;
        loop {
            self.step()?;
            let step = match self.iterator_step(iterator) {
                Ok(step) => step,
                Err(error) => {
                    iterator.mark_done();
                    return Err(error);
                }
            };
            let input = match step {
                IteratorStep::Value(value) => value,
                IteratorStep::Done => {
                    if self.change_promise_all_remaining(state, false)? == 0 {
                        self.call_value(&capability.resolve, &[values], Value::Undefined)?;
                    }
                    return Ok(());
                }
                IteratorStep::Abrupt(completion) => return completion.into_result().map(|_| ()),
            };
            self.set_promise_all_value(&values, index, Value::Undefined)?;
            let next_promise = self.call_value(
                promise_resolve,
                std::slice::from_ref(&input),
                constructor.clone(),
            )?;
            let resolve_element = self.create_promise_all_resolve_element(state, index)?;
            self.change_promise_all_remaining(state, true)?;
            let then = self.get_named(&next_promise, PROMISE_THEN_NAME)?;
            self.call_value(
                &then,
                &[resolve_element, capability.reject.clone()],
                next_promise,
            )?;
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::limit(PROMISE_COMBINATOR_COUNT_ERROR))?;
        }
    }

    pub(in crate::runtime::native) fn eval_promise_all_resolve_element(
        &mut self,
        state: crate::value::ObjectId,
        index: usize,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let element_state = Value::Object(state);
        let already_called = self.get_named(&element_state, PROMISE_ALL_ALREADY_CALLED_PROPERTY)?;
        let Value::Bool(already_called) = already_called else {
            return Err(Error::runtime("Promise.all call state is not boolean"));
        };
        if already_called {
            return Ok(Value::Undefined);
        }
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_ALREADY_CALLED_PROPERTY,
            Value::Bool(true),
        )?;
        let shared_state = self.get_named(&element_state, PROMISE_ALL_SHARED_STATE_PROPERTY)?;
        let Value::Object(shared_state) = shared_state else {
            return Err(Error::runtime("Promise.all shared state is not an object"));
        };
        let shared_value = Value::Object(shared_state);
        let values = self.get_named(&shared_value, PROMISE_ALL_VALUES_PROPERTY)?;
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        self.set_promise_all_value(&values, index, value)?;
        if self.change_promise_all_remaining(shared_state, false)? == 0 {
            let resolve = self.get_named(&shared_value, PROMISE_ALL_RESOLVE_PROPERTY)?;
            self.call_value(&resolve, &[values], Value::Undefined)?;
        }
        Ok(Value::Undefined)
    }

    fn create_promise_internal_state(&mut self) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_empty_data_object(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn create_promise_all_resolve_element(
        &mut self,
        shared_state: ObjectId,
        index: usize,
    ) -> Result<Value> {
        let state = self.create_promise_internal_state()?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_ALREADY_CALLED_PROPERTY,
            Value::Bool(false),
        )?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_SHARED_STATE_PROPERTY,
            Value::Object(shared_state),
        )?;
        self.create_ephemeral_native_function(
            NativeFunctionKind::PromiseCombinatorElement {
                state,
                index,
                kind: PromiseCombinatorElementKind::AllResolve,
            },
            Value::Undefined,
        )
    }

    fn change_promise_all_remaining(&mut self, state: ObjectId, increment: bool) -> Result<usize> {
        let state_value = Value::Object(state);
        let remaining = self.get_named(&state_value, PROMISE_ALL_REMAINING_PROPERTY)?;
        let Value::Number(remaining) = remaining else {
            return Err(Error::runtime("Promise.all remaining state is not numeric"));
        };
        let remaining =
            Self::finite_nonnegative_integer_to_usize(remaining, PROMISE_COMBINATOR_COUNT_ERROR)?;
        let next = if increment {
            remaining
                .checked_add(1)
                .ok_or_else(|| Error::limit(PROMISE_COMBINATOR_COUNT_ERROR))?
        } else {
            remaining
                .checked_sub(1)
                .ok_or_else(|| Error::runtime("Promise.all remaining state underflowed"))?
        };
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_REMAINING_PROPERTY,
            Value::Number(Self::usize_to_number(next, PROMISE_COMBINATOR_COUNT_ERROR)?),
        )?;
        Ok(next)
    }

    fn set_promise_all_value(&mut self, values: &Value, index: usize, value: Value) -> Result<()> {
        let Value::Object(values) = values else {
            return Err(Error::runtime("Promise.all values state is not an array"));
        };
        if self.objects.array_len_if_array(*values)?.is_none() {
            return Err(Error::runtime("Promise.all values state is not an array"));
        }
        let property = index.to_string();
        let key = self.intern_property_key(&property)?;
        self.objects.define_property(
            *values,
            key,
            &property,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::Yes),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
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
