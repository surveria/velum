use std::rc::Rc;

use crate::{
    binding_layout::{BindingLayout, BindingOperand},
    bytecode::{BytecodeBlock, BytecodeFunction, BytecodeFunctionParam, BytecodeNewTargetMode},
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::{BindingCell, BindingScope},
    runtime::binding::static_bindings::CompiledBindingFrame,
    runtime::call_args::RuntimeCallArgs,
    runtime::completion::Completion,
    runtime::object::{
        DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyKey, PropertyLookup, PropertyWritable,
    },
    runtime::property::get_property,
    storage::atom::AtomId,
    syntax::{DeclKind, StaticBindingId, StaticFunctionId, StaticName},
    value::{FunctionId, NativeFunctionId, ObjectId, Value},
};

mod intrinsic;
mod properties;
mod upvalues;

use crate::runtime::native::NativeFunctionKind;
pub(super) use properties::{FunctionIntrinsicDefaults, FunctionProperties};

const FUNCTION_PROTOTYPE_BIND_PROPERTY: &str = "bind";
const FUNCTION_PROTOTYPE_CALL_PROPERTY: &str = "call";

use super::FunctionNewTarget;
use properties::{FunctionPropertyKind, PROTOTYPE_CONSTRUCTOR_PROPERTY};

pub(super) struct BytecodeFunctionInit<'a> {
    pub(super) static_function_id: StaticFunctionId,
    pub(super) name: Option<&'a StaticName>,
    pub(super) bytecode: &'a BytecodeFunction,
    pub(super) constructable: bool,
    pub(super) is_async: bool,
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
                None,
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
        let arity = function_arity(params);
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
            param_binding_ids: function_param_binding_ids(params),
            param_atoms,
            bytecode: init.bytecode.clone(),
            upvalues: upvalues.cells,
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
            properties: FunctionProperties::new(prototype, intrinsic_defaults),
            constructable: init.constructable,
            is_async: init.is_async,
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
        let new_target = self.function_direct_call_new_target(id)?;
        if self.function(id)?.is_async {
            return self.eval_async_function_with_this(id, args, this_value, new_target);
        }
        let value = self
            .eval_function_completion_with_this_and_new_target(id, args, this_value, new_target)?
            .into_function_result()?;
        self.runtime_value(value)
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
        let args = args.evaluate();
        let caller_locals = std::mem::take(&mut self.locals);
        let scope = match self.function_scope(
            &param_atoms,
            &param_binding_ids,
            static_binding_layout.as_ref(),
            args,
        ) {
            Ok(scope) => scope,
            Err(error) => {
                self.locals = caller_locals;
                return Err(error);
            }
        };
        self.locals.push(scope);
        self.upvalue_frames.push(upvalues);
        self.this_values.push(this_value);
        self.new_target_values.push(new_target);
        let result = self.eval_function_body(
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
            &param_binding_ids,
            &param_atoms,
            &bytecode,
        );
        let removed_new_target = self.new_target_values.pop();
        let removed_this = self.this_values.pop();
        let removed_upvalues = self.upvalue_frames.pop();
        let removed = self.locals.pop();
        self.locals = caller_locals;
        if removed_this.is_none() {
            return Err(Error::runtime("function this binding disappeared"));
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

    pub(crate) fn function_constructor_prototype(
        &self,
        id: FunctionId,
    ) -> Result<Option<ObjectId>> {
        let function = self.function(id)?;
        if !function.constructable {
            return Err(Error::runtime("function is not a constructor"));
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

    fn function_param_atoms(&mut self, params: &[BytecodeFunctionParam]) -> Result<Rc<[AtomId]>> {
        let mut atoms = Vec::with_capacity(params.len());
        for param in params {
            atoms.push(self.intern_static_name_atom(param.binding().name())?);
        }
        Ok(atoms.into())
    }

    fn function_name_value(&mut self, name: Option<&StaticName>) -> Result<Value> {
        let Some(name) = name.filter(|name| !name.as_str().is_empty()) else {
            return self.heap_string_value("");
        };
        self.heap_string_value(name.as_str())
    }

    fn function_scope(
        &mut self,
        params: &[AtomId],
        binding_ids: &[StaticBindingId],
        layout: Option<&BindingLayout>,
        args: Vec<Value>,
    ) -> Result<BindingScope> {
        if params.len() != binding_ids.len() {
            return Err(Error::runtime("function parameter layout length mismatch"));
        }
        let mut scope = BindingScope::new();
        let mut args = args.into_iter();
        for (atom, binding) in params.iter().copied().zip(binding_ids.iter().copied()) {
            if !scope.contains(atom) {
                self.ensure_extra_binding_capacity(scope.len())?;
            }
            let value = self.runtime_value(args.next().unwrap_or(Value::Undefined))?;
            let cell = BindingCell::new(value, true, DeclKind::Var);
            if let Some(frame) = function_param_frame(binding, layout)? {
                let inserted = scope.insert_or_replace_at_slot(atom, cell, frame.slot())?;
                Self::mark_binding_scope_frame_slot(&mut scope, frame, inserted)?;
            } else {
                scope.insert(atom, cell);
            }
        }
        Ok(scope)
    }

    fn remember_function_params(
        &self,
        binding_ids: &[StaticBindingId],
        atoms: &[AtomId],
    ) -> Result<()> {
        if binding_ids.len() != atoms.len() {
            return Err(Error::runtime("function parameter layout length mismatch"));
        }
        for (binding, atom) in binding_ids.iter().copied().zip(atoms.iter().copied()) {
            self.remember_active_static_binding_id(binding, atom)?;
        }
        Ok(())
    }

    fn eval_function_body(
        &mut self,
        static_name_atom_cache: Option<super::StaticNameAtomCacheHandle>,
        static_binding_cache: Option<super::StaticBindingCacheHandle>,
        static_binding_layout: Option<crate::binding_layout::BindingLayout>,
        param_binding_ids: &[StaticBindingId],
        param_atoms: &[AtomId],
        bytecode: &BytecodeFunction,
    ) -> Result<Completion> {
        match (
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
        ) {
            (
                Some(static_name_atom_cache),
                Some(static_binding_cache),
                Some(static_binding_layout),
            ) => {
                let default_layout = static_binding_layout.clone();
                self.with_static_name_caches(
                    static_name_atom_cache,
                    static_binding_cache,
                    static_binding_layout,
                    |context| {
                        context.remember_function_params(param_binding_ids, param_atoms)?;
                        context.apply_function_param_defaults(
                            param_binding_ids,
                            param_atoms,
                            bytecode.param_defaults(),
                            Some(&default_layout),
                        )?;
                        context
                            .hoist_bytecode_declarations(bytecode.hoist_plan())
                            .and_then(|()| context.eval_bytecode_block(bytecode.body()))
                    },
                )
            }
            (Some(static_name_atom_cache), None, _) => {
                self.with_static_name_atom_cache(static_name_atom_cache, |context| {
                    context.apply_function_param_defaults(
                        param_binding_ids,
                        param_atoms,
                        bytecode.param_defaults(),
                        None,
                    )?;
                    context
                        .hoist_bytecode_declarations(bytecode.hoist_plan())
                        .and_then(|()| context.eval_bytecode_block(bytecode.body()))
                })
            }
            (None, _, _) | (Some(_), Some(_), None) => {
                self.apply_function_param_defaults(
                    param_binding_ids,
                    param_atoms,
                    bytecode.param_defaults(),
                    None,
                )?;
                self.hoist_bytecode_declarations(bytecode.hoist_plan())
                    .and_then(|()| self.eval_bytecode_block(bytecode.body()))
            }
        }
    }

    fn apply_function_param_defaults(
        &mut self,
        binding_ids: &[StaticBindingId],
        atoms: &[AtomId],
        defaults: &[Option<BytecodeBlock>],
        layout: Option<&BindingLayout>,
    ) -> Result<()> {
        if binding_ids.len() != atoms.len() || binding_ids.len() != defaults.len() {
            return Err(Error::runtime("function parameter layout length mismatch"));
        }
        for ((binding, atom), default) in binding_ids
            .iter()
            .copied()
            .zip(atoms.iter().copied())
            .zip(defaults.iter())
        {
            let Some(default) = default else {
                continue;
            };
            let cell = self.function_param_cell(binding, atom, layout)?;
            if !matches!(cell.value(), Value::Undefined) {
                continue;
            }
            let value = self.eval_bytecode_expression(default)?;
            cell.assign("function parameter", value)?;
        }
        Ok(())
    }

    fn function_param_cell(
        &self,
        binding: StaticBindingId,
        atom: AtomId,
        layout: Option<&BindingLayout>,
    ) -> Result<BindingCell> {
        let Some(scope) = self.locals.last() else {
            return Err(Error::runtime("function parameter scope is not active"));
        };
        if let Some(frame) = function_param_frame(binding, layout)? {
            return scope
                .cell_for_slot(atom, frame.slot())
                .ok_or_else(|| Error::runtime("function parameter binding is not defined"));
        }
        scope
            .get(atom)
            .ok_or_else(|| Error::runtime("function parameter binding is not defined"))
    }
}

fn function_param_binding_ids(params: &[BytecodeFunctionParam]) -> Rc<[StaticBindingId]> {
    params
        .iter()
        .map(|param| param.binding().id())
        .collect::<Vec<_>>()
        .into()
}

fn function_arity(params: &[BytecodeFunctionParam]) -> super::FunctionArity {
    let arity = params
        .iter()
        .take_while(|param| !param.has_default())
        .count();
    super::FunctionArity::new(arity)
}

fn function_param_frame(
    binding: StaticBindingId,
    layout: Option<&BindingLayout>,
) -> Result<Option<CompiledBindingFrame>> {
    let Some(layout) = layout else {
        return Ok(None);
    };
    let Some(operand) = layout.operand_for_binding_id(binding)? else {
        return Ok(None);
    };
    match operand {
        BindingOperand::Local { scope, slot } => Ok(Some(CompiledBindingFrame::local(
            scope,
            crate::runtime::binding::scope::BindingSlot::from_index(slot.index()?),
        ))),
        BindingOperand::Global { .. } | BindingOperand::Upvalue { .. } => Err(Error::runtime(
            "function parameter binding layout is not a local slot",
        )),
        BindingOperand::Unresolved => Ok(None),
    }
}

impl super::FunctionArity {
    fn value(self) -> Result<Value> {
        let length = u32::try_from(self.as_usize())
            .map_err(|_| Error::limit("function parameter count exceeded supported range"))?;
        Ok(Value::Number(f64::from(length)))
    }
}
