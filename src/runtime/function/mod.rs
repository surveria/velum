use std::rc::Rc;

use crate::{
    bytecode::{BytecodeFunction, BytecodeNewTargetMode},
    error::{Error, Result},
    runtime::call::RuntimeCallArgs,
    runtime::control::Completion,
    runtime::object::{
        DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit, OwnPropertyDescriptor,
        PropertyConfigurable, PropertyEnumerable, PropertyKey, PropertyLookup, PropertyUpdate,
        PropertyWritable,
    },
    runtime::{CompiledBindingFrame, Context},
    syntax::{StaticFunctionId, StaticName},
    value::{FunctionId, NativeFunctionId, ObjectId, Value},
};

mod arguments;
mod callback_fast_path;
mod class_support;
mod execution;
mod fast_path;
mod intrinsic;
mod names;
mod parameters;
mod properties;
mod property_dispatch;
mod storage;
mod suspended;
mod upvalues;

use crate::runtime::native::{
    NativeFunctionKind, OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME,
    OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_NAME, OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME,
    OBJECT_PROTOTYPE_TO_LOCALE_STRING_NAME, OBJECT_PROTOTYPE_VALUE_OF_NAME,
};
pub(in crate::runtime) use class_support::ResolvedClassField;

/// Per-call snapshot of the callee's shared metadata, extracted in one
/// borrow of the function table before the call frame is assembled.
struct FunctionCallSetup {
    param_atoms: Rc<[crate::storage::atom::AtomId]>,
    param_binding_ids: Rc<[crate::syntax::StaticBindingId]>,
    param_frames: Rc<[Option<crate::runtime::binding::static_bindings::CompiledBindingFrame>]>,
    bytecode: crate::bytecode::BytecodeFunction,
    upvalues: super::FunctionUpvalues,
    static_name_atom_cache:
        Option<crate::runtime::property::static_names::StaticNameAtomCacheHandle>,
    static_binding_cache:
        Option<crate::runtime::binding::static_bindings::StaticBindingCacheHandle>,
    static_binding_layout: Option<crate::binding_metadata::BindingLayout>,
    arguments_binding: Option<FunctionArgumentsBinding>,
    self_binding: Option<FunctionSelfBinding>,
    super_binding: Option<Rc<FunctionSuperBinding>>,
    private_environment: Option<Rc<super::private::PrivateEnvironment>>,
    remember_params: bool,
    scope_template: Option<Rc<FunctionScopeTemplate>>,
}
pub(super) use fast_path::FunctionFastPath;
use parameters::FunctionParameterState;
pub(in crate::runtime) use parameters::{
    FunctionArgumentsBinding, FunctionScopeTemplate, FunctionSelfBinding,
};
pub(super) use properties::{FunctionIntrinsicDefaults, FunctionProperties};
pub(in crate::runtime) use suspended::{DetachedFunctionExecution, SuspendedAsyncFunction};

const FUNCTION_PROTOTYPE_APPLY_PROPERTY: &str = "apply";
const FUNCTION_PROTOTYPE_BIND_PROPERTY: &str = "bind";
const FUNCTION_PROTOTYPE_CALL_PROPERTY: &str = "call";
const FUNCTION_PROTOTYPE_TO_STRING_PROPERTY: &str = "toString";
const FUNCTION_PROTOTYPE_ARGUMENTS_PROPERTY: &str = "arguments";
const FUNCTION_PROTOTYPE_CALLER_PROPERTY: &str = "caller";

use super::FunctionNewTarget;
use properties::{FunctionPropertyKind, PROTOTYPE_CONSTRUCTOR_PROPERTY};

fn expected_function_local_count(base: usize, has_self_binding: bool) -> Result<usize> {
    let with_function_scope = base
        .checked_add(1)
        .ok_or_else(|| Error::limit("function local scope count overflowed"))?;
    with_function_scope
        .checked_add(usize::from(has_self_binding))
        .ok_or_else(|| Error::limit("function local scope count overflowed"))
}

/// Super references available to a class constructor or method body: the
/// callable parent constructor (derived constructors only) and the home
/// prototype used by `super.property` lookups.
#[derive(Debug)]
pub(in crate::runtime) struct FunctionSuperBinding {
    pub(in crate::runtime) constructor: Option<Value>,
    pub(in crate::runtime) home_prototype: Value,
    /// The derived constructor owning this binding; its instance fields
    /// initialize after `super()` completes.
    pub(in crate::runtime) own_constructor: Option<FunctionId>,
}

