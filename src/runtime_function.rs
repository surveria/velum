use std::rc::Rc;

use crate::{
    ast::{DeclKind, Expr, StaticBinding, StaticBindingId, StaticFunctionId, StaticName, Stmt},
    atom::AtomId,
    binding_layout::BindingLayout,
    binding_layout_types::BindingOperand,
    error::{Error, Result},
    runtime::Context,
    runtime_completion::Completion,
    runtime_object::{
        DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyKey, PropertyLookup, PropertyWritable,
    },
    runtime_scope::{BindingCell, BindingScope},
    value::{FunctionId, NativeFunctionId, ObjectId, Value},
};

use super::runtime_function_properties::{
    FUNCTION_LENGTH_PROPERTY, FUNCTION_NAME_PROPERTY, FUNCTION_PROTOTYPE_PROPERTY,
    FunctionProperties, PROTOTYPE_CONSTRUCTOR_PROPERTY,
};
use super::runtime_static_bindings::CompiledBindingFrame;

impl Context {
    pub(crate) fn create_function(
        &mut self,
        id: StaticFunctionId,
        name: Option<&StaticName>,
        params: &Rc<[StaticBinding]>,
        body: &Rc<[Stmt]>,
    ) -> Result<Value> {
        self.create_function_with_properties(id, name, params, body, true)
    }

    pub(crate) fn create_method_function(
        &mut self,
        id: StaticFunctionId,
        name: &StaticName,
        params: &Rc<[StaticBinding]>,
        body: &Rc<[Stmt]>,
    ) -> Result<Value> {
        self.create_function_with_properties(id, Some(name), params, body, false)
    }

    fn create_function_with_properties(
        &mut self,
        static_function_id: StaticFunctionId,
        name: Option<&StaticName>,
        params: &Rc<[StaticBinding]>,
        body: &Rc<[Stmt]>,
        constructable: bool,
    ) -> Result<Value> {
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
        let function_name = self.function_name(name)?;
        let param_atoms = self.function_param_atoms(params)?;
        let static_name_atom_cache = self.current_static_name_atom_cache();
        let static_binding_cache = self.current_static_binding_cache();
        let static_binding_layout = self.current_static_binding_layout();
        let upvalues = self.capture_function_upvalues(
            static_function_id,
            body,
            static_binding_layout.as_ref(),
        )?;
        let captures = super::FunctionCaptures::from_current_locals(
            &self.locals,
            static_binding_layout.is_some(),
            &upvalues.cells,
            upvalues.needs_legacy_scope_fallback,
        );
        self.functions.push(super::Function {
            name: function_name,
            arity: super::FunctionArity::new(params.len()),
            param_binding_ids: function_param_binding_ids(params),
            param_atoms,
            body: Rc::clone(body),
            captures,
            upvalues: upvalues.cells,
            static_name_atom_cache,
            static_binding_cache,
            static_binding_layout,
            properties: FunctionProperties::new(prototype),
            constructable,
        });
        Ok(function)
    }

    pub(crate) fn eval_function(&mut self, id: FunctionId, args: &[Expr]) -> Result<Value> {
        self.eval_function_with_this(id, args, Value::Undefined)
    }

    pub(crate) fn eval_function_with_this(
        &mut self,
        id: FunctionId,
        args: &[Expr],
        this_value: Value,
    ) -> Result<Value> {
        let value = self
            .eval_function_completion_with_this(id, args, this_value)?
            .into_function_result()?;
        self.checked_value(value)
    }

    pub(crate) fn eval_function_completion(
        &mut self,
        id: FunctionId,
        args: &[Expr],
    ) -> Result<Completion> {
        self.eval_function_completion_with_this(id, args, Value::Undefined)
    }

    pub(crate) fn eval_function_completion_with_this(
        &mut self,
        id: FunctionId,
        args: &[Expr],
        this_value: Value,
    ) -> Result<Completion> {
        let (
            param_atoms,
            param_binding_ids,
            body,
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
                Rc::clone(&function.body),
                function.captures.call_locals(),
                Rc::clone(&function.upvalues),
                function.static_name_atom_cache.clone(),
                function.static_binding_cache.clone(),
                function.static_binding_layout.clone(),
            )
        };
        let args = self.eval_args(args)?;
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
            &body,
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
        let property_name = property.name();
        if let Some(default_descriptor) =
            function_default_intrinsic_descriptor(function, &self.atoms, property_name)?
            && let Some(value) = function
                .properties
                .intrinsic_value(property_name, default_descriptor)
        {
            return self.checked_value(value);
        }

        let value = match property_name {
            FUNCTION_PROTOTYPE_PROPERTY => function.properties.prototype(),
            _ => function.properties.get(property),
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
        if let Some(descriptor) =
            function_intrinsic_descriptor(function, &self.atoms, property.name())?
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
        let property_name = property.name();
        if matches!(
            property_name,
            FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY
        ) && function.properties.has_intrinsic(property_name)
        {
            return Ok(true);
        }
        Ok(
            (property_name == FUNCTION_PROTOTYPE_PROPERTY && function.constructable)
                || function.properties.has(property),
        )
    }

