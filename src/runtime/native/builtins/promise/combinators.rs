#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{IteratorSource, IteratorStep},
        call::RuntimeCallArgs,
        native::NativeFunctionKind,
        object::{
            DataPropertyUpdate, ObjectPropertyInit, OwnPropertyDescriptor, PropertyConfigurable,
            PropertyEnumerable, PropertyUpdate, PropertyWritable,
        },
        promise::{PromiseCombinatorElementKind, PromiseCombinatorKind},
        roots::VmRootKind,
    },
    value::{ObjectId, Value},
};

use super::{
    PROMISE_ALL_ALREADY_CALLED_PROPERTY, PROMISE_ALL_REMAINING_PROPERTY,
    PROMISE_ALL_RESOLVE_PROPERTY, PROMISE_ALL_SHARED_STATE_PROPERTY, PROMISE_ALL_VALUES_PROPERTY,
    PROMISE_COMBINATOR_COUNT_ERROR, PROMISE_RESOLVE_NAME, PROMISE_THEN_NAME, PromiseCapability,
};

const PROMISE_ANY_REJECT_PROPERTY: &str = "[[PromiseAnyReject]]";
const PROMISE_KEYED_KEYS_PROPERTY: &str = "[[PromiseKeyedKeys]]";
const SETTLED_STATUS_PROPERTY: &str = "status";
const SETTLED_VALUE_PROPERTY: &str = "value";
const SETTLED_REASON_PROPERTY: &str = "reason";
const SETTLED_FULFILLED_STATUS: &str = "fulfilled";
const SETTLED_REJECTED_STATUS: &str = "rejected";