pub(super) struct BytecodeFunctionInit<'a> {
    pub(super) static_function_id: StaticFunctionId,
    pub(super) name: Option<&'a StaticName>,
    pub(super) bytecode: &'a BytecodeFunction,
    pub(super) constructable: bool,
    pub(super) kind: crate::syntax::FunctionKind,
    pub(super) class_constructor: bool,
    pub(super) prototype_parent: Option<crate::value::ObjectId>,
    pub(super) new_target_mode: BytecodeNewTargetMode,
}

impl Context {
    fn create_bytecode_function_prototype(
        &mut self,
        id: FunctionId,
        init: &BytecodeFunctionInit<'_>,
    ) -> Result<Value> {
        if init.constructable {
            let constructor_key = self.intern_property_key(PROTOTYPE_CONSTRUCTOR_PROPERTY)?;
            let prototype_id = self.objects.create_with_prototype_property(
                init.prototype_parent,
                ObjectPropertyInit::new(
                    constructor_key,
                    PROTOTYPE_CONSTRUCTOR_PROPERTY,
                    Value::Function(id),
                    PropertyEnumerable::No,
                ),
                constructor_key,
                self.limits.max_objects,
                self.limits.max_object_properties,
            )?;
            return Ok(Value::Object(prototype_id));
        }
        if init.kind.is_async_generator() {
            let constructor_key = self.object_constructor_property_key()?;
            let generator_prototype = self.async_generator_prototype_id()?;
            return self.objects.create_with_prototype(
                Some(generator_prototype),
                constructor_key,
                self.limits.max_objects,
                self.limits.max_object_properties,
            );
        }
        if init.kind.is_generator() {
            let constructor_key = self.object_constructor_property_key()?;
            let generator_prototype = self.generator_prototype_id()?;
            return self.objects.create_with_prototype(
                Some(generator_prototype),
                constructor_key,
                self.limits.max_objects,
                self.limits.max_object_properties,
            );
        }
        Ok(Value::Undefined)
    }

    pub(super) fn create_bytecode_function(
        &mut self,
        init: &BytecodeFunctionInit<'_>,
    ) -> Result<Value> {
        self.functions.reserve_insert()?;
        let id = FunctionId::new(self.functions.next_index());
        let function = Value::Function(id);
        let prototype = self.create_bytecode_function_prototype(id, init)?;
        let function_name = self.function_name_value(init.name)?;
        let params = init.bytecode.params();
        let arity = parameters::function_arity(params);
        let prototype_default = (init.constructable || init.kind.is_generator()).then(|| {
            DataPropertyDescriptor::new(
                prototype.clone(),
                PropertyWritable::Yes,
                PropertyEnumerable::No,
                PropertyConfigurable::No,
            )
        });
        let intrinsic_defaults =
            FunctionIntrinsicDefaults::new(arity.value()?, function_name, prototype_default);
        let param_atoms = self.function_param_atoms(params)?;
        let static_name_atom_cache = self.current_static_name_atom_cache();
        let static_binding_cache = self.current_static_binding_cache();
        let static_binding_layout = self.current_static_binding_layout();
        let param_frames =
            parameters::function_param_frames(params, static_binding_layout.as_ref())?;
        let self_binding =
            self.compile_function_self_binding(init.bytecode, static_binding_layout.as_ref())?;
        let arguments_binding =
            self.compile_function_arguments_binding(init.bytecode, static_binding_layout.as_ref())?;
        let fast_path = self.compile_optional_function_fast_path(init, &param_frames)?;
        let upvalues = self.capture_function_upvalues(
            init.static_function_id,
            init.bytecode.capture_bindings(),
            static_binding_layout.as_ref(),
        )?;
        let scope_template = parameters::function_scope_template(
            &param_atoms,
            &param_frames,
            init.bytecode.has_parameter_defaults(),
            arguments_binding,
        )?;
        let param_binding_ids = parameters::function_param_binding_ids(params);
        let metadata_cache_count = Self::function_metadata_cache_count(
            param_binding_ids.len(),
            param_atoms.len(),
            param_frames.len(),
            fast_path.is_some(),
            scope_template.as_deref(),
            self_binding.is_some(),
            arguments_binding.is_some(),
        )?;
        let properties = self.activate_function_storage(
            upvalues.cells.len(),
            metadata_cache_count,
            FunctionProperties::new(prototype, intrinsic_defaults),
        )?;
        let super_binding = self.bytecode_function_super_binding(init.new_target_mode);
        self.functions.insert_at_next(
            id.index(),
            super::Function {
                self_binding,
                arguments_binding,
                param_binding_ids,
                param_atoms,
                param_frames,
                bytecode: init.bytecode.clone(),
                fast_path: fast_path.map(Rc::new),
                source: None,
                upvalues: upvalues.cells,
                static_name_atom_cache,
                static_binding_cache,
                static_binding_layout,
                properties,
                constructable: init.constructable,
                kind: init.kind,
                class_constructor: init.class_constructor,
                super_binding,
                static_parent: None,
                class_fields: None,
                class_private_slots: None,
                private_environment: self.current_private_environment(),
                private_slots: Vec::new(),
                params_remembered: std::cell::Cell::new(false),
                scope_template,
                new_target: FunctionNewTarget::from_mode(
                    init.new_target_mode,
                    self.current_new_target()?,
                ),
            },
        )?;
        Ok(function)
    }

