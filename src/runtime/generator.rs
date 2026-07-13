use std::collections::VecDeque;

mod async_generator;

use crate::{
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        async_trace::VmAsyncEdgeKind,
        call::RuntimeCallArgs,
        control::Completion,
        function::DetachedFunctionExecution,
        native::NativeFunctionKind,
        object::{
            DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
            PropertyKey, PropertyUpdate, PropertyWritable,
        },
        promise::PromiseId,
        trace::StrongEdgeReference,
        trace::StrongEdgeVisitor,
    },
    value::{FunctionId, ObjectId, Value},
};

const GENERATOR_RECEIVER_ERROR: &str = "generator method requires a compatible receiver";
const GENERATOR_EXECUTING_ERROR: &str = "generator is already executing";
const ITERATOR_NEXT_NAME: &str = "next";
const ITERATOR_RETURN_NAME: &str = "return";
const ITERATOR_THROW_NAME: &str = "throw";
const ITERATOR_RESULT_VALUE_NAME: &str = "value";
const ITERATOR_RESULT_DONE_NAME: &str = "done";
const ITERATOR_SYMBOL_DISPLAY: &str = "[Symbol.iterator]";
const ASYNC_ITERATOR_SYMBOL_DISPLAY: &str = "[Symbol.asyncIterator]";
const TO_STRING_TAG_SYMBOL_DISPLAY: &str = "[Symbol.toStringTag]";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const GENERATOR_TAG: &str = "Generator";
const ASYNC_GENERATOR_TAG: &str = "AsyncGenerator";
const ASYNC_ITERATOR_SYMBOL_PROPERTY: &str = "asyncIterator";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) struct GeneratorId(usize);

impl GeneratorId {
    pub(in crate::runtime) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
pub(in crate::runtime) struct GeneratorData {
    state: GeneratorState,
    asynchronous: bool,
    current_request: Option<AsyncGeneratorRequest>,
    requests: VecDeque<AsyncGeneratorRequest>,
}

#[derive(Debug)]
struct AsyncGeneratorRequest {
    promise: PromiseId,
    kind: GeneratorResumeKind,
    value: Value,
}

impl GeneratorData {
    pub(in crate::runtime) fn execution_frame_count(&self) -> Result<usize> {
        match &self.state {
            GeneratorState::Awaiting(awaiting) => awaiting.execution_frame_count(),
            GeneratorState::Suspended(execution) => execution.execution_frame_count(),
            GeneratorState::Executing | GeneratorState::Completed => Ok(0),
        }
    }

    pub(in crate::runtime) fn binding_count(&self) -> Result<usize> {
        match &self.state {
            GeneratorState::Awaiting(awaiting) => awaiting.binding_count(),
            GeneratorState::Suspended(execution) => execution.binding_count(),
            GeneratorState::Executing | GeneratorState::Completed => Ok(0),
        }
    }

    pub(in crate::runtime) fn cache_entry_count(&self) -> Result<usize> {
        match &self.state {
            GeneratorState::Awaiting(awaiting) => awaiting.cache_entry_count(),
            GeneratorState::Suspended(execution) => execution.cache_entry_count(),
            GeneratorState::Executing | GeneratorState::Completed => Ok(0),
        }
    }

    pub(in crate::runtime) fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        match &self.state {
            GeneratorState::Awaiting(awaiting) => awaiting.visit_strong_edges(visitor),
            GeneratorState::Suspended(execution) => {
                execution.visit_strong_edges(visitor, VmAsyncEdgeKind::GeneratorState)
            }
            GeneratorState::Executing | GeneratorState::Completed => Ok(()),
        }?;
        if let Some(request) = &self.current_request {
            request.visit_strong_edges(visitor)?;
        }
        for request in &self.requests {
            request.visit_strong_edges(visitor)?;
        }
        Ok(())
    }

    fn request_count(&self) -> usize {
        self.requests.len() + usize::from(self.current_request.is_some())
    }
}

impl AsyncGeneratorRequest {
    fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        visitor.visit(
            VmAsyncEdgeKind::GeneratorState,
            StrongEdgeReference::Promise(self.promise),
        )?;
        visitor.visit(
            VmAsyncEdgeKind::GeneratorState,
            StrongEdgeReference::Value(&self.value),
        )
    }
}

#[derive(Debug)]
enum GeneratorState {
    Suspended(DetachedFunctionExecution),
    Awaiting(AsyncGeneratorAwaitState),
    Executing,
    Completed,
}

