use crate::{
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        async_trace::VmAsyncEdgeKind,
        call::RuntimeCallArgs,
        collections::{CollectionId, CollectionKind},
        control::{Completion, runtime_exception_value},
        native::{
            ASYNC_DISPOSABLE_STACK_NAME, AsyncDisposableStackFunctionKind, NativeFunctionKind,
            OBJECT_CONSTRUCTOR_PROPERTY,
        },
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable,
        },
        promise::{PromiseId, PromiseReaction},
        property::DynamicPropertyKey,
        roots::{DirectRootVisitor, VmRootKind},
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::{ObjectId, Value},
};

const ASYNC_DISPOSE_SYMBOL_DISPLAY: &str = "[Symbol.asyncDispose]";
const ASYNC_DISPOSE_SYMBOL_PROPERTY: &str = "asyncDispose";
const DISPOSED_ERROR: &str = "AsyncDisposableStack is already disposed";
const DISPOSE_SYMBOL_DISPLAY: &str = "[Symbol.dispose]";
const DISPOSE_SYMBOL_PROPERTY: &str = "dispose";
const RECEIVER_ERROR: &str = "method requires an AsyncDisposableStack receiver";
const TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";

#[derive(Debug, Clone)]
enum AsyncDisposableResource {
    Use {
        value: Value,
        method: Value,
        await_result: bool,
    },
    Adopt {
        value: Value,
        method: Value,
    },
    Defer {
        method: Value,
    },
    Nullish,
}

impl AsyncDisposableResource {
    fn visit_edges<V: StrongEdgeVisitor<VmAsyncEdgeKind>>(
        &self,
        visitor: &mut V,
        kind: VmAsyncEdgeKind,
    ) -> Result<()> {
        let mut visit = |value: &Value| visitor.visit(kind, StrongEdgeReference::Value(value));
        match self {
            Self::Use { value, method, .. } | Self::Adopt { value, method } => {
                visit(value)?;
                visit(method)
            }
            Self::Defer { method } => visit(method),
            Self::Nullish => Ok(()),
        }
    }

    fn root_values(&self) -> impl Iterator<Item = &Value> {
        let (first, second) = match self {
            Self::Use { value, method, .. } | Self::Adopt { value, method } => {
                (Some(value), Some(method))
            }
            Self::Defer { method } => (Some(method), None),
            Self::Nullish => (None, None),
        };
        first.into_iter().chain(second)
    }
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct AsyncDisposableStackData {
    disposed: bool,
    resources: Vec<AsyncDisposableResource>,
}

impl AsyncDisposableStackData {
    pub(in crate::runtime) const fn new() -> Self {
        Self {
            disposed: false,
            resources: Vec::new(),
        }
    }

    pub(in crate::runtime) const fn resource_count(&self) -> usize {
        self.resources.len()
    }

