use std::rc::Rc;

use crate::{
    bytecode::{BytecodeFunction, BytecodeNewTargetMode},
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::control::Completion,
    runtime::object::{
        DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyKey, PropertyLookup, PropertyWritable,
    },
    runtime::property::get_property,
    syntax::{StaticFunctionId, StaticName},
    value::{FunctionId, NativeFunctionId, ObjectId, Value},
};

mod arguments;
mod callback_fast_path;
mod class_support;
mod execution;
mod fast_path;
mod intrinsic;
mod parameters;
mod properties;
mod storage;
mod suspended;
mod upvalues;

use crate::runtime::native::{
    NativeFunctionKind, OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME,
    OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME,
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
    binds_arguments: bool,
    super_binding: Option<Rc<FunctionSuperBinding>>,
    remember_params: bool,
    scope_template: Option<Rc<FunctionScopeTemplate>>,
}
pub(super) use fast_path::FunctionFastPath;
use parameters::FunctionParameterState;
pub(in crate::runtime) use parameters::FunctionScopeTemplate;
pub(super) use properties::{FunctionIntrinsicDefaults, FunctionProperties};
pub(in crate::runtime) use suspended::SuspendedAsyncFunction;

const FUNCTION_PROTOTYPE_APPLY_PROPERTY: &str = "apply";
const FUNCTION_PROTOTYPE_BIND_PROPERTY: &str = "bind";
const FUNCTION_PROTOTYPE_CALL_PROPERTY: &str = "call";
const FUNCTION_PROTOTYPE_TO_STRING_PROPERTY: &str = "toString";

use super::FunctionNewTarget;
use properties::{FunctionPropertyKind, PROTOTYPE_CONSTRUCTOR_PROPERTY};

fn expected_function_local_count(base: usize, binds_arguments: bool) -> Result<usize> {
    let with_function_scope = base
        .checked_add(1)
        .ok_or_else(|| Error::limit("function local scope count overflowed"))?;
    if binds_arguments {
        return with_function_scope
            .checked_add(1)
            .ok_or_else(|| Error::limit("function local scope count overflowed"));
    }
    Ok(with_function_scope)
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
    pub(super) is_async: bool,
    pub(super) class_constructor: bool,
    pub(super) prototype_parent: Option<crate::value::ObjectId>,
    pub(super) new_target_mode: BytecodeNewTargetMode,
}