#[derive(Debug)]
enum AsyncGeneratorAwaitState {
    Body(DetachedFunctionExecution),
    DelegatedYield(DetachedFunctionExecution),
    ResumeReturn(DetachedFunctionExecution),
    Yield(DetachedFunctionExecution),
    Return,
}

impl AsyncGeneratorAwaitState {
    const fn execution(&self) -> Option<&DetachedFunctionExecution> {
        match self {
            Self::Body(execution)
            | Self::DelegatedYield(execution)
            | Self::ResumeReturn(execution)
            | Self::Yield(execution) => Some(execution),
            Self::Return => None,
        }
    }

    fn execution_frame_count(&self) -> Result<usize> {
        self.execution()
            .map_or(Ok(0), DetachedFunctionExecution::execution_frame_count)
    }

    fn binding_count(&self) -> Result<usize> {
        self.execution()
            .map_or(Ok(0), DetachedFunctionExecution::binding_count)
    }

    fn cache_entry_count(&self) -> Result<usize> {
        self.execution()
            .map_or(Ok(0), DetachedFunctionExecution::cache_entry_count)
    }

    fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        let Some(execution) = self.execution() else {
            return Ok(());
        };
        execution.visit_strong_edges(visitor, VmAsyncEdgeKind::GeneratorState)
    }
}

enum AsyncGeneratorStep {
    Settled(GeneratorState, Value),
    Awaiting(AsyncGeneratorAwaitState, PromiseId),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum GeneratorResumeKind {
    Next,
    Return,
    Throw,
}

impl Context {
    pub(in crate::runtime) fn suspended_generator_binding_count(&self) -> Result<usize> {
        self.generators
            .iter()
            .try_fold(0_usize, |count, generator| {
                count
                    .checked_add(generator.binding_count()?)
                    .ok_or_else(|| Error::limit("generator binding count overflowed"))
            })
    }

    pub(in crate::runtime) fn suspended_generator_execution_frame_count(&self) -> Result<usize> {
        self.generators
            .iter()
            .try_fold(0_usize, |count, generator| {
                count
                    .checked_add(generator.execution_frame_count()?)
                    .ok_or_else(|| Error::limit("generator execution frame count overflowed"))
            })
    }

    pub(in crate::runtime) fn suspended_generator_cache_entry_count(&self) -> Result<usize> {
        self.generators
            .iter()
            .try_fold(0_usize, |count, generator| {
                count
                    .checked_add(generator.cache_entry_count()?)
                    .ok_or_else(|| Error::limit("generator cache entry count overflowed"))
            })
    }

    pub(in crate::runtime) fn async_generator_request_count(&self) -> Result<usize> {
        self.generators
            .iter()
            .try_fold(0_usize, |count, generator| {
                count
                    .checked_add(generator.request_count())
                    .ok_or_else(|| Error::limit("async generator request count overflowed"))
            })
    }

    pub(in crate::runtime) fn create_generator_object(
        &mut self,
        function: FunctionId,
        execution: DetachedFunctionExecution,
    ) -> Result<Value> {
        let asynchronous = self.function(function)?.kind.is_async_generator();
        let prototype = self.generator_instance_prototype(function)?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = object else {
            return Err(Error::runtime("generator object creation failed"));
        };
        self.generators.reserve_insert()?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        let id = GeneratorId(self.generators.next_index());
        let data = GeneratorData {
            state: GeneratorState::Suspended(execution),
            asynchronous,
            current_request: None,
            requests: VecDeque::new(),
        };
        if let Err(error) = self.generators.insert_at_next(id.index(), data) {
            self.storage_ledger
                .release_count(VmStorageKind::Association, 1)?;
            return Err(error);
        }
        self.bind_generator_object(object_id, id)?;
        Ok(Value::Object(object_id))
    }

    fn bind_generator_object(&mut self, object: ObjectId, generator: GeneratorId) -> Result<()> {
        let required = object
            .index()
            .checked_add(1)
            .ok_or_else(|| Error::limit("generator object slot index overflowed"))?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        if self.generator_object_slots.len() < required {
            self.generator_object_slots.resize(required, None);
        }
        let Some(slot) = self.generator_object_slots.get_mut(object.index()) else {
            self.storage_ledger
                .release_count(VmStorageKind::Association, 1)?;
            return Err(Error::runtime("generator object slot disappeared"));
        };
        *slot = Some(generator);
        Ok(())
    }

