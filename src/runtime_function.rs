use std::rc::Rc;

use crate::{
    ast::{DeclKind, Expr, Stmt},
    atom::AtomId,
    error::{Error, Result},
    runtime::Context,
    runtime_completion::Completion,
    runtime_object::{
        DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyWritable,
    },
    runtime_scope::{BindingCell, BindingScope},
    value::{FunctionId, NativeFunctionId, ObjectId, Value},
};

use super::runtime_function_properties::{
    FUNCTION_LENGTH_PROPERTY, FUNCTION_NAME_PROPERTY, FUNCTION_PROTOTYPE_PROPERTY,
    FunctionProperties, PROTOTYPE_CONSTRUCTOR_PROPERTY,
};

impl Context {
    pub(crate) fn create_function(
        &mut self,
        name: Option<&str>,
        params: &Rc<[String]>,
        body: &Rc<[Stmt]>,
    ) -> Result<Value> {
        self.create_function_with_properties(name, params, body, true)
    }

    pub(crate) fn create_method_function(
        &mut self,
        name: &str,
        params: &Rc<[String]>,
        body: &Rc<[Stmt]>,
    ) -> Result<Value> {
        self.create_function_with_properties(Some(name), params, body, false)
    }

    fn create_function_with_properties(
        &mut self,
        name: Option<&str>,
        params: &Rc<[String]>,
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
        self.functions.push(super::Function {
            name: function_name,
            arity: super::FunctionArity::new(params.len()),
            param_atoms,
            body: Rc::clone(body),
            captures: self.locals.clone(),
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
        let (param_atoms, body, captures) = {
            let function = self.function(id)?;
            (
                Rc::clone(&function.param_atoms),
                Rc::clone(&function.body),
                function.captures.clone(),
            )
        };
        let args = self.eval_args(args)?;
        let caller_locals = std::mem::replace(&mut self.locals, captures);
        let scope = match self.function_scope(&param_atoms, args) {
            Ok(scope) => scope,
            Err(error) => {
                self.locals = caller_locals;
                return Err(error);
            }
        };
        self.locals.push(scope);
        self.this_values.push(this_value);
        let result = self
            .hoist_var_declarations(&body)
            .and_then(|()| self.eval_block(&body));
        let removed_this = self.this_values.pop();
        let removed = self.locals.pop();
        self.locals = caller_locals;
        if removed_this.is_none() {
            return Err(Error::runtime("function this binding disappeared"));
        }
        if removed.is_none() {
            return Err(Error::runtime("function scope disappeared"));
        }
        result
    }

    pub(crate) fn get_function_property(&self, id: FunctionId, property: &str) -> Result<Value> {
        let function = self.function(id)?;
        if let Some(default_descriptor) =
            function_default_intrinsic_descriptor(function, &self.atoms, property)?
            && let Some(value) = function
                .properties
                .intrinsic_value(property, default_descriptor)
        {
            return self.checked_value(value);
        }

        let value = match property {
            FUNCTION_PROTOTYPE_PROPERTY => function.properties.prototype(),
            _ => function.properties.get(self.property_lookup(property)),
        };
        self.checked_value(value)
    }

    pub(crate) fn function_own_property_descriptor(
        &self,
        id: FunctionId,
        property: &str,
    ) -> Result<Option<DataPropertyDescriptor>> {
        let function = self.function(id)?;
        if let Some(descriptor) = function_intrinsic_descriptor(function, &self.atoms, property)? {
            return Ok(Some(descriptor));
        }
        Ok(function
            .properties
            .own_property_descriptor(self.property_lookup(property)))
    }

    pub(crate) fn has_function_property(&self, id: FunctionId, property: &str) -> Result<bool> {
        let function = self.function(id)?;
        if matches!(property, FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY)
            && function.properties.has_intrinsic(property)
        {
            return Ok(true);
        }
        Ok(
            (property == FUNCTION_PROTOTYPE_PROPERTY && function.constructable)
                || function.properties.has(self.property_lookup(property)),
        )
    }

    pub(crate) fn set_function_property(
        &mut self,
        id: FunctionId,
        property: &str,
        value: Value,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let default_intrinsic = {
            let function = self.function(id)?;
            function_default_intrinsic_descriptor(function, &self.atoms, property)?
        };
        let key = self.intern_property_key(property)?;
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

    pub(crate) fn delete_function_property(
        &mut self,
        id: FunctionId,
        property: &str,
    ) -> Result<bool> {
        let default_intrinsic = {
            let function = self.function(id)?;
            function_default_intrinsic_descriptor(function, &self.atoms, property)?
        };
        let lookup = self.property_lookup(property);
        let function = self.function_mut(id)?;
        Ok(function.properties.delete(lookup, default_intrinsic))
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

    pub(crate) fn get_native_function_property(
        &self,
        id: NativeFunctionId,
        property: &str,
    ) -> Result<Value> {
        let function = self.native_function(id)?;
        if let Some(default_descriptor) =
            native_function_default_intrinsic_descriptor(function, property)
            && let Some(value) = function
                .properties()
                .intrinsic_value(property, default_descriptor)
        {
            return self.checked_value(value);
        }

        let value = match property {
            FUNCTION_PROTOTYPE_PROPERTY => function.properties().prototype(),
            _ => function
                .intrinsic_property(property)
                .unwrap_or_else(|| function.properties().get(self.property_lookup(property))),
        };
        self.checked_value(value)
    }

    pub(crate) fn native_function_own_property_descriptor(
        &self,
        id: NativeFunctionId,
        property: &str,
    ) -> Result<Option<DataPropertyDescriptor>> {
        let function = self.native_function(id)?;
        if let Some(descriptor) = native_function_intrinsic_descriptor(function, property) {
            return Ok(Some(descriptor));
        }
        if let Some(value) = function.intrinsic_property(property) {
            return Ok(Some(DataPropertyDescriptor::new(
                value,
                PropertyWritable::No,
                PropertyEnumerable::No,
                PropertyConfigurable::No,
            )));
        }
        Ok(function
            .properties()
            .own_property_descriptor(self.property_lookup(property)))
    }

    pub(crate) fn has_native_function_property(
        &self,
        id: NativeFunctionId,
        property: &str,
    ) -> Result<bool> {
        let function = self.native_function(id)?;
        if matches!(property, FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY)
            && function.properties().has_intrinsic(property)
        {
            return Ok(true);
        }
        Ok((property == FUNCTION_PROTOTYPE_PROPERTY)
            || function.has_intrinsic_property(property)
            || function.properties().has(self.property_lookup(property)))
    }

    pub(crate) fn set_native_function_property(
        &mut self,
        id: NativeFunctionId,
        property: &str,
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
        let key = self.intern_property_key(property)?;
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

    pub(crate) fn delete_native_function_property(
        &mut self,
        id: NativeFunctionId,
        property: &str,
    ) -> Result<bool> {
        let default_intrinsic =
            native_function_default_intrinsic_descriptor(self.native_function(id)?, property);
        if self.native_function(id)?.has_intrinsic_property(property) {
            return Ok(false);
        }
        let lookup = self.property_lookup(property);
        let function = self.native_function_mut(id)?;
        Ok(function.properties_mut().delete(lookup, default_intrinsic))
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

    fn function_param_atoms(&mut self, params: &[String]) -> Result<Rc<[AtomId]>> {
        let mut atoms = Vec::with_capacity(params.len());
        for param in params {
            atoms.push(self.intern_atom(param)?);
        }
        Ok(atoms.into())
    }

    fn function_name(&mut self, name: Option<&str>) -> Result<super::FunctionName> {
        let Some(name) = name.filter(|name| !name.is_empty()) else {
            return Ok(super::FunctionName::anonymous());
        };
        Ok(super::FunctionName::new(self.intern_atom(name)?))
    }

    fn function_scope(&self, params: &[AtomId], args: Vec<Value>) -> Result<BindingScope> {
        let mut scope = BindingScope::new();
        let mut args = args.into_iter();
        for atom in params {
            if !scope.contains(*atom) {
                self.ensure_extra_binding_capacity(scope.len())?;
            }
            let value = args.next().unwrap_or(Value::Undefined);
            self.checked_value(value.clone())?;
            scope.insert(*atom, BindingCell::new(value, true, DeclKind::Var));
        }
        Ok(scope)
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