impl Context {
    pub(super) fn create_bytecode_function(
        &mut self,
        init: &BytecodeFunctionInit<'_>,
    ) -> Result<Value> {
        self.functions.reserve_insert()?;
        let id = FunctionId::new(self.functions.next_index());
        let function = Value::Function(id);
        let prototype = if init.constructable {
            let constructor_key = self.intern_property_key(PROTOTYPE_CONSTRUCTOR_PROPERTY)?;
            let prototype_id = self.objects.create_with_prototype_property(
                init.prototype_parent,
                ObjectPropertyInit::new(
                    constructor_key,
                    PROTOTYPE_CONSTRUCTOR_PROPERTY,
                    function.clone(),
                    PropertyEnumerable::No,
                ),
                constructor_key,
                self.limits.max_objects,
                self.limits.max_object_properties,
            )?;
            Value::Object(prototype_id)
        } else {
            Value::Undefined
        };
        let function_name = self.function_name_value(init.name)?;
        let params = init.bytecode.params();
        let arity = parameters::function_arity(params);
        let prototype_default = init.constructable.then(|| {
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
        let fast_path = FunctionFastPath::compile(
            init.bytecode,
            &param_frames,
            init.new_target_mode,
            init.is_async,
            init.class_constructor,
        )?;
        let upvalues = self.capture_function_upvalues(
            init.static_function_id,
            init.bytecode.capture_bindings(),
            static_binding_layout.as_ref(),
        )?;
        let scope_template = parameters::function_scope_template(
            &param_atoms,
            &param_frames,
            init.bytecode.has_parameter_defaults(),
        )?;
        let param_binding_ids = parameters::function_param_binding_ids(params);
        let metadata_cache_count = Self::function_metadata_cache_count(
            param_binding_ids.len(),
            param_atoms.len(),
            param_frames.len(),
            fast_path.is_some(),
            scope_template.as_deref(),
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
                is_async: init.is_async,
                class_constructor: init.class_constructor,
                super_binding,
                static_parent: None,
                class_fields: None,
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
        if self.function(id)?.is_async {
            let value = self.eval_async_function_with_this(id, args, this_value, new_target)?;
            return Ok(Completion::Normal(value));
        }
        self.eval_function_completion_with_this_and_new_target(id, args, this_value, new_target)?
            .into_call_completion()
    }

    pub(crate) fn eval_function_completion(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Completion> {
        self.eval_function_completion_with_this(id, args, Value::Undefined)
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
            binds_arguments: function.bytecode.uses_arguments()
                && !matches!(function.new_target, FunctionNewTarget::Lexical(_)),
            super_binding: function.super_binding.clone(),
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
            binds_arguments,
            super_binding,
            remember_params,
            scope_template,
        } = self.function_call_setup(id)?;
        let packed_args = if bytecode.has_rest_parameter() {
            Some(self.pack_rest_arguments(bytecode.params(), raw_args.to_vec())?)
        } else {
            None
        };
        let args = packed_args.as_deref().unwrap_or(raw_args);
        let original_args = if binds_arguments {
            Some(raw_args.to_vec())
        } else {
            None
        };
        let has_parameter_defaults = bytecode.has_parameter_defaults();
        let local_base =
            self.push_call_activation(id, upvalues, this_value, new_target, super_binding)?;
        let scope_result = if let Some(template) = scope_template.as_deref() {
            self.function_scope_from_template(template, args)
        } else {
            self.function_scope(&param_atoms, &param_frames, args, has_parameter_defaults)
        };
        let scope = match scope_result {
            Ok(scope) => scope,
            Err(error) => {
                self.leave_function_local_frame(local_base)?;
                self.pop_call_activation(local_base)?;
                return Err(error);
            }
        };
        if let Err(error) =
            self.push_function_binding_storage(local_base, scope, original_args.as_deref())
        {
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
        if CAN_SUSPEND
            && result
                .as_ref()
                .is_ok_and(|completion| matches!(completion, Completion::Suspended(_)))
        {
            return result;
        }
        let binding_result = self.pop_function_binding_storage(local_base, binds_arguments);
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

    pub(crate) fn get_function_property_lookup(
        &mut self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let property_kind = FunctionPropertyKind::from_name(property.name());
        let own_value = {
            let function = self.function(id)?;
            function.properties.own_value(property, property_kind)
        };
        if let Some(value) = own_value {
            return self.checked_value(value);
        }
        let static_parent = self.function(id)?.static_parent.clone();
        if let Some(parent) = static_parent {
            let value = self.get_named(&parent, property.name())?;
            if !matches!(value, Value::Undefined) {
                return Ok(value);
            }
        }
        self.get_function_object_prototype_property(id, property)
    }

    fn get_function_object_prototype_property(
        &mut self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        if !self.should_materialize_function_prototype_for(property) {
            return Ok(Value::Undefined);
        }
        let prototype = self.function_object_prototype_value(id)?;
        let Some(property) = self.known_function_prototype_lookup(property) else {
            return Ok(Value::Undefined);
        };
        let value = get_property(&self.objects, &prototype, property)?;
        self.runtime_property_value(value)
    }

    pub(crate) fn function_object_prototype_value(&mut self, id: FunctionId) -> Result<Value> {
        let is_async = self.function(id)?.is_async;
        if is_async {
            return self.async_function_constructor_prototype_value();
        }
        self.function_constructor_prototype_value()
    }

    pub(crate) fn function_own_property_descriptor_lookup(
        &self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<DataPropertyDescriptor>> {
        let function = self.function(id)?;
        if let Some(descriptor) = function
            .properties
            .intrinsic_descriptor(FunctionPropertyKind::from_name(property.name()))
        {
            return Ok(Some(descriptor));
        }
        Ok(function.properties.own_property_descriptor(property))
    }

    pub(crate) fn has_function_property_lookup(
        &self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let function = self.function(id)?;
        let property_kind = FunctionPropertyKind::from_name(property.name());
        if property_kind.is_intrinsic_slot() && function.properties.has_intrinsic(property_kind) {
            return Ok(true);
        }
        Ok((property_kind.is_prototype() && function.constructable)
            || function.properties.has(property))
    }

    pub(crate) fn set_function_property_key(
        &mut self,
        id: FunctionId,
        property: &str,
        key: PropertyKey,
        value: Value,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let property_kind = FunctionPropertyKind::from_name(property);
        let function = self.function_mut(id)?;
        function
            .properties
            .set(key, property_kind, value, max_properties)
    }

    pub(crate) fn define_function_property_key(
        &mut self,
        id: FunctionId,
        property: &str,
        key: PropertyKey,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let property_kind = FunctionPropertyKind::from_name(property);
        let function = self.function_mut(id)?;
        function
            .properties
            .define_property(key, property_kind, update, max_properties)
    }

    pub(crate) fn delete_function_property_lookup(
        &mut self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let property_kind = FunctionPropertyKind::from_name(property.name());
        let function = self.function_mut(id)?;
        function.properties.delete(property, property_kind)
    }

    pub(crate) fn function_enumerable_keys(&self, id: FunctionId) -> Result<Vec<String>> {
        let function = self.function(id)?;
        function.properties.keys(&self.atoms)
    }

    pub(in crate::runtime) fn set_function_static_parent(
        &mut self,
        id: FunctionId,
        parent: Value,
    ) -> Result<()> {
        self.function_mut(id)?.static_parent = Some(parent);
        Ok(())
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

    fn function_mut(&mut self, id: FunctionId) -> Result<&mut super::Function> {
        self.functions
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("function id is not defined"))
    }

    pub(crate) fn get_native_function_property_lookup(
        &mut self,
        id: NativeFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let property_name = property.name();
        let property_kind = FunctionPropertyKind::from_name(property_name);
        let own_value = {
            let function = self.native_function(id)?;
            function
                .properties()
                .own_value(property, property_kind)
                .or_else(|| function.intrinsic_property(property_name))
        };
        if let Some(value) = own_value {
            return self.checked_value(value);
        }
        self.get_native_function_object_prototype_property(id, property)
    }

    fn get_native_function_object_prototype_property(
        &mut self,
        id: NativeFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        if !self.should_materialize_function_prototype_for(property) {
            return Ok(Value::Undefined);
        }
        let prototype = self.native_function_object_prototype_value(id)?;
        let Some(property) = self.known_function_prototype_lookup(property) else {
            return Ok(Value::Undefined);
        };
        let value = get_property(&self.objects, &prototype, property)?;
        self.runtime_property_value(value)
    }

    fn known_function_prototype_lookup<'a>(
        &self,
        property: PropertyLookup<'a>,
    ) -> Option<PropertyLookup<'a>> {
        let Some(key) = property.key() else {
            return self
                .known_property_key(property.name())
                .map(|key| PropertyLookup::from_key(property.name(), key));
        };
        Some(PropertyLookup::from_key(property.name(), key))
    }

    fn should_materialize_function_prototype_for(&self, property: PropertyLookup<'_>) -> bool {
        property.key().is_some()
            || self.known_property_key(property.name()).is_some()
            || property.name() == PROTOTYPE_CONSTRUCTOR_PROPERTY
            || property.name() == FUNCTION_PROTOTYPE_APPLY_PROPERTY
            || property.name() == FUNCTION_PROTOTYPE_BIND_PROPERTY
            || property.name() == FUNCTION_PROTOTYPE_CALL_PROPERTY
            || property.name() == FUNCTION_PROTOTYPE_TO_STRING_PROPERTY
            || property.name() == OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME
            || property.name() == OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME
    }

    pub(crate) fn native_function_object_prototype_value(
        &mut self,
        id: NativeFunctionId,
    ) -> Result<Value> {
        let kind = self.native_function(id)?.kind();
        if kind == NativeFunctionKind::AsyncFunction {
            return self.function_constructor_value();
        }
        self.function_constructor_prototype_value()
    }

    pub(crate) fn native_function_own_property_descriptor_lookup(
        &self,
        id: NativeFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<DataPropertyDescriptor>> {
        let function = self.native_function(id)?;
        let property_name = property.name();
        let property_kind = FunctionPropertyKind::from_name(property_name);
        if let Some(descriptor) = function.properties().intrinsic_descriptor(property_kind) {
            return Ok(Some(descriptor));
        }
        if let Some(value) = function.intrinsic_property(property_name) {
            return Ok(Some(DataPropertyDescriptor::new(
                value,
                PropertyWritable::No,
                PropertyEnumerable::No,
                PropertyConfigurable::No,
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
        function
            .properties_mut()
            .define_property(key, property_kind, update, max_properties)
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
