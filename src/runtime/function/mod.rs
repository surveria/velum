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
mod intrinsic;
mod parameters;
mod properties;
mod upvalues;

use crate::runtime::native::{
    NativeFunctionKind, OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME,
    OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME,
};
use parameters::FunctionParameterState;
pub(super) use properties::{FunctionIntrinsicDefaults, FunctionProperties};

const FUNCTION_PROTOTYPE_BIND_PROPERTY: &str = "bind";
const FUNCTION_PROTOTYPE_CALL_PROPERTY: &str = "call";

use super::FunctionNewTarget;
use properties::{FunctionPropertyKind, PROTOTYPE_CONSTRUCTOR_PROPERTY};

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

/// A resolved public instance field: the property key computed at class
/// definition time plus the lazily evaluated initializer block.
#[derive(Debug)]
pub(in crate::runtime) struct ResolvedClassField {
    pub(in crate::runtime) key: crate::runtime::object::PropertyKey,
    pub(in crate::runtime) name: String,
    pub(in crate::runtime) initializer: Option<crate::bytecode::BytecodeBlock>,
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
        let id = FunctionId::new(self.functions.len());
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
        let upvalues = self.capture_function_upvalues(
            init.static_function_id,
            init.bytecode.capture_bindings(),
            static_binding_layout.as_ref(),
        )?;
        self.functions.push(super::Function {
            param_binding_ids: parameters::function_param_binding_ids(params),
            param_atoms,
            bytecode: init.bytecode.clone(),
            source: None,
            upvalues: upvalues.cells,
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
            properties: FunctionProperties::new(prototype, intrinsic_defaults),
            constructable: init.constructable,
            is_async: init.is_async,
            class_constructor: init.class_constructor,
            super_binding: if init.new_target_mode == BytecodeNewTargetMode::Lexical {
                self.super_frames.last().cloned().flatten()
            } else {
                None
            },
            static_parent: None,
            class_fields: None,
            new_target: FunctionNewTarget::from_mode(
                init.new_target_mode,
                self.current_new_target()?,
            ),
        });
        Ok(function)
    }

    pub(crate) fn eval_function_with_this(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
    ) -> Result<Value> {
        self.reject_class_constructor_call(id)?;
        let new_target = self.function_direct_call_new_target(id)?;
        if self.function(id)?.is_async {
            return self.eval_async_function_with_this(id, args, this_value, new_target);
        }
        let value = self
            .eval_function_completion_with_this_and_new_target(id, args, this_value, new_target)?
            .into_function_result()?;
        self.runtime_value(value)
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

    pub(crate) fn eval_function_completion_with_this_and_new_target(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
        new_target: Value,
    ) -> Result<Completion> {
        self.call_depth = self
            .call_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("call stack depth overflowed"))?;
        if self.call_depth > self.limits.max_expression_depth {
            self.call_depth = self.call_depth.saturating_sub(1);
            return Err(Error::limit(format!(
                "call stack depth exceeded {}",
                self.limits.max_expression_depth
            )));
        }
        let result =
            self.eval_function_completion_with_this_inner(id, args, this_value, new_target);
        self.call_depth = self.call_depth.saturating_sub(1);
        result
    }

    fn function_direct_call_new_target(&self, id: FunctionId) -> Result<Value> {
        match &self.function(id)?.new_target {
            FunctionNewTarget::Own => Ok(Value::Undefined),
            FunctionNewTarget::Lexical(value) => Ok(value.clone()),
        }
    }

    fn eval_function_completion_with_this_inner(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
        new_target: Value,
    ) -> Result<Completion> {
        let (
            param_atoms,
            param_binding_ids,
            bytecode,
            upvalues,
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
        ) = {
            let function = self.function(id)?;
            (
                Rc::clone(&function.param_atoms),
                Rc::clone(&function.param_binding_ids),
                function.bytecode.clone(),
                Rc::clone(&function.upvalues),
                function.static_name_atom_cache.clone(),
                function.static_binding_cache.clone(),
                function.static_binding_layout.clone(),
            )
        };
        let args = args.to_owned_values();
        let original_args = args.clone();
        let args = self.pack_rest_arguments(bytecode.params(), args)?;
        let has_parameter_defaults = bytecode.has_parameter_defaults();
        let caller_locals = std::mem::take(&mut self.locals);
        let scope = match self.function_scope(
            &param_atoms,
            &param_binding_ids,
            static_binding_layout.as_ref(),
            &args,
            has_parameter_defaults,
        ) {
            Ok(scope) => scope,
            Err(error) => {
                self.locals = caller_locals;
                return Err(error);
            }
        };
        let binds_arguments = bytecode.uses_arguments()
            && self.function(id).is_ok_and(|function| {
                !matches!(function.new_target, FunctionNewTarget::Lexical(_))
            });
        if binds_arguments {
            match self.arguments_wrapper_scope(&original_args) {
                Ok(wrapper) => self.locals.push(wrapper),
                Err(error) => {
                    self.locals = caller_locals;
                    return Err(error);
                }
            }
        }
        self.locals.push(scope);
        self.upvalue_frames.push(upvalues);
        self.this_values.push(this_value);
        self.new_target_values.push(new_target);
        self.super_frames
            .push(self.function(id).ok().and_then(|f| f.super_binding.clone()));
        let result = self.eval_function_body(
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
            FunctionParameterState::new(&param_binding_ids, &param_atoms, &args),
            &bytecode,
        );
        let removed_super = self.super_frames.pop();
        let removed_new_target = self.new_target_values.pop();
        let removed_this = self.this_values.pop();
        let removed_upvalues = self.upvalue_frames.pop();
        let removed = self.locals.pop();
        let removed_arguments_scope = if binds_arguments {
            self.locals.pop().is_some()
        } else {
            true
        };
        self.locals = caller_locals;
        if removed_this.is_none() {
            return Err(Error::runtime("function this binding disappeared"));
        }
        if removed_super.is_none() {
            return Err(Error::runtime("function super frame disappeared"));
        }
        if removed_new_target.is_none() {
            return Err(Error::runtime("function new.target binding disappeared"));
        }
        if removed_upvalues.is_none() {
            return Err(Error::runtime("function upvalue frame disappeared"));
        }
        if removed.is_none() {
            return Err(Error::runtime("function scope disappeared"));
        }
        if !removed_arguments_scope {
            return Err(Error::runtime("function arguments scope disappeared"));
        }
        result
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
            let value = self.get_property_value(&parent, property.name())?;
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
        Ok(function.properties.delete(property, property_kind))
    }

    pub(crate) fn function_enumerable_keys(&self, id: FunctionId) -> Result<Vec<String>> {
        let function = self.function(id)?;
        function.properties.keys(&self.atoms)
    }

    /// Runs a parent class constructor for `super(...)`: the current `this`
    /// is initialized in place, the parent's return-object override is
    /// ignored, and throws propagate as completions.
    pub(in crate::runtime) fn eval_class_super_constructor_completion(
        &mut self,
        id: FunctionId,
        args: &[Value],
        this_value: &Value,
        new_target: Value,
    ) -> Result<Completion> {
        self.initialize_class_fields(id, this_value)?;
        match self.eval_function_completion_with_this_and_new_target(
            id,
            RuntimeCallArgs::values(args),
            this_value.clone(),
            new_target,
        )? {
            Completion::Normal(_) | Completion::Return(_) => {
                Ok(Completion::Normal(Value::Undefined))
            }
            completion @ Completion::Throw(_) => Ok(completion),
            Completion::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Completion::Continue(_) => Err(Error::runtime("continue statement outside loop")),
        }
    }

    pub(in crate::runtime) fn set_function_super_binding(
        &mut self,
        id: FunctionId,
        binding: Rc<FunctionSuperBinding>,
    ) -> Result<()> {
        self.function_mut(id)?.super_binding = Some(binding);
        Ok(())
    }

    pub(in crate::runtime) fn set_function_class_fields(
        &mut self,
        id: FunctionId,
        fields: Rc<[ResolvedClassField]>,
    ) -> Result<()> {
        self.function_mut(id)?.class_fields = Some(fields);
        Ok(())
    }

    /// True when the function is a derived class constructor whose fields
    /// initialize after `super()` instead of at construction entry.
    pub(in crate::runtime) fn is_derived_class_constructor(&self, id: FunctionId) -> bool {
        self.function(id).is_ok_and(|function| {
            function
                .super_binding
                .as_ref()
                .is_some_and(|binding| binding.constructor.is_some())
        })
    }

    /// Defines the class instance fields on a freshly created object with
    /// `this` bound to it while initializers run, in declaration order.
    pub(in crate::runtime) fn initialize_class_fields(
        &mut self,
        id: FunctionId,
        instance: &Value,
    ) -> Result<()> {
        let Some(fields) = self.function(id)?.class_fields.clone() else {
            return Ok(());
        };
        let Value::Object(object_id) = instance else {
            return Ok(());
        };
        for field in fields.iter() {
            self.this_values.push(instance.clone());
            let value = field
                .initializer
                .as_ref()
                .map_or(Ok(Completion::Normal(Value::Undefined)), |initializer| {
                    self.eval_bytecode_block(initializer)
                });
            if self.this_values.pop().is_none() {
                return Err(Error::runtime("class field this binding disappeared"));
            }
            let value = value?.into_result()?;
            let update = crate::runtime::object::PropertyUpdate::Data(
                crate::runtime::object::DataPropertyUpdate::new(
                    Some(value),
                    Some(crate::runtime::object::PropertyWritable::Yes),
                    Some(crate::runtime::object::PropertyEnumerable::Yes),
                    Some(crate::runtime::object::PropertyConfigurable::Yes),
                ),
            );
            self.objects.define_property(
                *object_id,
                field.key,
                &field.name,
                update,
                self.limits.max_object_properties,
            )?;
        }
        Ok(())
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
        self.super_frames.last().cloned().flatten()
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
            | Value::HostFunction(_)
            | Value::Error(_) => Ok(None),
        }
    }

    fn function(&self, id: FunctionId) -> Result<&super::Function> {
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
            || property.name() == FUNCTION_PROTOTYPE_BIND_PROPERTY
            || property.name() == FUNCTION_PROTOTYPE_CALL_PROPERTY
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
        Ok(property_kind.is_prototype()
            || function.has_intrinsic_property(property_name)
            || function.properties().has(property))
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
        Ok(function.properties_mut().delete(property, property_kind))
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