    pub(crate) fn set_function_property_key(
        &mut self,
        id: FunctionId,
        property: &str,
        key: PropertyKey,
        value: Value,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let default_intrinsic = {
            let function = self.function(id)?;
            function_default_intrinsic_descriptor(function, &self.atoms, property)?
        };
        let function = self.function_mut(id)?;
        function
            .properties
            .set(key, property, value, max_properties, default_intrinsic)
    }

    pub(crate) fn define_function_property(
        &mut self,
        id: FunctionId,
        property: &str,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let default_intrinsic = {
            let function = self.function(id)?;
            function_default_intrinsic_descriptor(function, &self.atoms, property)?
        };
        let key = self.intern_property_key(property)?;
        let function = self.function_mut(id)?;
        function.properties.define_property(
            key,
            property,
            update,
            max_properties,
            default_intrinsic,
        )
    }

    pub(crate) fn delete_function_property_lookup(
        &mut self,
        id: FunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let default_intrinsic = {
            let function = self.function(id)?;
            function_default_intrinsic_descriptor(function, &self.atoms, property.name())?
        };
        let function = self.function_mut(id)?;
        Ok(function.properties.delete(property, default_intrinsic))
    }

    pub(crate) fn function_enumerable_keys(&self, id: FunctionId) -> Result<Vec<String>> {
        let function = self.function(id)?;
        let length =
            function_default_intrinsic_descriptor(function, &self.atoms, FUNCTION_LENGTH_PROPERTY)?;
        let name =
            function_default_intrinsic_descriptor(function, &self.atoms, FUNCTION_NAME_PROPERTY)?;
        function.properties.keys(&self.atoms, length, name)
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
        if let Some(default_descriptor) =
            native_function_default_intrinsic_descriptor(function, property_name)
            && let Some(value) = function
                .properties()
                .intrinsic_value(property_name, default_descriptor)
        {
            return self.checked_value(value);
        }

        let value = match property_name {
            FUNCTION_PROTOTYPE_PROPERTY => function.properties().prototype(),
            _ => function
                .intrinsic_property(property_name)
                .unwrap_or_else(|| function.properties().get(property)),
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
        if let Some(descriptor) = native_function_intrinsic_descriptor(function, property_name) {
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
        if matches!(
            property_name,
            FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY
        ) && function.properties().has_intrinsic(property_name)
        {
            return Ok(true);
        }
        Ok((property_name == FUNCTION_PROTOTYPE_PROPERTY)
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
        if property == FUNCTION_PROTOTYPE_PROPERTY {
            self.native_function(id)?;
            return Ok(());
        }
        let default_intrinsic =
            native_function_default_intrinsic_descriptor(self.native_function(id)?, property);
        if self.native_function(id)?.has_intrinsic_property(property) {
            return Ok(());
        }
        let max_properties = self.limits.max_object_properties;
        let function = self.native_function_mut(id)?;
        function
            .properties_mut()
            .set(key, property, value, max_properties, default_intrinsic)
    }

    pub(crate) fn define_native_function_property(
        &mut self,
        id: NativeFunctionId,
        property: &str,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        let default_intrinsic =
            native_function_default_intrinsic_descriptor(self.native_function(id)?, property);
        if self.native_function(id)?.has_intrinsic_property(property) {
            return Ok(());
        }
        let max_properties = self.limits.max_object_properties;
        let key = self.intern_property_key(property)?;
        let function = self.native_function_mut(id)?;
        function.properties_mut().define_property(
            key,
            property,
            update,
            max_properties,
            default_intrinsic,
        )
    }

    pub(crate) fn delete_native_function_property_lookup(
        &mut self,
        id: NativeFunctionId,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let property_name = property.name();
        let default_intrinsic =
            native_function_default_intrinsic_descriptor(self.native_function(id)?, property_name);
        if self
            .native_function(id)?
            .has_intrinsic_property(property_name)
        {
            return Ok(false);
        }
        let function = self.native_function_mut(id)?;
        Ok(function
            .properties_mut()
            .delete(property, default_intrinsic))
    }

    pub(crate) fn native_function_enumerable_keys(
        &self,
        id: NativeFunctionId,
    ) -> Result<Vec<String>> {
        let function = self.native_function(id)?;
        let length =
            native_function_default_intrinsic_descriptor(function, FUNCTION_LENGTH_PROPERTY);
        let name = native_function_default_intrinsic_descriptor(function, FUNCTION_NAME_PROPERTY);
        function.properties().keys(&self.atoms, length, name)
    }

    fn eval_args(&mut self, args: &[Expr]) -> Result<Vec<Value>> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval_expr(arg)?);
        }
        Ok(values)
    }