    fn generator_instance_prototype(&mut self, function: FunctionId) -> Result<ObjectId> {
        let prototype = self.function_prototype_value(function)?;
        if let Value::Object(id) = prototype {
            return Ok(id);
        }
        if self.function(function)?.kind.is_async_generator() {
            self.async_generator_prototype_id()
        } else {
            self.generator_prototype_id()
        }
    }

    pub(in crate::runtime) fn generator_prototype_id(&mut self) -> Result<ObjectId> {
        if let Some(prototype) = self.realm.generator_prototype {
            return Ok(prototype);
        }
        // %GeneratorPrototype% inherits the iterator helpers through
        // %Iterator.prototype% per the specification prototype chain.
        let iterator_prototype = self.iterator_prototype_object_id()?;
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            Some(iterator_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype) = value else {
            return Err(Error::runtime("generator prototype creation failed"));
        };
        self.install_generator_prototype(prototype)?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        self.realm.generator_prototype = Some(prototype);
        Ok(prototype)
    }

    pub(in crate::runtime) fn generator_function_prototype_value(&mut self) -> Result<Value> {
        if let Some(prototype) = self.realm.generator_function_prototype {
            return Ok(Value::Object(prototype));
        }
        self.generator_function_constructor_value()?;
        self.realm
            .generator_function_prototype
            .map(Value::Object)
            .ok_or_else(|| Error::runtime("generator function prototype disappeared"))
    }

    pub(in crate::runtime) fn generator_function_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::GeneratorFunction) {
            return Ok(Value::NativeFunction(id));
        }
        self.function_constructor_value()?;
        let prototype = self.create_generator_function_prototype()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let name = self.native_function_name_value(NativeFunctionKind::GeneratorFunction)?;
        self.push_native_function_with_id(
            id,
            NativeFunctionKind::GeneratorFunction,
            Value::Object(prototype),
            name,
        )?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.define_property(
            prototype,
            constructor_key,
            "constructor",
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(constructor.clone()),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        Ok(constructor)
    }

    fn create_generator_function_prototype(&mut self) -> Result<ObjectId> {
        let function_prototype = self.function_constructor_prototype_value()?;
        let Value::Object(function_prototype) = function_prototype else {
            return Err(Error::runtime("Function prototype is not an object"));
        };
        let generator_prototype = self.generator_prototype_id()?;
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            Some(function_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype) = value else {
            return Err(Error::runtime(
                "generator function prototype creation failed",
            ));
        };
        let prototype_key = self.intern_property_key("prototype")?;
        self.objects.define_property(
            prototype,
            prototype_key,
            "prototype",
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Object(generator_prototype)),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.objects.define_property(
            generator_prototype,
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
        self.define_builtin_to_string_tag(prototype, "GeneratorFunction")?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 1)?;
        self.realm.generator_function_prototype = Some(prototype);
        Ok(prototype)
    }