impl Context {
    pub(super) fn eval_promise_combinator(
        &mut self,
        kind: PromiseCombinatorKind,
        args: RuntimeCallArgs<'_>,
        constructor: &Value,
    ) -> Result<Value> {
        if kind == PromiseCombinatorKind::All {
            return self.eval_promise_all(args, constructor);
        }
        let iterable = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let capability = self.new_promise_capability(constructor)?;
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            capability
                .root_values()
                .into_iter()
                .chain(core::iter::once(&iterable)),
        )?;
        if let Err(error) =
            self.setup_settlement_combinator(kind, &capability, constructor, &iterable)
        {
            self.reject_promise_combinator_capability(&capability, error)?;
        }
        Ok(capability.promise)
    }

    pub(super) fn eval_promise_combinator_element(
        &mut self,
        state: ObjectId,
        index: usize,
        kind: PromiseCombinatorElementKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        match kind {
            PromiseCombinatorElementKind::AllResolve => {
                self.eval_promise_all_resolve_element(state, index, args)
            }
            PromiseCombinatorElementKind::AllKeyedResolve => {
                self.eval_keyed_element(state, index, None, args)
            }
            PromiseCombinatorElementKind::AllSettledFulfill => {
                self.eval_all_settled_element(state, index, true, args)
            }
            PromiseCombinatorElementKind::AllSettledReject => {
                self.eval_all_settled_element(state, index, false, args)
            }
            PromiseCombinatorElementKind::AllSettledKeyedFulfill => {
                self.eval_keyed_element(state, index, Some(true), args)
            }
            PromiseCombinatorElementKind::AllSettledKeyedReject => {
                self.eval_keyed_element(state, index, Some(false), args)
            }
            PromiseCombinatorElementKind::AnyReject => {
                self.eval_promise_any_reject_element(state, index, args)
            }
        }
    }

    fn setup_settlement_combinator(
        &mut self,
        kind: PromiseCombinatorKind,
        capability: &PromiseCapability,
        constructor: &Value,
        iterable: &Value,
    ) -> Result<()> {
        let promise_resolve = self.get_named(constructor, PROMISE_RESOLVE_NAME)?;
        if !self.semantic_is_callable(&promise_resolve)? {
            return Err(Error::type_error("Promise resolve method must be callable"));
        }
        if matches!(
            kind,
            PromiseCombinatorKind::AllKeyed | PromiseCombinatorKind::AllSettledKeyed
        ) {
            return self.perform_promise_keyed(
                capability,
                constructor,
                &promise_resolve,
                iterable,
                kind == PromiseCombinatorKind::AllSettledKeyed,
            );
        }
        let mut iterator = self.get_iterator(iterable)?;
        let result = match kind {
            PromiseCombinatorKind::All => {
                return Err(Error::runtime(
                    "Promise.all reached settlement combinator setup",
                ));
            }
            PromiseCombinatorKind::AllKeyed => Err(Error::runtime(
                "Promise.allKeyed reached iterator combinator setup",
            )),
            PromiseCombinatorKind::AllSettled => self.perform_promise_all_settled(
                capability,
                constructor,
                &promise_resolve,
                &mut iterator,
            ),
            PromiseCombinatorKind::AllSettledKeyed => Err(Error::runtime(
                "Promise.allSettledKeyed reached iterator combinator setup",
            )),
            PromiseCombinatorKind::Any => {
                self.perform_promise_any(capability, constructor, &promise_resolve, &mut iterator)
            }
            PromiseCombinatorKind::Race => {
                self.perform_promise_race(capability, constructor, &promise_resolve, &mut iterator)
            }
        };
        result.map_err(|error| self.iterator_close_on_error(&mut iterator, error))
    }

    fn perform_promise_race(
        &mut self,
        capability: &PromiseCapability,
        constructor: &Value,
        promise_resolve: &Value,
        iterator: &mut IteratorSource,
    ) -> Result<()> {
        let _iterator_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, iterator.root_values())?;
        loop {
            self.step()?;
            let Some(input) = self.next_combinator_input(iterator)? else {
                return Ok(());
            };
            let next_promise = self.call_value(
                promise_resolve,
                core::slice::from_ref(&input),
                constructor.clone(),
            )?;
            let then = self.get_named(&next_promise, PROMISE_THEN_NAME)?;
            self.call_value(
                &then,
                &[capability.resolve.clone(), capability.reject.clone()],
                next_promise,
            )?;
        }
    }

    fn perform_promise_keyed(
        &mut self,
        capability: &PromiseCapability,
        constructor: &Value,
        promise_resolve: &Value,
        promises: &Value,
        settled: bool,
    ) -> Result<()> {
        if self.semantic_object_ref(promises)?.is_none() {
            return Err(Error::type_error(
                "Promise keyed combinator input must be an object",
            ));
        }
        let source_keys = self.semantic_own_property_keys(promises)?;
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values(source_keys.iter())?;
        let (state, values) =
            self.create_settlement_state(PROMISE_ALL_RESOLVE_PROPERTY, capability.resolve.clone())?;
        let keys = self.create_array_from_elements(Vec::new())?;
        roots.add_values([&values, &keys])?;
        self.define_non_enumerable_object_property(
            state,
            PROMISE_KEYED_KEYS_PROPERTY,
            keys.clone(),
        )?;
        let mut index = 0_usize;
        for key in source_keys {
            self.step()?;
            let property = self.dynamic_property_key(&key)?;
            let Some(descriptor) = self.semantic_own_property_descriptor(promises, &property)?
            else {
                continue;
            };
            let enumerable = match descriptor {
                OwnPropertyDescriptor::Data(descriptor) => descriptor.enumerable(),
                OwnPropertyDescriptor::Accessor(descriptor) => descriptor.enumerable(),
            };
            if !enumerable.is_yes() {
                continue;
            }
            let input = self.get(promises, property.lookup())?;
            self.set_promise_all_value(&keys, index, key)?;
            self.set_promise_all_value(&values, index, Value::Undefined)?;
            let next_promise = self.call_value(
                promise_resolve,
                core::slice::from_ref(&input),
                constructor.clone(),
            )?;
            let (on_fulfilled, on_rejected) =
                self.create_keyed_elements(state, index, settled, &capability.reject)?;
            self.change_promise_all_remaining(state, true)?;
            let then = self.get_named(&next_promise, PROMISE_THEN_NAME)?;
            let handlers: [Value; 2] = (on_fulfilled, on_rejected).into();
            self.call_value(&then, &handlers, next_promise)?;
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::limit(PROMISE_COMBINATOR_COUNT_ERROR))?;
        }
        if self.change_promise_all_remaining(state, false)? == 0 {
            self.resolve_keyed_state(state)?;
        }
        Ok(())
    }

    fn perform_promise_all_settled(
        &mut self,
        capability: &PromiseCapability,
        constructor: &Value,
        promise_resolve: &Value,
        iterator: &mut IteratorSource,
    ) -> Result<()> {
        let _iterator_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, iterator.root_values())?;
        let (state, values) =
            self.create_settlement_state(PROMISE_ALL_RESOLVE_PROPERTY, capability.resolve.clone())?;
        let mut index = 0_usize;
        loop {
            self.step()?;
            let Some(input) = self.next_combinator_input(iterator)? else {
                if self.change_promise_all_remaining(state, false)? == 0 {
                    self.call_value(&capability.resolve, &[values], Value::Undefined)?;
                }
                return Ok(());
            };
            self.set_promise_all_value(&values, index, Value::Undefined)?;
            let next_promise = self.call_value(
                promise_resolve,
                core::slice::from_ref(&input),
                constructor.clone(),
            )?;
            let (on_fulfilled, on_rejected) = self.create_all_settled_elements(state, index)?;
            self.change_promise_all_remaining(state, true)?;
            let then = self.get_named(&next_promise, PROMISE_THEN_NAME)?;
            let handlers: [Value; 2] = (on_fulfilled, on_rejected).into();
            self.call_value(&then, &handlers, next_promise)?;
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::limit(PROMISE_COMBINATOR_COUNT_ERROR))?;
        }
    }

    fn perform_promise_any(
        &mut self,
        capability: &PromiseCapability,
        constructor: &Value,
        promise_resolve: &Value,
        iterator: &mut IteratorSource,
    ) -> Result<()> {
        let _iterator_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, iterator.root_values())?;
        let (state, errors) =
            self.create_settlement_state(PROMISE_ANY_REJECT_PROPERTY, capability.reject.clone())?;
        let mut index = 0_usize;
        loop {
            self.step()?;
            let Some(input) = self.next_combinator_input(iterator)? else {
                if self.change_promise_all_remaining(state, false)? == 0 {
                    self.reject_promise_any_errors(&capability.reject, errors)?;
                }
                return Ok(());
            };
            self.set_promise_all_value(&errors, index, Value::Undefined)?;
            let next_promise = self.call_value(
                promise_resolve,
                core::slice::from_ref(&input),
                constructor.clone(),
            )?;
            let on_rejected = self.create_promise_any_reject_element(state, index)?;
            self.change_promise_all_remaining(state, true)?;
            let then = self.get_named(&next_promise, PROMISE_THEN_NAME)?;
            self.call_value(
                &then,
                &[capability.resolve.clone(), on_rejected],
                next_promise,
            )?;
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::limit(PROMISE_COMBINATOR_COUNT_ERROR))?;
        }
    }

    fn next_combinator_input(&mut self, iterator: &mut IteratorSource) -> Result<Option<Value>> {
        match self.iterator_step(iterator) {
            Ok(IteratorStep::Value(value)) => Ok(Some(value)),
            Ok(IteratorStep::Done) => Ok(None),
            Ok(IteratorStep::Abrupt(completion)) => completion.into_result().map(Some),
            Err(error) => {
                iterator.mark_done();
                Err(error)
            }
        }
    }

    fn create_settlement_state(
        &mut self,
        finalizer_property: &str,
        finalizer: Value,
    ) -> Result<(ObjectId, Value)> {
        let values = self.create_array_from_elements(Vec::new())?;
        let state = self.create_promise_internal_state()?;
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
        self.define_non_enumerable_object_property(state, finalizer_property, finalizer)?;
        Ok((state, values))
    }

    fn create_all_settled_elements(
        &mut self,
        shared_state: ObjectId,
        index: usize,
    ) -> Result<(Value, Value)> {
        let state = self.create_combinator_element_state(shared_state)?;
        let on_fulfilled = self.create_combinator_element_function(
            state,
            index,
            PromiseCombinatorElementKind::AllSettledFulfill,
        )?;
        let _fulfilled_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            core::iter::once(&on_fulfilled),
        )?;
        let on_rejected = self.create_combinator_element_function(
            state,
            index,
            PromiseCombinatorElementKind::AllSettledReject,
        )?;
        Ok((on_fulfilled, on_rejected))
    }

    fn create_keyed_elements(
        &mut self,
        shared_state: ObjectId,
        index: usize,
        settled: bool,
        reject: &Value,
    ) -> Result<(Value, Value)> {
        if !settled {
            let state = self.create_combinator_element_state(shared_state)?;
            let on_fulfilled = self.create_combinator_element_function(
                state,
                index,
                PromiseCombinatorElementKind::AllKeyedResolve,
            )?;
            return Ok((on_fulfilled, reject.clone()));
        }
        let state = self.create_combinator_element_state(shared_state)?;
        let on_fulfilled = self.create_combinator_element_function(
            state,
            index,
            PromiseCombinatorElementKind::AllSettledKeyedFulfill,
        )?;
        let _fulfilled_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            core::iter::once(&on_fulfilled),
        )?;
        let on_rejected = self.create_combinator_element_function(
            state,
            index,
            PromiseCombinatorElementKind::AllSettledKeyedReject,
        )?;
        Ok((on_fulfilled, on_rejected))
    }

    fn create_promise_any_reject_element(
        &mut self,
        shared_state: ObjectId,
        index: usize,
    ) -> Result<Value> {
        let state = self.create_combinator_element_state(shared_state)?;
        self.create_combinator_element_function(
            state,
            index,
            PromiseCombinatorElementKind::AnyReject,
        )
    }

    fn create_combinator_element_state(&mut self, shared_state: ObjectId) -> Result<ObjectId> {
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
        Ok(state)
    }

    fn create_combinator_element_function(
        &mut self,
        state: ObjectId,
        index: usize,
        kind: PromiseCombinatorElementKind,
    ) -> Result<Value> {
        self.create_ephemeral_native_function(
            NativeFunctionKind::PromiseCombinatorElement { state, index, kind },
            Value::Undefined,
        )
    }

    fn begin_combinator_element(&mut self, state: ObjectId) -> Result<Option<ObjectId>> {
        let element = Value::Object(state);
        let already_called = self.get_named(&element, PROMISE_ALL_ALREADY_CALLED_PROPERTY)?;
        let Value::Bool(already_called) = already_called else {
            return Err(Error::runtime(
                "Promise combinator call state is not boolean",
            ));
        };
        if already_called {
            return Ok(None);
        }
        self.define_non_enumerable_object_property(
            state,
            PROMISE_ALL_ALREADY_CALLED_PROPERTY,
            Value::Bool(true),
        )?;
        let shared_state = self.get_named(&element, PROMISE_ALL_SHARED_STATE_PROPERTY)?;
        let Value::Object(shared_state) = shared_state else {
            return Err(Error::runtime(
                "Promise combinator shared state is not an object",
            ));
        };
        Ok(Some(shared_state))
    }

    fn eval_all_settled_element(
        &mut self,
        state: ObjectId,
        index: usize,
        fulfilled: bool,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let Some(shared_state) = self.begin_combinator_element(state)? else {
            return Ok(Value::Undefined);
        };
        let shared = Value::Object(shared_state);
        let values = self.get_named(&shared, PROMISE_ALL_VALUES_PROPERTY)?;
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let result = self.create_settlement_result(fulfilled, value)?;
        self.set_promise_all_value(&values, index, result)?;
        if self.change_promise_all_remaining(shared_state, false)? == 0 {
            let resolve = self.get_named(&shared, PROMISE_ALL_RESOLVE_PROPERTY)?;
            self.call_value(&resolve, &[values], Value::Undefined)?;
        }
        Ok(Value::Undefined)
    }

    fn eval_keyed_element(
        &mut self,
        state: ObjectId,
        index: usize,
        settled: Option<bool>,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let Some(shared_state) = self.begin_combinator_element(state)? else {
            return Ok(Value::Undefined);
        };
        let shared = Value::Object(shared_state);
        let values = self.get_named(&shared, PROMISE_ALL_VALUES_PROPERTY)?;
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let value = if let Some(fulfilled) = settled {
            self.create_settlement_result(fulfilled, value)?
        } else {
            value
        };
        self.set_promise_all_value(&values, index, value)?;
        if self.change_promise_all_remaining(shared_state, false)? == 0 {
            self.resolve_keyed_state(shared_state)?;
        }
        Ok(Value::Undefined)
    }

    fn resolve_keyed_state(&mut self, state: ObjectId) -> Result<()> {
        let shared = Value::Object(state);
        let keys = self.get_named(&shared, PROMISE_KEYED_KEYS_PROPERTY)?;
        let values = self.get_named(&shared, PROMISE_ALL_VALUES_PROPERTY)?;
        let result = self.create_keyed_result(&keys, &values)?;
        let resolve = self.get_named(&shared, PROMISE_ALL_RESOLVE_PROPERTY)?;
        self.call_value(&resolve, &[result], Value::Undefined)
            .map(|_value| ())
    }

    fn create_keyed_result(&mut self, keys: &Value, values: &Value) -> Result<Value> {
        let Value::Object(keys_id) = keys else {
            return Err(Error::runtime("Promise keyed keys state is not an array"));
        };
        let Some(length) = self.objects.array_len_if_array(*keys_id)? else {
            return Err(Error::runtime("Promise keyed keys state is not an array"));
        };
        let result = self
            .objects
            .create_with_exact_prototype(None, self.limits.max_objects)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, core::iter::once(&result))?;
        for index in 0..length {
            let index_name = index.to_string();
            let key = self.get_named(keys, &index_name)?;
            let value = self.get_named(values, &index_name)?;
            self.define_keyed_result_property(&result, &key, value)?;
        }
        Ok(result)
    }

    fn define_keyed_result_property(
        &mut self,
        result: &Value,
        key: &Value,
        value: Value,
    ) -> Result<()> {
        let mut property = self.dynamic_property_key(key)?;
        let update = PropertyUpdate::Data(DataPropertyUpdate::new(
            Some(value),
            Some(PropertyWritable::Yes),
            Some(PropertyEnumerable::Yes),
            Some(PropertyConfigurable::Yes),
        ));
        if self.semantic_define_own_property_update(result, &mut property, update)? {
            return Ok(());
        }
        Err(Error::type_error(
            "Promise keyed result property could not be defined",
        ))
    }

    fn eval_promise_any_reject_element(
        &mut self,
        state: ObjectId,
        index: usize,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let Some(shared_state) = self.begin_combinator_element(state)? else {
            return Ok(Value::Undefined);
        };
        let shared = Value::Object(shared_state);
        let errors = self.get_named(&shared, PROMISE_ALL_VALUES_PROPERTY)?;
        let reason = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        self.set_promise_all_value(&errors, index, reason)?;
        if self.change_promise_all_remaining(shared_state, false)? == 0 {
            let reject = self.get_named(&shared, PROMISE_ANY_REJECT_PROPERTY)?;
            self.reject_promise_any_errors(&reject, errors)?;
        }
        Ok(Value::Undefined)
    }

    fn reject_promise_any_errors(&mut self, reject: &Value, errors: Value) -> Result<()> {
        let aggregate = self.create_aggregate_error(errors)?;
        self.call_value(reject, &[aggregate], Value::Undefined)
            .map(|_value| ())
    }

    fn create_settlement_result(&mut self, fulfilled: bool, value: Value) -> Result<Value> {
        let (status, payload_name) = if fulfilled {
            (SETTLED_FULFILLED_STATUS, SETTLED_VALUE_PROPERTY)
        } else {
            (SETTLED_REJECTED_STATUS, SETTLED_REASON_PROPERTY)
        };
        let status_key = self.intern_property_key(SETTLED_STATUS_PROPERTY)?;
        let payload_key = self.intern_property_key(payload_name)?;
        let status = self.heap_string_value(status)?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            vec![
                ObjectPropertyInit::new(
                    status_key,
                    SETTLED_STATUS_PROPERTY,
                    status,
                    PropertyEnumerable::Yes,
                ),
                ObjectPropertyInit::new(payload_key, payload_name, value, PropertyEnumerable::Yes),
            ],
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}