    fn compile_optional_function_fast_path(
        &self,
        init: &BytecodeFunctionInit<'_>,
        param_frames: &[Option<CompiledBindingFrame>],
    ) -> Result<Option<FunctionFastPath>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        if init.bytecode.self_binding().is_some() {
            return Ok(None);
        }
        FunctionFastPath::compile(
            init.bytecode,
            param_frames,
            init.new_target_mode,
            init.kind.is_async() || init.kind.is_generator(),
            init.class_constructor,
        )
    }

    fn bytecode_function_super_binding(
        &self,
        mode: BytecodeNewTargetMode,
    ) -> Option<Rc<FunctionSuperBinding>> {
        if mode == BytecodeNewTargetMode::Lexical {
            return self.current_activation_super();
        }
        None
    }

    pub(crate) fn eval_function_call_completion_with_this(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
    ) -> Result<Completion> {
        self.reject_class_constructor_call(id)?;
        let new_target = self.function_direct_call_new_target(id)?;
        if self.function(id)?.kind.is_async_generator() {
            let completion = self.eval_generator_function_completion_with_this_and_new_target(
                id, args, this_value, new_target,
            )?;
            return match completion {
                Completion::GeneratorStart => {
                    let execution = self.detach_function_execution(id)?;
                    self.create_generator_object(id, execution)
                        .map(Completion::Normal)
                }
                completion @ Completion::Throw(_) => Ok(completion),
                completion => Err(Error::runtime(format!(
                    "async generator initialization produced invalid completion {completion:?}"
                ))),
            };
        }
        if self.function(id)?.kind.is_async() {
            let value = self.eval_async_function_with_this(id, args, this_value, new_target)?;
            return Ok(Completion::Normal(value));
        }
        if self.function(id)?.kind.is_generator() {
            let completion = self.eval_generator_function_completion_with_this_and_new_target(
                id, args, this_value, new_target,
            )?;
            return match completion {
                Completion::GeneratorStart => {
                    let execution = self.detach_function_execution(id)?;
                    self.create_generator_object(id, execution)
                        .map(Completion::Normal)
                }
                completion @ Completion::Throw(_) => Ok(completion),
                completion => Err(Error::runtime(format!(
                    "generator initialization produced invalid completion {completion:?}"
                ))),
            };
        }
        self.eval_function_completion_with_this_and_new_target(id, args, this_value, new_target)?
            .into_call_completion()
    }

    pub(crate) fn eval_function_completion_with_this(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
    ) -> Result<Completion> {
        let new_target = self.function_direct_call_new_target(id)?;
        self.eval_function_completion_with_this_and_new_target(id, args, this_value, new_target)
    }

    fn function_direct_call_new_target(&self, id: FunctionId) -> Result<Value> {
        match &self.function(id)?.new_target {
            FunctionNewTarget::Own => Ok(Value::Undefined),
            FunctionNewTarget::Lexical(value) => Ok(value.clone()),
        }
    }

    fn function_call_setup(&self, id: FunctionId) -> Result<FunctionCallSetup> {
        let function = self.function(id)?;
        Ok(FunctionCallSetup {
            param_atoms: Rc::clone(&function.param_atoms),
            param_binding_ids: Rc::clone(&function.param_binding_ids),
            param_frames: Rc::clone(&function.param_frames),
            bytecode: function.bytecode.clone(),
            upvalues: Rc::clone(&function.upvalues),
            static_name_atom_cache: function.static_name_atom_cache.clone(),
            static_binding_cache: function.static_binding_cache.clone(),
            static_binding_layout: function.static_binding_layout.clone(),
            arguments_binding: function.arguments_binding,
            self_binding: function.self_binding,
            super_binding: function.super_binding.clone(),
            private_environment: function.private_environment.clone(),
            remember_params: !function.params_remembered.replace(true),
            scope_template: function.scope_template.clone(),
        })
    }

    pub(super) fn eval_function_completion_with_this_inner<const CAN_SUSPEND: bool>(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
        new_target: Value,
    ) -> Result<Completion> {
        let raw_args = args.as_slice();
        if let Some(completion) = self.try_eval_pre_setup_function_fast_path(id, raw_args)? {
            return Ok(completion);
        }
        let FunctionCallSetup {
            param_atoms,
            param_binding_ids,
            param_frames,
            bytecode,
            upvalues,
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
            arguments_binding,
            self_binding,
            super_binding,
            private_environment,
            remember_params,
            scope_template,
        } = self.function_call_setup(id)?;
        let packed_args = if bytecode.has_rest_parameter() {
            Some(self.pack_rest_arguments(bytecode.params(), raw_args.to_vec())?)
        } else {
            None
        };
        let args = packed_args.as_deref().unwrap_or(raw_args);
        let has_parameter_defaults = bytecode.has_parameter_defaults();
        let local_base = self.push_call_activation(
            id,
            upvalues,
            this_value,
            new_target,
            super_binding,
            private_environment,
        )?;
        if let Some(self_binding) = self_binding {
            let self_scope = self.named_function_self_scope(id, self_binding)?;
            if let Err(error) = self.push_lexical_scope_with(self_scope) {
                self.leave_function_local_frame(local_base)?;
                self.pop_call_activation(local_base)?;
                return Err(error);
            }
        }
        let scope_result = if let Some(template) = scope_template.as_deref() {
            self.function_scope_from_template(
                template,
                args,
                arguments_binding.map(|binding| (binding, raw_args)),
            )
        } else {
            self.function_scope(
                &param_atoms,
                &param_frames,
                args,
                has_parameter_defaults,
                arguments_binding.map(|binding| (binding, raw_args)),
            )
        };
        let scope = match scope_result {
            Ok(scope) => scope,
            Err(error) => {
                self.leave_function_local_frame(local_base)?;
                self.pop_call_activation(local_base)?;
                return Err(error);
            }
        };
        if let Err(error) = self.push_function_binding_storage(local_base, scope) {
            self.pop_call_activation(local_base)?;
            return Err(error);
        }
        let result = self.eval_function_body::<CAN_SUSPEND>(
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
            FunctionParameterState::new(id, &param_binding_ids, &param_atoms, args),
            &bytecode,
            remember_params,
        );
        if CAN_SUSPEND && result.as_ref().is_ok_and(Completion::suspends_execution) {
            return result;
        }
        let binding_result = self.pop_function_binding_storage(local_base, self_binding.is_some());
        let activation_result = self.pop_call_activation(local_base);
        binding_result?;
        activation_result?;
        result
    }

    fn try_eval_pre_setup_function_fast_path(
        &mut self,
        id: FunctionId,
        raw_args: &[Value],
    ) -> Result<Option<Completion>> {
        let Some((fast_path, fast_upvalues)) = ({
            let function = self.function(id)?;
            function.fast_path.as_ref().map(|fast_path| {
                let upvalues = if fast_path.needs_upvalues() {
                    Some(Rc::clone(&function.upvalues))
                } else {
                    None
                };
                (Rc::clone(fast_path), upvalues)
            })
        }) else {
            return Ok(None);
        };
        let upvalues = fast_upvalues.as_deref().unwrap_or(&[]);
        self.eval_bytecode_function_pre_setup_fast_path(&fast_path, raw_args, upvalues)
    }

    pub(in crate::runtime) fn current_super_frame(&self) -> Option<Rc<FunctionSuperBinding>> {
        self.current_activation_super()
    }

    /// Class constructors are constructor-only callables.
    fn reject_class_constructor_call(&self, id: FunctionId) -> Result<()> {
        if self.function(id)?.class_constructor {
            return Err(Error::type_error(
                "Class constructor cannot be invoked without 'new'",
            ));
        }
        Ok(())
    }

    pub(crate) fn function_constructor_prototype(
        &self,
        id: FunctionId,
    ) -> Result<Option<ObjectId>> {
        let function = self.function(id)?;
        if !function.constructable {
            return Err(Error::type_error("function is not a constructor"));
        }
        match function.properties.prototype() {
            Value::Object(id) => Ok(Some(id)),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => Ok(None),
        }
    }

    pub(in crate::runtime) fn is_function_constructable(&self, id: FunctionId) -> Result<bool> {
        Ok(self.function(id)?.constructable)
    }

    pub(in crate::runtime) fn function(&self, id: FunctionId) -> Result<&super::Function> {
        self.functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("function id is not defined"))
    }

    pub(in crate::runtime) fn function_prototype_value(&self, id: FunctionId) -> Result<Value> {
        Ok(self.function(id)?.properties.prototype())
    }

    pub(in crate::runtime) fn function_source_text(&self, id: FunctionId) -> Result<String> {
        let Some(source) = &self.function(id)?.source else {
            return Ok("function()".to_owned());
        };
        Ok(source.to_string())
    }

    pub(in crate::runtime) fn set_function_source(
        &mut self,
        id: FunctionId,
        source: Rc<str>,
    ) -> Result<()> {
        let previous_bytes = self.function(id)?.source.as_deref().map_or(0, str::len);
        let additional_count = usize::from(self.function(id)?.source.is_none());
        let projected_count = self
            .source_record_count()
            .checked_add(additional_count)
            .ok_or_else(|| Error::limit("source record count overflowed"))?;
        let projected_payload_bytes = self
            .source_record_bytes()?
            .checked_sub(previous_bytes)
            .and_then(|bytes| bytes.checked_add(source.len()))
            .ok_or_else(|| Error::limit("source record payload bytes overflowed"))?;
        self.ensure_storage_totals(
            crate::runtime::VmStorageKind::SourceRecord,
            projected_count,
            projected_payload_bytes,
        )?;
        self.function_mut(id)?.source = Some(source);
        Ok(())
    }

    pub(in crate::runtime) fn set_generated_function_name(
        &mut self,
        id: FunctionId,
        name: &str,
    ) -> Result<()> {
        let value = self.heap_string_value(name)?;
        self.function_mut(id)?.properties.set_generated_name(value);
        Ok(())
    }

    fn function_mut(&mut self, id: FunctionId) -> Result<&mut super::Function> {
        self.functions
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("function id is not defined"))
    }

    pub(in crate::runtime) fn should_materialize_function_prototype_for(
        &self,
        property: PropertyLookup<'_>,
    ) -> bool {
        self.native_function_id(NativeFunctionKind::Function)
            .is_some()
            || matches!(property.key(), Some(PropertyKey::Symbol(_)))
            || property.name() == PROTOTYPE_CONSTRUCTOR_PROPERTY
            || property.name() == FUNCTION_PROTOTYPE_APPLY_PROPERTY
            || property.name() == FUNCTION_PROTOTYPE_BIND_PROPERTY
            || property.name() == FUNCTION_PROTOTYPE_CALL_PROPERTY
            || property.name() == FUNCTION_PROTOTYPE_TO_STRING_PROPERTY
            || property.name() == OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME
            || property.name() == OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME
            || property.name() == OBJECT_PROTOTYPE_VALUE_OF_NAME
            || property.name() == OBJECT_PROTOTYPE_TO_LOCALE_STRING_NAME
            || property.name() == OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_NAME
    }

    pub(in crate::runtime) fn function_should_materialize_prototype_for(
        &self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        if self.should_materialize_function_prototype_for(property) {
            return Ok(true);
        }
        self.function_uses_restricted_prototype(id, property)
    }

    pub(in crate::runtime) fn function_uses_restricted_prototype(
        &self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let kind = self.function(id)?.kind;
        Ok((kind.is_generator() || kind.is_async()) && Self::is_restricted_property(property))
    }

    pub(in crate::runtime) fn is_restricted_property(property: PropertyLookup<'_>) -> bool {
        matches!(
            property.name(),
            FUNCTION_PROTOTYPE_ARGUMENTS_PROPERTY | FUNCTION_PROTOTYPE_CALLER_PROPERTY
        )
    }

    pub(crate) fn native_function_object_prototype_value(
        &mut self,
        id: NativeFunctionId,
    ) -> Result<Value> {
        let kind = self.native_function(id)?.kind();
        if matches!(kind, NativeFunctionKind::TypedArray(_)) {
            return self.typed_array_intrinsic_constructor_value();
        }
        if matches!(
            kind,
            NativeFunctionKind::AsyncFunction | NativeFunctionKind::AsyncGeneratorFunction
        ) {
            return self.function_constructor_value();
        }
        self.function_constructor_prototype_value()
    }

    pub(crate) fn native_function_own_property_descriptor_lookup(
        &self,
        id: NativeFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        let function = self.native_function(id)?;
        let property_name = property.name();
        let property_kind = FunctionPropertyKind::from_name(property_name);
        if let Some(descriptor) = function.properties().intrinsic_descriptor(property_kind) {
            return Ok(Some(OwnPropertyDescriptor::Data(descriptor)));
        }
        if let Some(value) = function.intrinsic_property(property_name) {
            return Ok(Some(OwnPropertyDescriptor::Data(
                DataPropertyDescriptor::new(
                    value,
                    PropertyWritable::No,
                    PropertyEnumerable::No,
                    PropertyConfigurable::No,
                ),
            )));
        }
        Ok(function.properties().own_property_descriptor(property))
    }

    pub(crate) fn has_native_function_property_lookup(
        &self,
        id: NativeFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let function = self.native_function(id)?;
        let property_name = property.name();
        let property_kind = FunctionPropertyKind::from_name(property_name);
        if property_kind.is_intrinsic_slot() && function.properties().has_intrinsic(property_kind) {
            return Ok(true);
        }
        if property_kind.is_prototype() {
            return Ok(function
                .properties()
                .intrinsic_descriptor(property_kind)
                .is_some());
        }
        Ok(function.has_intrinsic_property(property_name) || function.properties().has(property))
    }

    pub(crate) fn has_native_function_property_including_prototype_lookup(
        &mut self,
        id: NativeFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        if self.has_native_function_property_lookup(id, property)? {
            return Ok(true);
        }
        let kind = self.native_function(id)?.kind();
        if !matches!(kind, NativeFunctionKind::TypedArray(_))
            && !self.should_materialize_function_prototype_for(property)
        {
            return Ok(false);
        }
        let parent = self.native_function_object_prototype_value(id)?;
        if matches!(parent, Value::Null | Value::Undefined) {
            return Ok(false);
        }
        let Some(presence) = self.semantic_property_presence(&parent, property)? else {
            return Ok(false);
        };
        self.finish_semantic_property_presence(presence, property)
    }

    pub(crate) fn set_native_function_property_key(
        &mut self,
        id: NativeFunctionId,
        property: &str,
        key: PropertyKey,
        value: Value,
    ) -> Result<()> {
        let property_kind = FunctionPropertyKind::from_name(property);
        if property_kind.is_prototype() {
            self.native_function(id)?;
            return Ok(());
        }
        if self.native_function(id)?.has_intrinsic_property(property) {
            return Ok(());
        }
        let max_properties = self.limits.max_object_properties;
        let function = self.native_function_mut(id)?;
        function
            .properties_mut()
            .set(key, property_kind, value, max_properties)
    }

    pub(crate) fn define_native_function_property_key(
        &mut self,
        id: NativeFunctionId,
        property: &str,
        key: PropertyKey,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        let property_kind = FunctionPropertyKind::from_name(property);
        if self.native_function(id)?.has_intrinsic_property(property) {
            return Ok(());
        }
        let max_properties = self.limits.max_object_properties;
        let function = self.native_function_mut(id)?;
        function.properties_mut().define_property(
            key,
            property_kind,
            PropertyUpdate::Data(update),
            max_properties,
        )
    }

    pub(crate) fn delete_native_function_property_lookup(
        &mut self,
        id: NativeFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let property_name = property.name();
        let property_kind = FunctionPropertyKind::from_name(property_name);
        if self
            .native_function(id)?
            .has_intrinsic_property(property_name)
        {
            return Ok(false);
        }
        let function = self.native_function_mut(id)?;
        function.properties_mut().delete(property, property_kind)
    }

    pub(crate) fn native_function_enumerable_keys(
        &self,
        id: NativeFunctionId,
    ) -> Result<Vec<String>> {
        let function = self.native_function(id)?;
        function.properties().keys(&self.atoms)
    }

    fn function_name_value(&mut self, name: Option<&StaticName>) -> Result<Value> {
        let Some(name) = name.filter(|name| !name.as_str().is_empty()) else {
            return self.heap_string_value("");
        };
        self.heap_string_value(name.as_str())
    }
}
