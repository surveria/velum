use crate::{
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        call::RuntimeCallArgs,
        control::{Completion, Suspension, runtime_exception_value},
        native::NativeFunctionKind,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate, PropertyWritable,
        },
        promise::{PromiseId, PromiseReaction},
    },
    value::{FunctionId, ObjectId, Value},
};

use super::{
    ASYNC_GENERATOR_TAG, ASYNC_ITERATOR_SYMBOL_DISPLAY, ASYNC_ITERATOR_SYMBOL_PROPERTY,
    AsyncGeneratorAwaitState, AsyncGeneratorRequest, AsyncGeneratorStep, GENERATOR_EXECUTING_ERROR,
    GENERATOR_RECEIVER_ERROR, GeneratorId, GeneratorResumeKind, GeneratorState, ITERATOR_NEXT_NAME,
    ITERATOR_RETURN_NAME, ITERATOR_THROW_NAME, TO_STRING_TAG_PROPERTY,
    TO_STRING_TAG_SYMBOL_DISPLAY,
};

impl Context {
    pub(in crate::runtime) fn async_generator_function_prototype_value(&mut self) -> Result<Value> {
        let Value::NativeFunction(id) = self.async_generator_function_constructor_value()? else {
            return Err(Error::runtime(
                "AsyncGeneratorFunction constructor value is not native",
            ));
        };
        Ok(self.native_function(id)?.properties().prototype())
    }

    pub(in crate::runtime) fn create_async_generator_function_prototype(
        &mut self,
    ) -> Result<ObjectId> {
        if let Some(prototype) = self.realm.async_generator_function_prototype {
            return Ok(prototype);
        }
        let async_function_prototype = self.async_function_constructor_prototype_value()?;
        let Value::Object(async_function_prototype) = async_function_prototype else {
            return Err(Error::runtime("AsyncFunction prototype is not an object"));
        };
        let async_generator_prototype = self.async_generator_prototype_id()?;
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            Some(async_function_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype) = value else {
            return Err(Error::runtime(
                "async generator function prototype creation failed",
            ));
        };
        let prototype_key = self.intern_property_key("prototype")?;
        self.objects.define_property(
            prototype,
            prototype_key,
            "prototype",
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Object(async_generator_prototype)),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.objects.define_property(
            async_generator_prototype,
            constructor_key,
            "constructor",
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Object(prototype)),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.install_to_string_tag(prototype, "AsyncGeneratorFunction")?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        self.realm.async_generator_function_prototype = Some(prototype);
        Ok(prototype)
    }