    fn install_generator_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in [
            (ITERATOR_NEXT_NAME, NativeFunctionKind::GeneratorNext),
            (ITERATOR_RETURN_NAME, NativeFunctionKind::GeneratorReturn),
            (ITERATOR_THROW_NAME, NativeFunctionKind::GeneratorThrow),
        ] {
            let method = self.create_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        if let Some(symbol) = self.iterator_symbol() {
            let method =
                self.create_native_function(NativeFunctionKind::IteratorSelf, Value::Undefined)?;
            self.objects.define_property(
                prototype,
                PropertyKey::symbol(symbol),
                ITERATOR_SYMBOL_DISPLAY,
                PropertyUpdate::Data(DataPropertyUpdate::new(
                    Some(method),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                )),
                self.limits.max_object_properties,
            )?;
        }
        let tag = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(tag) = tag else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let tag_value = self.heap_string_value(GENERATOR_TAG)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(tag.id()),
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

    pub(in crate::runtime) fn eval_generator_resume(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        kind: GeneratorResumeKind,
        asynchronous: bool,
    ) -> Result<Value> {
        if asynchronous {
            return self.enqueue_async_generator_request(args, this_value, kind);
        }
        let id = self.generator_id_from_this(this_value)?;
        if self.generator_mut(id)?.asynchronous {
            return Err(Error::type_error(GENERATOR_RECEIVER_ERROR));
        }
        let state = {
            let generator = self.generator_mut(id)?;
            std::mem::replace(&mut generator.state, GeneratorState::Executing)
        };
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let result = self.resume_generator_state(state, kind, value);
        if result.is_err() {
            self.generator_mut(id)?.state = GeneratorState::Completed;
        }
        let (state, value) = result?;
        self.generator_mut(id)?.state = state;
        Ok(value)
    }

    fn resume_generator_state(
        &mut self,
        state: GeneratorState,
        kind: GeneratorResumeKind,
        value: Value,
    ) -> Result<(GeneratorState, Value)> {
        match state {
            GeneratorState::Awaiting(_) | GeneratorState::Executing => {
                Err(Error::type_error(GENERATOR_EXECUTING_ERROR))
            }
            GeneratorState::Completed => self.resume_completed_generator(kind, value),
            GeneratorState::Suspended(execution) => {
                let function = execution.function();
                let resume = match kind {
                    GeneratorResumeKind::Next => Completion::Normal(value),
                    GeneratorResumeKind::Return => Completion::Return(value),
                    GeneratorResumeKind::Throw => Completion::Throw(value),
                };
                let completion = self.resume_function_execution(execution, resume)?;
                self.finish_generator_completion(function, completion)
            }
        }
    }

    fn resume_completed_generator(
        &mut self,
        kind: GeneratorResumeKind,
        value: Value,
    ) -> Result<(GeneratorState, Value)> {
        match kind {
            GeneratorResumeKind::Throw => Err(Error::javascript(value)),
            GeneratorResumeKind::Next => Ok((
                GeneratorState::Completed,
                self.create_generator_result(Value::Undefined, true)?,
            )),
            GeneratorResumeKind::Return => Ok((
                GeneratorState::Completed,
                self.create_generator_result(value, true)?,
            )),
        }
    }

    fn finish_generator_completion(
        &mut self,
        function: FunctionId,
        completion: Completion,
    ) -> Result<(GeneratorState, Value)> {
        match completion {
            Completion::Yielded(value) => {
                let execution = self.detach_function_execution(function)?;
                let result = self.create_generator_result(value, false)?;
                Ok((GeneratorState::Suspended(execution), result))
            }
            Completion::DelegatedYield(delegated) => {
                let result = delegated.into_iterator_result()?;
                if self.semantic_object_ref(&result)?.is_none() {
                    return Err(Error::runtime(
                        "delegated generator result is not an object",
                    ));
                }
                let execution = self.detach_function_execution(function)?;
                Ok((GeneratorState::Suspended(execution), result))
            }
            Completion::Normal(_) => Ok((
                GeneratorState::Completed,
                self.create_generator_result(Value::Undefined, true)?,
            )),
            Completion::Return(value) | Completion::ReturnDirect(value) => Ok((
                GeneratorState::Completed,
                self.create_generator_result(value, true)?,
            )),
            Completion::Throw(value) => Err(Error::javascript(value)),
            Completion::TailCall(_) => Err(Error::runtime("tail call escaped generator function")),
            Completion::Break { .. } | Completion::Continue { .. } => {
                Err(Error::runtime("invalid generator completion"))
            }
            Completion::Suspended(_) => Err(Error::runtime(
                "generator execution suspended on an async Promise",
            )),
            Completion::GeneratorStart => Err(Error::runtime(
                "generator restarted from its initial suspension",
            )),
        }
    }

    fn create_generator_result(&mut self, value: Value, done: bool) -> Result<Value> {
        let value_key = self.intern_property_key(ITERATOR_RESULT_VALUE_NAME)?;
        let done_key = self.intern_property_key(ITERATOR_RESULT_DONE_NAME)?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create(
            vec![
                ObjectPropertyInit::new(
                    value_key,
                    ITERATOR_RESULT_VALUE_NAME,
                    value,
                    PropertyEnumerable::Yes,
                ),
                ObjectPropertyInit::new(
                    done_key,
                    ITERATOR_RESULT_DONE_NAME,
                    Value::Bool(done),
                    PropertyEnumerable::Yes,
                ),
            ],
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn generator_id_from_this(&self, this_value: &Value) -> Result<GeneratorId> {
        let Value::Object(object) = this_value else {
            return Err(Error::type_error(GENERATOR_RECEIVER_ERROR));
        };
        self.generator_object_slots
            .get(object.index())
            .copied()
            .flatten()
            .ok_or_else(|| Error::type_error(GENERATOR_RECEIVER_ERROR))
    }

    fn generator_mut(&mut self, id: GeneratorId) -> Result<&mut GeneratorData> {
        self.generators
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("generator storage disappeared"))
    }
}
