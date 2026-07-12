use crate::{
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        call::RuntimeCallArgs,
        collections::{CollectionId, CollectionKind},
        control::Completion,
        native::{
            DISPOSABLE_STACK_NAME, DisposableStackFunctionKind, NativeFunctionKind,
            OBJECT_CONSTRUCTOR_PROPERTY,
        },
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::{ObjectId, Value},
};

const DISPOSED_ERROR: &str = "DisposableStack is already disposed";
const RECEIVER_ERROR: &str = "method requires a DisposableStack receiver";
const DISPOSE_SYMBOL_DISPLAY: &str = "[Symbol.dispose]";
const DISPOSE_SYMBOL_PROPERTY: &str = "dispose";
const TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";

#[derive(Debug, Clone)]
enum DisposableResource {
    Use { value: Value, method: Value },
    Adopt { value: Value, method: Value },
    Defer { method: Value },
}

impl DisposableResource {
    fn visit_edges<V: StrongEdgeVisitor<crate::runtime::async_trace::VmAsyncEdgeKind>>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        let mut visit = |value: &Value| {
            visitor.visit(
                crate::runtime::async_trace::VmAsyncEdgeKind::CollectionEntry,
                StrongEdgeReference::Value(value),
            )
        };
        match self {
            Self::Use { value, method } | Self::Adopt { value, method } => {
                visit(value)?;
                visit(method)
            }
            Self::Defer { method } => visit(method),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct DisposableStackData {
    disposed: bool,
    resources: Vec<DisposableResource>,
}

impl DisposableStackData {
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
        V: StrongEdgeVisitor<crate::runtime::async_trace::VmAsyncEdgeKind>,
    {
        for resource in &self.resources {
            resource.visit_edges(visitor)?;
        }
        Ok(())
    }
}

impl Context {
    pub(in crate::runtime) fn disposable_stack_constructor_value(&mut self) -> Result<Value> {
        let kind = NativeFunctionKind::DisposableStack(DisposableStackFunctionKind::Constructor);
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
        self.install_disposable_stack_prototype(prototype)?;
        self.insert_global_builtin(DISPOSABLE_STACK_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime) fn construct_disposable_stack(&mut self) -> Result<Value> {
        let prototype = self.disposable_stack_intrinsic_prototype()?;
        self.create_disposable_stack_with_prototype(prototype)
    }

    pub(in crate::runtime) fn construct_disposable_stack_with_new_target(
        &mut self,
        new_target: &Value,
    ) -> Result<Value> {
        let prototype = self.constructor_instance_prototype_with_default(
            new_target,
            NativeFunctionKind::DisposableStack(DisposableStackFunctionKind::Constructor),
        )?;
        self.create_disposable_stack_with_prototype(prototype)
    }

    fn disposable_stack_intrinsic_prototype(&mut self) -> Result<ObjectId> {
        let constructor = self.disposable_stack_constructor_value()?;
        let Value::NativeFunction(id) = constructor else {
            return Err(Error::runtime("DisposableStack constructor disappeared"));
        };
        let Value::Object(prototype) = self.native_function(id)?.properties().prototype() else {
            return Err(Error::runtime("DisposableStack prototype is not an object"));
        };
        Ok(prototype)
    }

    fn create_disposable_stack_with_prototype(&mut self, prototype: ObjectId) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        let value = self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object) = value else {
            return Err(Error::runtime("DisposableStack allocation failed"));
        };
        let collection = self.create_collection(CollectionKind::DisposableStack)?;
        self.bind_collection_object(object, CollectionKind::DisposableStack, collection)?;
        Ok(Value::Object(object))
    }

    fn install_disposable_stack_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        let methods = [
            ("adopt", DisposableStackFunctionKind::Adopt),
            ("defer", DisposableStackFunctionKind::Defer),
            ("dispose", DisposableStackFunctionKind::Dispose),
            ("move", DisposableStackFunctionKind::Move),
            ("use", DisposableStackFunctionKind::Use),
        ];
        let mut dispose = None;
        for (name, method_kind) in methods {
            let method = self.disposable_stack_method_value(method_kind)?;
            if matches!(method_kind, DisposableStackFunctionKind::Dispose) {
                dispose = Some(method.clone());
            }
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        self.install_disposable_stack_disposed_getter(prototype)?;
        let Some(dispose) = dispose else {
            return Err(Error::runtime(
                "DisposableStack dispose method was not installed",
            ));
        };
        self.install_disposable_stack_symbols(prototype, dispose)
    }

    fn disposable_stack_method_value(
        &mut self,
        method: DisposableStackFunctionKind,
    ) -> Result<Value> {
        let kind = NativeFunctionKind::DisposableStack(method);
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.create_native_function(kind, Value::Undefined)
    }

    fn install_disposable_stack_disposed_getter(&mut self, prototype: ObjectId) -> Result<()> {
        let getter =
            self.disposable_stack_method_value(DisposableStackFunctionKind::DisposedGetter)?;
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

    fn install_disposable_stack_symbols(
        &mut self,
        prototype: ObjectId,
        dispose: Value,
    ) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let dispose_symbol = self.get_named(&symbol_constructor, DISPOSE_SYMBOL_PROPERTY)?;
        let Value::Symbol(dispose_symbol) = dispose_symbol else {
            return Err(Error::runtime("Symbol.dispose is not initialized"));
        };
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(dispose_symbol.id()),
            DISPOSE_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(dispose),
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
        let tag = self.heap_string_value(DISPOSABLE_STACK_NAME)?;
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

    pub(in crate::runtime) fn eval_disposable_stack_function(
        &mut self,
        method: DisposableStackFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        match method {
            DisposableStackFunctionKind::Constructor => Err(Error::type_error(
                "DisposableStack constructor requires 'new'",
            )),
            DisposableStackFunctionKind::Adopt => self.disposable_stack_adopt(args, this_value),
            DisposableStackFunctionKind::Defer => self.disposable_stack_defer(args, this_value),
            DisposableStackFunctionKind::Dispose => self.disposable_stack_dispose(this_value),
            DisposableStackFunctionKind::DisposedGetter => {
                self.disposable_stack_disposed(this_value)
            }
            DisposableStackFunctionKind::Move => self.disposable_stack_move(this_value),
            DisposableStackFunctionKind::Use => self.disposable_stack_use(args, this_value),
        }
    }

    fn disposable_stack_data(&self, id: CollectionId) -> Result<&DisposableStackData> {
        self.collection(id)?
            .disposable_stack
            .as_ref()
            .ok_or_else(|| Error::runtime("DisposableStack storage disappeared"))
    }

    fn disposable_stack_data_mut(&mut self, id: CollectionId) -> Result<&mut DisposableStackData> {
        self.collection_mut(id)?
            .disposable_stack
            .as_mut()
            .ok_or_else(|| Error::runtime("DisposableStack storage disappeared"))
    }

    fn disposable_stack_id(&self, this_value: &Value) -> Result<CollectionId> {
        self.collection_from_this(this_value, CollectionKind::DisposableStack)
            .map_err(|_error| Error::type_error(RECEIVER_ERROR))
    }

    fn ensure_disposable_stack_pending(&self, id: CollectionId) -> Result<()> {
        if self.disposable_stack_data(id)?.disposed {
            return Err(Error::exception(
                crate::value::ErrorName::ReferenceError,
                DISPOSED_ERROR,
            ));
        }
        Ok(())
    }

    fn push_disposable_resource(
        &mut self,
        id: CollectionId,
        resource: DisposableResource,
    ) -> Result<()> {
        self.storage_ledger
            .grow_count(VmStorageKind::CollectionEntry, 1)?;
        self.disposable_stack_data_mut(id)?.resources.push(resource);
        Ok(())
    }

    fn disposable_stack_adopt(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let id = self.disposable_stack_id(this_value)?;
        self.ensure_disposable_stack_pending(id)?;
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let method = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_callable(&method)? {
            return Err(Error::type_error(
                "DisposableStack adopt callback is not callable",
            ));
        }
        self.push_disposable_resource(
            id,
            DisposableResource::Adopt {
                value: value.clone(),
                method,
            },
        )?;
        Ok(value)
    }

    fn disposable_stack_defer(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let id = self.disposable_stack_id(this_value)?;
        self.ensure_disposable_stack_pending(id)?;
        let method = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_callable(&method)? {
            return Err(Error::type_error(
                "DisposableStack defer callback is not callable",
            ));
        }
        self.push_disposable_resource(id, DisposableResource::Defer { method })?;
        Ok(Value::Undefined)
    }

    fn disposable_stack_use(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let id = self.disposable_stack_id(this_value)?;
        self.ensure_disposable_stack_pending(id)?;
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if matches!(value, Value::Null | Value::Undefined) {
            return Ok(value);
        }
        if self.semantic_object_ref(&value)?.is_none() {
            return Err(Error::type_error(
                "DisposableStack resource must be an object",
            ));
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        let symbol = self.get_named(&symbol_constructor, DISPOSE_SYMBOL_PROPERTY)?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("Symbol.dispose is not initialized"));
        };
        let key = DynamicPropertyKey::new(
            DISPOSE_SYMBOL_DISPLAY.to_owned(),
            Some(PropertyKey::symbol(symbol.id())),
        );
        let Some(method) = self.get_method(&value, key.lookup())? else {
            return Err(Error::type_error(
                "DisposableStack resource has no dispose method",
            ));
        };
        self.push_disposable_resource(
            id,
            DisposableResource::Use {
                value: value.clone(),
                method,
            },
        )?;
        Ok(value)
    }

    fn disposable_stack_disposed(&self, this_value: &Value) -> Result<Value> {
        let id = self.disposable_stack_id(this_value)?;
        Ok(Value::Bool(self.disposable_stack_data(id)?.disposed))
    }

    fn disposable_stack_move(&mut self, this_value: &Value) -> Result<Value> {
        let source = self.disposable_stack_id(this_value)?;
        self.ensure_disposable_stack_pending(source)?;
        let moved = self.construct_disposable_stack()?;
        let target = self.disposable_stack_id(&moved)?;
        let resources = {
            let source_data = self.disposable_stack_data_mut(source)?;
            source_data.disposed = true;
            std::mem::take(&mut source_data.resources)
        };
        self.disposable_stack_data_mut(target)?.resources = resources;
        Ok(moved)
    }

    fn disposable_stack_dispose(&mut self, this_value: &Value) -> Result<Value> {
        self.dispose_disposable_stack_completion(this_value, Completion::Normal(Value::Undefined))?
            .into_native_value_result()
    }

    pub(in crate::runtime) fn dispose_disposable_stack_completion(
        &mut self,
        this_value: &Value,
        mut completion: Completion,
    ) -> Result<Completion> {
        let id = self.disposable_stack_id(this_value)?;
        let resources = {
            let data = self.disposable_stack_data_mut(id)?;
            if data.disposed {
                return Ok(completion);
            }
            data.disposed = true;
            std::mem::take(&mut data.resources)
        };
        self.storage_ledger
            .release_count(VmStorageKind::CollectionEntry, resources.len())?;
        for resource in resources.into_iter().rev() {
            if let Some(error) = self.dispose_resource(resource)? {
                completion = match completion {
                    Completion::Throw(suppressed) => {
                        Completion::Throw(self.create_suppressed_error(error, suppressed)?)
                    }
                    _ => Completion::Throw(error),
                };
            }
        }
        Ok(completion)
    }

    fn dispose_resource(&mut self, resource: DisposableResource) -> Result<Option<Value>> {
        let completion = match resource {
            DisposableResource::Use { value, method } => self.semantic_call(&method, &[], value),
            DisposableResource::Adopt { value, method } => {
                self.semantic_call(&method, &[value], Value::Undefined)
            }
            DisposableResource::Defer { method } => {
                self.semantic_call(&method, &[], Value::Undefined)
            }
        };
        match completion {
            Ok(Completion::Throw(value)) => Ok(Some(value)),
            Ok(Completion::Normal(_)) => Ok(None),
            Ok(other) => other.into_native_value_result().map(|_value| None),
            Err(error) => {
                if let Some(value) = error.javascript_value() {
                    return Ok(Some(value.clone()));
                }
                Err(error)
            }
        }
    }
}