    pub(in crate::runtime) fn install_async_generator_function_constructor(
        &mut self,
        prototype: ObjectId,
        constructor: Value,
    ) -> Result<()> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.define_property(
            prototype,
            constructor_key,
            "constructor",
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(constructor),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime) fn async_generator_prototype_id(&mut self) -> Result<ObjectId> {
        if let Some(prototype) = self.realm.async_generator_prototype {
            return Ok(prototype);
        }
        let parent = self.async_iterator_prototype_id()?;
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            Some(parent),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype) = value else {
            return Err(Error::runtime("async generator prototype creation failed"));
        };
        self.install_async_generator_prototype(prototype)?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        self.realm.async_generator_prototype = Some(prototype);
        Ok(prototype)
    }

    fn async_iterator_prototype_id(&mut self) -> Result<ObjectId> {
        if let Some(prototype) = self.realm.async_iterator_prototype {
            return Ok(prototype);
        }
        let constructor_key = self.object_constructor_property_key()?;
        let object_prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let value = self.objects.create_with_prototype(
            Some(object_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype) = value else {
            return Err(Error::runtime("async iterator prototype creation failed"));
        };
        let symbol = self.well_known_symbol(ASYNC_ITERATOR_SYMBOL_PROPERTY)?;
        let method =
            self.create_native_function(NativeFunctionKind::IteratorSelf, Value::Undefined)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol),
            ASYNC_ITERATOR_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(method),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        self.realm.async_iterator_prototype = Some(prototype);
        Ok(prototype)
    }

    fn install_async_generator_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in [
            (ITERATOR_NEXT_NAME, NativeFunctionKind::AsyncGeneratorNext),
            (
                ITERATOR_RETURN_NAME,
                NativeFunctionKind::AsyncGeneratorReturn,
            ),
            (ITERATOR_THROW_NAME, NativeFunctionKind::AsyncGeneratorThrow),
        ] {
            let method = self.create_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        self.install_to_string_tag(prototype, ASYNC_GENERATOR_TAG)
    }

    fn install_to_string_tag(&mut self, object: ObjectId, value: &str) -> Result<()> {
        let tag = self.well_known_symbol(TO_STRING_TAG_PROPERTY)?;
        let tag_value = self.heap_string_value(value)?;
        self.objects.define_property(
            object,
            PropertyKey::symbol(tag),
            TO_STRING_TAG_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(tag_value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn well_known_symbol(&mut self, property: &str) -> Result<crate::storage::symbol::SymbolId> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, property)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime(
                "well-known Symbol property is not initialized",
            ));
        };
        Ok(symbol.id())
    }

    pub(super) fn enqueue_async_generator_request(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        kind: GeneratorResumeKind,
    ) -> Result<Value> {
        let (promise, object) = self.create_pending_promise()?;
        let id = match self.generator_id_from_this(this_value) {
            Ok(id) if self.generator_mut(id)?.asynchronous => id,
            Ok(_) | Err(_) => {
                let error = Error::type_error(GENERATOR_RECEIVER_ERROR);
                let Some(reason) = runtime_exception_value(self, &error)? else {
                    return Err(error);
                };
                self.reject_promise(promise, reason)?;
                return Ok(object);
            }
        };
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        self.generator_mut(id)?
            .requests
            .push_back(AsyncGeneratorRequest {
                promise,
                kind,
                value,
            });
        self.process_async_generator_queue(id)?;
        Ok(object)
    }

    fn process_async_generator_queue(&mut self, id: GeneratorId) -> Result<()> {
        loop {
            let action = {
                let generator = self.generator_mut(id)?;
                if matches!(
                    generator.state,
                    GeneratorState::Awaiting(_) | GeneratorState::Executing
                ) {
                    return Ok(());
                }
                if generator.current_request.is_none() {
                    generator.current_request = generator.requests.pop_front();
                }
                let Some(request) = generator.current_request.as_ref() else {
                    return Ok(());
                };
                let kind = request.kind;
                let value = request.value.clone();
                let state = std::mem::replace(&mut generator.state, GeneratorState::Executing);
                (state, kind, value)
            };
            let step = self.resume_async_generator_state(action.0, action.1, action.2);
            if self.apply_async_generator_step(id, step)? {
                return Ok(());
            }
        }
    }

    fn resume_async_generator_state(
        &mut self,
        state: GeneratorState,
        kind: GeneratorResumeKind,
        value: Value,
    ) -> Result<AsyncGeneratorStep> {
        match state {
            GeneratorState::Awaiting(_) | GeneratorState::Executing => {
                Err(Error::runtime(GENERATOR_EXECUTING_ERROR))
            }
            GeneratorState::Completed => {
                if kind == GeneratorResumeKind::Return {
                    let Completion::Suspend(Suspension::Await(awaited)) =
                        self.eval_bytecode_await(value)?
                    else {
                        return Err(Error::runtime(
                            "completed async generator return did not await a Promise",
                        ));
                    };
                    return Ok(AsyncGeneratorStep::Awaiting(
                        AsyncGeneratorAwaitState::Return,
                        awaited,
                    ));
                }
                let (state, value) = self.resume_completed_generator(kind, value)?;
                Ok(AsyncGeneratorStep::Settled(state, value))
            }
            GeneratorState::Suspended(execution) => {
                let function = execution.function();
                if kind == GeneratorResumeKind::Return && !execution.has_yield_delegate() {
                    let awaited = match self.eval_bytecode_await(value) {
                        Ok(Completion::Suspend(Suspension::Await(awaited))) => awaited,
                        Ok(completion) => {
                            return Err(Error::runtime(format!(
                                "async generator return resumption produced {completion:?}"
                            )));
                        }
                        Err(error) => {
                            let Some(reason) = runtime_exception_value(self, &error)? else {
                                return Err(error);
                            };
                            let completion = self
                                .resume_function_execution(execution, Completion::Throw(reason))?;
                            return self.finish_async_generator_completion(function, completion);
                        }
                    };
                    return Ok(AsyncGeneratorStep::Awaiting(
                        AsyncGeneratorAwaitState::ResumeReturn(execution),
                        awaited,
                    ));
                }
                let resume = match kind {
                    GeneratorResumeKind::Next => Completion::Normal(value),
                    GeneratorResumeKind::Return => Completion::Return(value),
                    GeneratorResumeKind::Throw => Completion::Throw(value),
                };
                let completion = self.resume_function_execution(execution, resume)?;
                self.finish_async_generator_completion(function, completion)
            }
        }
    }

    fn apply_async_generator_step(
        &mut self,
        id: GeneratorId,
        step: Result<AsyncGeneratorStep>,
    ) -> Result<bool> {
        match step {
            Ok(AsyncGeneratorStep::Settled(state, value)) => {
                self.generator_mut(id)?.state = state;
                let promise = self.take_async_generator_request(id)?;
                self.resolve_promise(promise, value)?;
                Ok(false)
            }
            Ok(AsyncGeneratorStep::Awaiting(awaiting, awaited)) => {
                self.generator_mut(id)?.state = GeneratorState::Awaiting(awaiting);
                if let Err(error) = self
                    .add_promise_reaction(awaited, PromiseReaction::awaiting_async_generator(id))
                {
                    self.cancel_async_generator_await(id)?;
                    return Err(error);
                }
                Ok(true)
            }
            Err(error) => {
                self.generator_mut(id)?.state = GeneratorState::Completed;
                let promise = self.take_async_generator_request(id)?;
                let Some(reason) = runtime_exception_value(self, &error)? else {
                    return Err(error);
                };
                self.reject_promise(promise, reason)?;
                Ok(false)
            }
        }
    }

    fn take_async_generator_request(&mut self, id: GeneratorId) -> Result<PromiseId> {
        let request = self
            .generator_mut(id)?
            .current_request
            .take()
            .ok_or_else(|| Error::runtime("async generator request disappeared"))?;
        self.storage_ledger
            .release_count(VmStorageKind::Association, 1)?;
        Ok(request.promise)
    }

    pub(in crate::runtime) fn resume_async_generator_await(
        &mut self,
        id: GeneratorId,
        resume: Completion,
    ) -> Result<()> {
        let state = {
            let generator = self.generator_mut(id)?;
            std::mem::replace(&mut generator.state, GeneratorState::Executing)
        };
        let GeneratorState::Awaiting(awaiting) = state else {
            return Err(Error::runtime("async generator await state disappeared"));
        };
        let step = match (awaiting, resume) {
            (AsyncGeneratorAwaitState::Body(execution), resume) => {
                let function = execution.function();
                match self.resume_function_execution(execution, resume) {
                    Ok(completion) => self.finish_async_generator_completion(function, completion),
                    Err(error) => Err(error),
                }
            }
            (
                AsyncGeneratorAwaitState::Yield(execution)
                | AsyncGeneratorAwaitState::DelegatedYield(execution),
                Completion::Normal(value),
            ) => {
                let result = self.create_generator_result(value, false)?;
                Ok(AsyncGeneratorStep::Settled(
                    GeneratorState::Suspended(execution),
                    result,
                ))
            }
            (
                AsyncGeneratorAwaitState::Return | AsyncGeneratorAwaitState::DelegatedYield(_),
                Completion::Throw(value),
            ) => Err(Error::javascript(value)),
            (AsyncGeneratorAwaitState::ResumeReturn(execution), resume) => {
                let function = execution.function();
                let resume = match resume {
                    Completion::Normal(value) => Completion::ReturnDirect(value),
                    Completion::Throw(value) => Completion::Throw(value),
                    completion => {
                        return Err(Error::runtime(format!(
                            "invalid async generator return resumption {completion:?}"
                        )));
                    }
                };
                match self.resume_function_execution(execution, resume) {
                    Ok(completion) => self.finish_async_generator_completion(function, completion),
                    Err(error) => Err(error),
                }
            }
            (AsyncGeneratorAwaitState::Yield(execution), Completion::Throw(value)) => {
                let function = execution.function();
                match self.resume_function_execution(execution, Completion::Throw(value)) {
                    Ok(completion) => self.finish_async_generator_completion(function, completion),
                    Err(error) => Err(error),
                }
            }
            (AsyncGeneratorAwaitState::Return, Completion::Normal(value)) => {
                let result = self.create_generator_result(value, true)?;
                Ok(AsyncGeneratorStep::Settled(
                    GeneratorState::Completed,
                    result,
                ))
            }
            (_, completion) => Err(Error::runtime(format!(
                "async generator received invalid await completion {completion:?}"
            ))),
        };
        if !self.apply_async_generator_step(id, step)? {
            self.process_async_generator_queue(id)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn cancel_async_generator_await(
        &mut self,
        id: GeneratorId,
    ) -> Result<()> {
        let (execution, requests) = {
            let generator = self.generator_mut(id)?;
            let state = std::mem::replace(&mut generator.state, GeneratorState::Completed);
            let execution = match state {
                GeneratorState::Awaiting(
                    AsyncGeneratorAwaitState::Body(execution)
                    | AsyncGeneratorAwaitState::DelegatedYield(execution)
                    | AsyncGeneratorAwaitState::ResumeReturn(execution)
                    | AsyncGeneratorAwaitState::Yield(execution),
                )
                | GeneratorState::Suspended(execution) => Some(execution),
                GeneratorState::Awaiting(AsyncGeneratorAwaitState::Return)
                | GeneratorState::Executing
                | GeneratorState::Completed => None,
            };
            let request_count = generator.request_count();
            generator.current_request = None;
            generator.requests.clear();
            (execution, request_count)
        };
        if let Some(execution) = execution {
            execution.cancel_storage(&self.storage_ledger)?;
        }
        self.storage_ledger
            .release_count(VmStorageKind::Association, requests)
    }

    fn finish_async_generator_completion(
        &mut self,
        function: FunctionId,
        completion: Completion,
    ) -> Result<AsyncGeneratorStep> {
        match completion {
            Completion::Suspend(Suspension::Await(awaited)) => {
                let execution = self.detach_function_execution(function)?;
                Ok(AsyncGeneratorStep::Awaiting(
                    AsyncGeneratorAwaitState::Body(execution),
                    awaited,
                ))
            }
            Completion::Suspend(Suspension::Yield(value)) => {
                let Completion::Suspend(Suspension::Await(awaited)) =
                    self.eval_bytecode_await(value)?
                else {
                    return Err(Error::runtime(
                        "async generator yield did not await a Promise",
                    ));
                };
                let execution = self.detach_function_execution(function)?;
                Ok(AsyncGeneratorStep::Awaiting(
                    AsyncGeneratorAwaitState::Yield(execution),
                    awaited,
                ))
            }
            Completion::Suspend(Suspension::DelegatedYield(delegated)) => {
                let (value, await_before_yield) = delegated.into_async_value()?;
                if await_before_yield {
                    let Completion::Suspend(Suspension::Await(awaited)) =
                        self.eval_bytecode_await(value)?
                    else {
                        return Err(Error::runtime(
                            "async generator delegated yield did not await a Promise",
                        ));
                    };
                    let execution = self.detach_function_execution(function)?;
                    return Ok(AsyncGeneratorStep::Awaiting(
                        AsyncGeneratorAwaitState::DelegatedYield(execution),
                        awaited,
                    ));
                }
                let execution = self.detach_function_execution(function)?;
                let result = self.create_generator_result(value, false)?;
                Ok(AsyncGeneratorStep::Settled(
                    GeneratorState::Suspended(execution),
                    result,
                ))
            }
            Completion::Return(value) => {
                let Completion::Suspend(Suspension::Await(awaited)) =
                    self.eval_bytecode_await(value)?
                else {
                    return Err(Error::runtime(
                        "async generator return did not await a Promise",
                    ));
                };
                Ok(AsyncGeneratorStep::Awaiting(
                    AsyncGeneratorAwaitState::Return,
                    awaited,
                ))
            }
            completion => {
                let (state, value) = self.finish_generator_completion(function, completion)?;
                Ok(AsyncGeneratorStep::Settled(state, value))
            }
        }
    }
}