    fn function_param_atoms(&mut self, params: &[StaticBinding]) -> Result<Rc<[AtomId]>> {
        let mut atoms = Vec::with_capacity(params.len());
        for param in params {
            atoms.push(self.intern_static_name_atom(param.name())?);
        }
        Ok(atoms.into())
    }

    fn function_name(&mut self, name: Option<&StaticName>) -> Result<super::FunctionName> {
        let Some(name) = name.filter(|name| !name.as_str().is_empty()) else {
            return Ok(super::FunctionName::anonymous());
        };
        Ok(super::FunctionName::new(
            self.intern_static_name_atom(name)?,
        ))
    }

    fn function_scope(
        &self,
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
            let value = args.next().unwrap_or(Value::Undefined);
            self.checked_value(value.clone())?;
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
        body: &[Stmt],
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
                        .hoist_var_declarations(body)
                        .and_then(|()| context.eval_block(body))
                },
            ),
            (Some(static_name_atom_cache), None, _) => {
                self.with_static_name_atom_cache(static_name_atom_cache, |context| {
                    context
                        .hoist_var_declarations(body)
                        .and_then(|()| context.eval_block(body))
                })
            }
            (None, _, _) | (Some(_), Some(_), None) => self
                .hoist_var_declarations(body)
                .and_then(|()| self.eval_block(body)),
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
            crate::runtime_scope::BindingSlot::from_index(slot.index()?),
        ))),
        BindingOperand::Global { .. } | BindingOperand::Upvalue { .. } => Err(Error::runtime(
            "function parameter binding layout is not a local slot",
        )),
        BindingOperand::Unresolved => Ok(None),
    }
}

impl super::Function {
    fn length(&self) -> Result<f64> {
        let length = u32::try_from(self.arity.as_usize())
            .map_err(|_| Error::limit("function parameter count exceeded supported range"))?;
        Ok(f64::from(length))
    }
}

fn function_intrinsic_descriptor(
    function: &super::Function,
    atoms: &crate::atom::AtomTable,
    property: &str,
) -> Result<Option<DataPropertyDescriptor>> {
    if let Some(default) = function_default_intrinsic_descriptor(function, atoms, property)? {
        return Ok(function.properties.intrinsic_descriptor(property, default));
    }
    let descriptor = match property {
        FUNCTION_PROTOTYPE_PROPERTY if function.constructable => DataPropertyDescriptor::new(
            function.properties.prototype(),
            PropertyWritable::Yes,
            PropertyEnumerable::No,
            PropertyConfigurable::No,
        ),
        _ => return Ok(None),
    };
    Ok(Some(descriptor))
}

fn function_default_intrinsic_descriptor(
    function: &super::Function,
    atoms: &crate::atom::AtomTable,
    property: &str,
) -> Result<Option<DataPropertyDescriptor>> {
    let descriptor = match property {
        FUNCTION_LENGTH_PROPERTY => DataPropertyDescriptor::new(
            Value::Number(function.length()?),
            PropertyWritable::No,
            PropertyEnumerable::No,
            PropertyConfigurable::Yes,
        ),
        FUNCTION_NAME_PROPERTY => DataPropertyDescriptor::new(
            function.name.value(atoms)?,
            PropertyWritable::No,
            PropertyEnumerable::No,
            PropertyConfigurable::Yes,
        ),
        _ => return Ok(None),
    };
    Ok(Some(descriptor))
}

fn native_function_intrinsic_descriptor(
    function: &super::runtime_native::NativeFunction,
    property: &str,
) -> Option<DataPropertyDescriptor> {
    if let Some(default) = native_function_default_intrinsic_descriptor(function, property) {
        return function
            .properties()
            .intrinsic_descriptor(property, default);
    }
    let descriptor = match property {
        FUNCTION_PROTOTYPE_PROPERTY => DataPropertyDescriptor::new(
            function.properties().prototype(),
            PropertyWritable::No,
            PropertyEnumerable::No,
            PropertyConfigurable::No,
        ),
        _ => return None,
    };
    Some(descriptor)
}

fn native_function_default_intrinsic_descriptor(
    function: &super::runtime_native::NativeFunction,
    property: &str,
) -> Option<DataPropertyDescriptor> {
    let descriptor = match property {
        FUNCTION_LENGTH_PROPERTY => DataPropertyDescriptor::new(
            Value::Number(function.length()),
            PropertyWritable::No,
            PropertyEnumerable::No,
            PropertyConfigurable::Yes,
        ),
        FUNCTION_NAME_PROPERTY => DataPropertyDescriptor::new(
            Value::String(function.name().to_owned()),
            PropertyWritable::No,
            PropertyEnumerable::No,
            PropertyConfigurable::Yes,
        ),
        _ => return None,
    };
    Some(descriptor)
}