    pub(in crate::runtime) fn visit_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        for resource in &self.resources {
            resource.visit_edges(visitor, VmAsyncEdgeKind::CollectionEntry)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(in crate::runtime) struct AsyncDisposableStackContinuation {
    result_promise: PromiseId,
    resources: Vec<AsyncDisposableResource>,
    thrown: Option<Value>,
}

impl AsyncDisposableStackContinuation {
    const fn new(result_promise: PromiseId, resources: Vec<AsyncDisposableResource>) -> Self {
        Self {
            result_promise,
            resources,
            thrown: None,
        }
    }

    pub(in crate::runtime) fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        visitor.visit(
            VmAsyncEdgeKind::PromiseReaction,
            StrongEdgeReference::Promise(self.result_promise),
        )?;
        for resource in &self.resources {
            resource.visit_edges(visitor, VmAsyncEdgeKind::PromiseReaction)?;
        }
        if let Some(thrown) = &self.thrown {
            visitor.visit(
                VmAsyncEdgeKind::PromiseReaction,
                StrongEdgeReference::Value(thrown),
            )?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        visitor.visit_promise(VmRootKind::QueuedJob, self.result_promise)?;
        for value in self
            .resources
            .iter()
            .flat_map(AsyncDisposableResource::root_values)
        {
            visitor.visit_value(VmRootKind::QueuedJob, value)?;
        }
        if let Some(thrown) = &self.thrown {
            visitor.visit_value(VmRootKind::QueuedJob, thrown)?;
        }
        Ok(())
    }
}

enum AsyncDisposalDrive {
    Await(PromiseId),
    Complete,
}

impl Context {
    pub(in crate::runtime) fn async_disposable_stack_constructor_value(&mut self) -> Result<Value> {
        let kind =
            NativeFunctionKind::AsyncDisposableStack(AsyncDisposableStackFunctionKind::Constructor);
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_property(
            None,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor.clone(),
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let name = self.native_function_name_value(kind)?;
        self.push_native_function_with_id(id, kind, Value::Object(prototype), name)?;
        self.install_async_disposable_stack_prototype(prototype)?;
        self.insert_global_builtin(ASYNC_DISPOSABLE_STACK_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime) fn construct_async_disposable_stack(&mut self) -> Result<Value> {
        let prototype = self.async_disposable_stack_intrinsic_prototype()?;
        self.create_async_disposable_stack_with_prototype(prototype)
    }

    pub(in crate::runtime) fn construct_async_disposable_stack_with_new_target(
        &mut self,
        new_target: &Value,
    ) -> Result<Value> {
        let prototype = self.constructor_instance_prototype_with_default(
            new_target,
            NativeFunctionKind::AsyncDisposableStack(AsyncDisposableStackFunctionKind::Constructor),
        )?;
        self.create_async_disposable_stack_with_prototype(prototype)
    }

    fn async_disposable_stack_intrinsic_prototype(&mut self) -> Result<ObjectId> {
        let constructor = self.async_disposable_stack_constructor_value()?;
        let Value::NativeFunction(id) = constructor else {
            return Err(Error::runtime(
                "AsyncDisposableStack constructor disappeared",
            ));
        };
        let Value::Object(prototype) = self.native_function(id)?.properties().prototype() else {
            return Err(Error::runtime(
                "AsyncDisposableStack prototype is not an object",
            ));
        };
        Ok(prototype)
    }

    fn create_async_disposable_stack_with_prototype(
        &mut self,
        prototype: ObjectId,
    ) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object) = value else {
            return Err(Error::runtime("AsyncDisposableStack allocation failed"));
        };
        let collection = self.create_collection(CollectionKind::AsyncDisposableStack)?;
        self.bind_collection_object(object, CollectionKind::AsyncDisposableStack, collection)?;
        Ok(Value::Object(object))
    }

    fn install_async_disposable_stack_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        let methods = [
            ("adopt", AsyncDisposableStackFunctionKind::Adopt),
            ("defer", AsyncDisposableStackFunctionKind::Defer),
            (
                "disposeAsync",
                AsyncDisposableStackFunctionKind::DisposeAsync,
            ),
            ("move", AsyncDisposableStackFunctionKind::Move),
            ("use", AsyncDisposableStackFunctionKind::Use),
        ];
        let mut dispose_async = None;
        for (name, method_kind) in methods {
            let method = self.async_disposable_stack_method_value(method_kind)?;
            if matches!(method_kind, AsyncDisposableStackFunctionKind::DisposeAsync) {
                dispose_async = Some(method.clone());
            }
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        self.install_async_disposable_stack_disposed_getter(prototype)?;
        let Some(dispose_async) = dispose_async else {
            return Err(Error::runtime(
                "AsyncDisposableStack disposeAsync method was not installed",
            ));
        };
        self.install_async_disposable_stack_symbols(prototype, dispose_async)
    }

    fn async_disposable_stack_method_value(
        &mut self,
        method: AsyncDisposableStackFunctionKind,
    ) -> Result<Value> {
        let kind = NativeFunctionKind::AsyncDisposableStack(method);
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.create_native_function(kind, Value::Undefined)
    }

    fn install_async_disposable_stack_disposed_getter(
        &mut self,
        prototype: ObjectId,
    ) -> Result<()> {
        let getter = self.async_disposable_stack_method_value(
            AsyncDisposableStackFunctionKind::DisposedGetter,
        )?;
        let key = self.intern_property_key("disposed")?;
        self.objects.define_property(
            prototype,
            key,
            "disposed",
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_async_disposable_stack_symbols(
        &mut self,
        prototype: ObjectId,
        dispose_async: Value,
    ) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let async_dispose_symbol =
            self.get_named(&symbol_constructor, ASYNC_DISPOSE_SYMBOL_PROPERTY)?;
        let Value::Symbol(async_dispose_symbol) = async_dispose_symbol else {
            return Err(Error::runtime("Symbol.asyncDispose is not initialized"));
        };
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(async_dispose_symbol.id()),
            ASYNC_DISPOSE_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(dispose_async),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        let tag_symbol = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(tag_symbol) = tag_symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let tag = self.heap_string_value(ASYNC_DISPOSABLE_STACK_NAME)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(tag_symbol.id()),
            TO_STRING_TAG_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(tag),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime) fn eval_async_disposable_stack_function(
        &mut self,
        method: AsyncDisposableStackFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        match method {
            AsyncDisposableStackFunctionKind::Constructor => Err(Error::type_error(
                "AsyncDisposableStack constructor requires 'new'",
            )),
            AsyncDisposableStackFunctionKind::Adopt => {
                self.async_disposable_stack_adopt(args, this_value)
            }
            AsyncDisposableStackFunctionKind::Defer => {
                self.async_disposable_stack_defer(args, this_value)
            }
            AsyncDisposableStackFunctionKind::DisposeAsync => {
                self.async_disposable_stack_dispose_async(this_value)
            }
            AsyncDisposableStackFunctionKind::DisposedGetter => {
                self.async_disposable_stack_disposed(this_value)
            }
            AsyncDisposableStackFunctionKind::Move => self.async_disposable_stack_move(this_value),
            AsyncDisposableStackFunctionKind::Use => {
                self.async_disposable_stack_use(args, this_value)
            }
        }
    }

    fn async_disposable_stack_data(&self, id: CollectionId) -> Result<&AsyncDisposableStackData> {
        self.collection(id)?
            .async_disposable_stack
            .as_ref()
            .ok_or_else(|| Error::runtime("AsyncDisposableStack storage disappeared"))
    }

    fn async_disposable_stack_data_mut(
        &mut self,
        id: CollectionId,
    ) -> Result<&mut AsyncDisposableStackData> {
        self.collection_mut(id)?
            .async_disposable_stack
            .as_mut()
            .ok_or_else(|| Error::runtime("AsyncDisposableStack storage disappeared"))
    }

    fn async_disposable_stack_id(&self, this_value: &Value) -> Result<CollectionId> {
        self.collection_from_this(this_value, CollectionKind::AsyncDisposableStack)
            .map_err(|_error| Error::type_error(RECEIVER_ERROR))
    }

    fn ensure_async_disposable_stack_pending(&self, id: CollectionId) -> Result<()> {
        if self.async_disposable_stack_data(id)?.disposed {
            return Err(Error::exception(
                crate::value::ErrorName::ReferenceError,
                DISPOSED_ERROR,
            ));
        }
        Ok(())
    }

    fn push_async_disposable_resource(
        &mut self,
        id: CollectionId,
        resource: AsyncDisposableResource,
    ) -> Result<()> {
        self.storage_ledger
            .grow_count(VmStorageKind::CollectionEntry, 1)?;
        self.async_disposable_stack_data_mut(id)?
            .resources
            .push(resource);
        Ok(())
    }

    fn async_disposable_stack_adopt(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let id = self.async_disposable_stack_id(this_value)?;
        self.ensure_async_disposable_stack_pending(id)?;
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let method = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_callable(&method)? {
            return Err(Error::type_error(
                "AsyncDisposableStack adopt callback is not callable",
            ));
        }
        self.push_async_disposable_resource(
            id,
            AsyncDisposableResource::Adopt {
                value: value.clone(),
                method,
            },
        )?;
        Ok(value)
    }

    fn async_disposable_stack_defer(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let id = self.async_disposable_stack_id(this_value)?;
        self.ensure_async_disposable_stack_pending(id)?;
        let method = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_callable(&method)? {
            return Err(Error::type_error(
                "AsyncDisposableStack defer callback is not callable",
            ));
        }
        self.push_async_disposable_resource(id, AsyncDisposableResource::Defer { method })?;
        Ok(Value::Undefined)
    }

    fn async_disposable_stack_use(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let id = self.async_disposable_stack_id(this_value)?;
        self.ensure_async_disposable_stack_pending(id)?;
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if matches!(value, Value::Null | Value::Undefined) {
            self.push_async_disposable_resource(id, AsyncDisposableResource::Nullish)?;
            return Ok(value);
        }
        if self.semantic_object_ref(&value)?.is_none() {
            return Err(Error::type_error(
                "AsyncDisposableStack resource must be an object",
            ));
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        let async_symbol = self.get_named(&symbol_constructor, ASYNC_DISPOSE_SYMBOL_PROPERTY)?;
        let Value::Symbol(async_symbol) = async_symbol else {
            return Err(Error::runtime("Symbol.asyncDispose is not initialized"));
        };
        let async_key = DynamicPropertyKey::new(
            ASYNC_DISPOSE_SYMBOL_DISPLAY.to_owned(),
            Some(PropertyKey::symbol(async_symbol.id())),
        );
        let (method, await_result) = if let Some(method) =
            self.get_method(&value, async_key.lookup())?
        {
            (method, true)
        } else {
            let dispose_symbol = self.get_named(&symbol_constructor, DISPOSE_SYMBOL_PROPERTY)?;
            let Value::Symbol(dispose_symbol) = dispose_symbol else {
                return Err(Error::runtime("Symbol.dispose is not initialized"));
            };
            let dispose_key = DynamicPropertyKey::new(
                DISPOSE_SYMBOL_DISPLAY.to_owned(),
                Some(PropertyKey::symbol(dispose_symbol.id())),
            );
            let Some(method) = self.get_method(&value, dispose_key.lookup())? else {
                return Err(Error::type_error(
                    "AsyncDisposableStack resource has no dispose method",
                ));
            };
            (method, false)
        };
        self.push_async_disposable_resource(
            id,
            AsyncDisposableResource::Use {
                value: value.clone(),
                method,
                await_result,
            },
        )?;
        Ok(value)
    }

    fn async_disposable_stack_disposed(&self, this_value: &Value) -> Result<Value> {
        let id = self.async_disposable_stack_id(this_value)?;
        Ok(Value::Bool(self.async_disposable_stack_data(id)?.disposed))
    }

    fn async_disposable_stack_move(&mut self, this_value: &Value) -> Result<Value> {
        let source = self.async_disposable_stack_id(this_value)?;
        self.ensure_async_disposable_stack_pending(source)?;
        let moved = self.construct_async_disposable_stack()?;
        let target = self.async_disposable_stack_id(&moved)?;
        let resources = {
            let source_data = self.async_disposable_stack_data_mut(source)?;
            source_data.disposed = true;
            core::mem::take(&mut source_data.resources)
        };
        self.async_disposable_stack_data_mut(target)?.resources = resources;
        Ok(moved)
    }

    fn async_disposable_stack_dispose_async(&mut self, this_value: &Value) -> Result<Value> {
        let (result_promise, promise_object) = self.create_pending_promise()?;
        let _root_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            core::iter::once(&promise_object),
        )?;
        let id = match self.async_disposable_stack_id(this_value) {
            Ok(id) => id,
            Err(error) => {
                self.reject_async_disposal_error(result_promise, &error)?;
                return Ok(promise_object);
            }
        };
        let resources = {
            let data = self.async_disposable_stack_data_mut(id)?;
            if data.disposed {
                self.resolve_promise(result_promise, Value::Undefined)?;
                return Ok(promise_object);
            }
            data.disposed = true;
            core::mem::take(&mut data.resources)
        };
        self.storage_ledger
            .release_count(VmStorageKind::CollectionEntry, resources.len())?;
        let continuation = AsyncDisposableStackContinuation::new(result_promise, resources);
        self.continue_async_disposable_stack_disposal(continuation, None)?;
        Ok(promise_object)
    }

    pub(in crate::runtime) fn resume_async_disposable_stack_disposal(
        &mut self,
        continuation: AsyncDisposableStackContinuation,
        resume: Completion,
    ) -> Result<()> {
        self.continue_async_disposable_stack_disposal(continuation, Some(resume))
    }

    fn continue_async_disposable_stack_disposal(
        &mut self,
        mut continuation: AsyncDisposableStackContinuation,
        resume: Option<Completion>,
    ) -> Result<()> {
        let result_promise = continuation.result_promise;
        if let Some(resume) = resume {
            match resume {
                Completion::Normal(_) => {}
                Completion::Throw(reason) => {
                    self.record_async_disposal_error(&mut continuation, reason)?;
                }
                other => {
                    return Err(Error::runtime(format!(
                        "invalid AsyncDisposableStack resume completion {other:?}"
                    )));
                }
            }
        }
        let drive = self.drive_async_disposable_stack_disposal(&mut continuation);
        match drive {
            Ok(AsyncDisposalDrive::Await(awaited)) => {
                let reaction = PromiseReaction::awaiting_async_disposable_stack(continuation);
                if let Err(error) = self.add_promise_reaction(awaited, reaction) {
                    self.reject_async_disposal_error(result_promise, &error)?;
                }
            }
            Ok(AsyncDisposalDrive::Complete) => {
                if let Some(reason) = continuation.thrown {
                    self.reject_promise(result_promise, reason)?;
                } else {
                    self.resolve_promise(result_promise, Value::Undefined)?;
                }
            }
            Err(error) => self.reject_async_disposal_error(result_promise, &error)?,
        }
        Ok(())
    }

    fn drive_async_disposable_stack_disposal(
        &mut self,
        continuation: &mut AsyncDisposableStackContinuation,
    ) -> Result<AsyncDisposalDrive> {
        let root_values: Vec<&Value> = continuation
            .resources
            .iter()
            .flat_map(AsyncDisposableResource::root_values)
            .chain(continuation.thrown.iter())
            .collect();
        let _root_scope = self.transient_root_scope(VmRootKind::TransientTemporary, root_values)?;
        while let Some(resource) = continuation.resources.pop() {
            let result = match self.call_async_disposable_resource(resource) {
                Ok(result) => result,
                Err(error) => {
                    let Some(reason) = runtime_exception_value(self, &error)? else {
                        return Err(error);
                    };
                    self.record_async_disposal_error(continuation, reason)?;
                    continue;
                }
            };
            let awaited = self.promise_resolve_for_await(result)?;
            return Ok(AsyncDisposalDrive::Await(awaited));
        }
        Ok(AsyncDisposalDrive::Complete)
    }

    fn call_async_disposable_resource(
        &mut self,
        resource: AsyncDisposableResource,
    ) -> Result<Value> {
        let completion = match resource {
            AsyncDisposableResource::Use {
                value,
                method,
                await_result,
            } => {
                let completion = self.semantic_call(&method, &[], value)?;
                if await_result {
                    completion
                } else {
                    match completion {
                        Completion::Normal(_) => Completion::Normal(Value::Undefined),
                        other => other,
                    }
                }
            }
            AsyncDisposableResource::Adopt { value, method } => {
                self.semantic_call(&method, &[value], Value::Undefined)?
            }
            AsyncDisposableResource::Defer { method } => {
                self.semantic_call(&method, &[], Value::Undefined)?
            }
            AsyncDisposableResource::Nullish => Completion::Normal(Value::Undefined),
        };
        completion.into_native_value_result()
    }

    fn record_async_disposal_error(
        &mut self,
        continuation: &mut AsyncDisposableStackContinuation,
        error: Value,
    ) -> Result<()> {
        continuation.thrown = Some(if let Some(suppressed) = continuation.thrown.take() {
            self.create_suppressed_error(error, suppressed)?
        } else {
            error
        });
        Ok(())
    }

    fn reject_async_disposal_error(&mut self, promise: PromiseId, error: &Error) -> Result<()> {
        let Some(reason) = runtime_exception_value(self, error)? else {
            return Err(error.clone());
        };
        self.reject_promise(promise, reason)
    }
}
