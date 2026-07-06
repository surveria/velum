use std::rc::Rc;

use crate::{
    ast::{DeclKind, StaticBinding, StaticBindingId, StaticFunctionId, StaticName},
    atom::AtomId,
    binding_layout::{BindingLayout, BindingOperand},
    bytecode::BytecodeFunction,
    error::{Error, Result},
    runtime::Context,
    runtime::call_args::RuntimeCallArgs,
    runtime::completion::Completion,
    runtime::object::{
        DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyKey, PropertyLookup, PropertyWritable,
    },
    runtime::scope::{BindingCell, BindingScope},
    value::{FunctionId, NativeFunctionId, ObjectId, Value},
};

mod intrinsic;
mod properties;
mod upvalues;

pub(super) use properties::{FunctionIntrinsicDefaults, FunctionProperties};

use super::static_bindings::CompiledBindingFrame;
use properties::{FunctionPropertyKind, PROTOTYPE_CONSTRUCTOR_PROPERTY};

impl Context {
    pub(crate) fn create_bytecode_function(
        &mut self,
        static_function_id: StaticFunctionId,
        name: Option<&StaticName>,
        params: &Rc<[StaticBinding]>,
        bytecode: &BytecodeFunction,
        constructable: bool,
    ) -> Result<Value> {
        if !constructable && name.is_none() {
            return Err(Error::runtime("method function name disappeared"));
        }
        let id = FunctionId::new(self.functions.len());
        let function = Value::Function(id);
        let prototype = if constructable {
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
        let function_name = self.function_name_value(name)?;
        let arity = super::FunctionArity::new(params.len());
        let prototype_default = constructable.then(|| {
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
            static_function_id,
            bytecode.capture_bindings(),
            static_binding_layout.as_ref(),
        )?;
        let captures = super::FunctionCaptures::from_current_locals(
            &self.locals,
            static_binding_layout.is_some(),
            &upvalues.cells,
            upvalues.needs_legacy_scope_fallback,
        );
        self.functions.push(super::Function {
            param_binding_ids: function_param_binding_ids(params),
            param_atoms,
            bytecode: bytecode.clone(),
            captures,
            upvalues: upvalues.cells,
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
            properties: FunctionProperties::new(prototype, intrinsic_defaults),
            constructable,
        });
        Ok(function)
    }

    pub(crate) fn eval_function_with_this(
        &mut self,
        id: FunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: Value,
    ) -> Result<Value> {
        let value = self
            .eval_function_completion_with_this(id, args, this_value)?
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
        let (
            param_atoms,
            param_binding_ids,
            bytecode,
            captures,
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
                function.captures.call_locals(),
                Rc::clone(&function.upvalues),
                function.static_name_atom_cache.clone(),
                function.static_binding_cache.clone(),
                function.static_binding_layout.clone(),
            )
        };
        let args = args.evaluate();
        let caller_locals = std::mem::replace(&mut self.locals, captures);
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
        let result = self.eval_function_body(
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
            &param_binding_ids,
            &param_atoms,
            &bytecode,
        );
        let removed_this = self.this_values.pop();
        let removed_upvalues = self.upvalue_frames.pop();
        let removed = self.locals.pop();
        self.locals = caller_locals;
        if removed_this.is_none() {
            return Err(Error::runtime("function this binding disappeared"));
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
        &self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let function = self.function(id)?;
        let property_kind = FunctionPropertyKind::from_name(property.name());
        if let Some(value) = function.properties.intrinsic_value(property_kind) {
            return self.checked_value(value);
        }

        let value = if property_kind.is_prototype() {
            function.properties.prototype()
        } else {
            function.properties.get(property)
        };
        self.checked_value(value)
    }

    pub(crate) fn function_own_property_descriptor(
        &self,
        id: FunctionId,
        property: &str,
    ) -> Result<Option<DataPropertyDescriptor>> {
        self.function_own_property_descriptor_lookup(id, self.property_lookup(property))
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

    pub(crate) fn has_function_property(&self, id: FunctionId, property: &str) -> Result<bool> {
        self.has_function_property_lookup(id, self.property_lookup(property))
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

    pub(crate) fn define_function_property(
        &mut self,
        id: FunctionId,
        property: &str,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let property_kind = FunctionPropertyKind::from_name(property);
        let key = self.intern_property_key(property)?;
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
        &self,
        id: NativeFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<Value> {
        let function = self.native_function(id)?;
        let property_name = property.name();
        let property_kind = FunctionPropertyKind::from_name(property_name);
        if let Some(value) = function.properties().intrinsic_value(property_kind) {
            return self.checked_value(value);
        }

        let value = if property_kind.is_prototype() {
            function.properties().prototype()
        } else {
            function
                .intrinsic_property(property_name)
                .unwrap_or_else(|| function.properties().get(property))
        };
        self.checked_value(value)
    }

    pub(crate) fn native_function_own_property_descriptor(
        &self,
        id: NativeFunctionId,
        property: &str,
    ) -> Result<Option<DataPropertyDescriptor>> {
        self.native_function_own_property_descriptor_lookup(id, self.property_lookup(property))
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

    pub(crate) fn has_native_function_property(
        &self,
        id: NativeFunctionId,
        property: &str,
    ) -> Result<bool> {
        self.has_native_function_property_lookup(id, self.property_lookup(property))
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

    pub(crate) fn define_native_function_property(
        &mut self,
        id: NativeFunctionId,
        property: &str,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        let property_kind = FunctionPropertyKind::from_name(property);
        if self.native_function(id)?.has_intrinsic_property(property) {
            return Ok(());
        }
        let max_properties = self.limits.max_object_properties;
        let key = self.intern_property_key(property)?;
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

    fn function_param_atoms(&mut self, params: &[StaticBinding]) -> Result<Rc<[AtomId]>> {
        let mut atoms = Vec::with_capacity(params.len());
        for param in params {
            atoms.push(self.intern_static_name_atom(param.name())?);
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
            ) => self.with_static_name_caches(
                static_name_atom_cache,
                static_binding_cache,
                static_binding_layout,
                |context| {
                    context.remember_function_params(param_binding_ids, param_atoms)?;
                    context
                        .hoist_bytecode_var_declarations(bytecode.hoist_plan())
                        .and_then(|()| context.eval_bytecode_block(bytecode.body()))
                },
            ),
            (Some(static_name_atom_cache), None, _) => {
                self.with_static_name_atom_cache(static_name_atom_cache, |context| {
                    context
                        .hoist_bytecode_var_declarations(bytecode.hoist_plan())
                        .and_then(|()| context.eval_bytecode_block(bytecode.body()))
                })
            }
            (None, _, _) | (Some(_), Some(_), None) => self
                .hoist_bytecode_var_declarations(bytecode.hoist_plan())
                .and_then(|()| self.eval_bytecode_block(bytecode.body())),
        }
    }
}

fn function_param_binding_ids(params: &[StaticBinding]) -> Rc<[StaticBindingId]> {
    params
        .iter()
        .map(StaticBinding::id)
        .collect::<Vec<_>>()
        .into()
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
            crate::runtime::scope::BindingSlot::from_index(slot.index()?),
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
