use std::{
    collections::{BTreeMap, btree_map::Entry},
    rc::Rc,
};

use crate::{
    ast::{DeclKind, Expr, Stmt},
    error::{Error, Result},
    runtime::Context,
    runtime_completion::Completion,
    runtime_object::{
        DataPropertyDescriptor, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
        PropertyWritable,
    },
    runtime_scope::{BindingCell, BindingScope},
    value::{FunctionId, NativeFunctionId, ObjectId, Value},
};

const FUNCTION_LENGTH_PROPERTY: &str = "length";
const FUNCTION_NAME_PROPERTY: &str = "name";
const FUNCTION_PROTOTYPE_PROPERTY: &str = "prototype";
const PROTOTYPE_CONSTRUCTOR_PROPERTY: &str = "constructor";

#[derive(Debug, Clone)]
pub(super) struct FunctionProperties {
    prototype: Value,
    properties: BTreeMap<String, FunctionProperty>,
    property_order: Vec<String>,
}

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
            let prototype_id = self.objects.create_with_prototype_property(
                None,
                PROTOTYPE_CONSTRUCTOR_PROPERTY.to_owned(),
                function.clone(),
                PropertyEnumerable::No,
                self.limits.max_objects,
                self.limits.max_object_properties,
            )?;
            Value::Object(prototype_id)
        } else {
            Value::Undefined
        };
        self.functions.push(super::Function {
            name: name.unwrap_or_default().to_owned(),
            params: Rc::clone(params),
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
        let (params, body, captures) = {
            let function = self.function(id)?;
            (
                Rc::clone(&function.params),
                Rc::clone(&function.body),
                function.captures.clone(),
            )
        };
        let args = self.eval_args(args)?;
        let caller_locals = std::mem::replace(&mut self.locals, captures);
        let scope = match self.function_scope(&params, args) {
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
        let value = match property {
            FUNCTION_LENGTH_PROPERTY => Value::Number(function.length()?),
            FUNCTION_NAME_PROPERTY => Value::String(function.name.clone()),
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
        let function = self.function(id)?;
        if let Some(descriptor) = function_intrinsic_descriptor(function, property)? {
            return Ok(Some(descriptor));
        }
        Ok(function.properties.own_property_descriptor(property))
    }

    pub(crate) fn has_function_property(&self, id: FunctionId, property: &str) -> Result<bool> {
        let function = self.function(id)?;
        Ok(
            matches!(property, FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY)
                || (property == FUNCTION_PROTOTYPE_PROPERTY && function.constructable)
                || function.properties.has(property),
        )
    }

    pub(crate) fn set_function_property(
        &mut self,
        id: FunctionId,
        property: String,
        value: Value,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let function = self.function_mut(id)?;
        function.properties.set(property, value, max_properties)
    }

    pub(crate) fn define_function_property(
        &mut self,
        id: FunctionId,
        property: String,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let function = self.function_mut(id)?;
        function
            .properties
            .define_property(property, update, max_properties)
    }

    pub(crate) fn delete_function_property(
        &mut self,
        id: FunctionId,
        property: &str,
    ) -> Result<bool> {
        let function = self.function_mut(id)?;
        Ok(function.properties.delete(property))
    }

    pub(crate) fn function_enumerable_keys(&self, id: FunctionId) -> Result<Vec<String>> {
        self.function(id).map(|function| function.properties.keys())
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
        let value = match property {
            FUNCTION_LENGTH_PROPERTY => Value::Number(function.length()),
            FUNCTION_NAME_PROPERTY => Value::String(function.name().to_owned()),
            FUNCTION_PROTOTYPE_PROPERTY => function.properties().prototype(),
            _ => function
                .intrinsic_property(property)
                .unwrap_or_else(|| function.properties().get(property)),
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
        Ok(function.properties().own_property_descriptor(property))
    }

    pub(crate) fn has_native_function_property(
        &self,
        id: NativeFunctionId,
        property: &str,
    ) -> Result<bool> {
        let function = self.native_function(id)?;
        Ok(matches!(
            property,
            FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY | FUNCTION_PROTOTYPE_PROPERTY
        ) || function.has_intrinsic_property(property)
            || function.properties().has(property))
    }

    pub(crate) fn set_native_function_property(
        &mut self,
        id: NativeFunctionId,
        property: String,
        value: Value,
    ) -> Result<()> {
        if property == FUNCTION_PROTOTYPE_PROPERTY {
            self.native_function(id)?;
            return Ok(());
        }
        if self.native_function(id)?.has_intrinsic_property(&property) {
            return Ok(());
        }
        let max_properties = self.limits.max_object_properties;
        let function = self.native_function_mut(id)?;
        function
            .properties_mut()
            .set(property, value, max_properties)
    }

    pub(crate) fn define_native_function_property(
        &mut self,
        id: NativeFunctionId,
        property: String,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        if self.native_function(id)?.has_intrinsic_property(&property) {
            return Ok(());
        }
        let max_properties = self.limits.max_object_properties;
        let function = self.native_function_mut(id)?;
        function
            .properties_mut()
            .define_property(property, update, max_properties)
    }

    pub(crate) fn delete_native_function_property(
        &mut self,
        id: NativeFunctionId,
        property: &str,
    ) -> Result<bool> {
        if self.native_function(id)?.has_intrinsic_property(property) {
            return Ok(false);
        }
        let function = self.native_function_mut(id)?;
        Ok(function.properties_mut().delete(property))
    }

    pub(crate) fn native_function_enumerable_keys(
        &self,
        id: NativeFunctionId,
    ) -> Result<Vec<String>> {
        self.native_function(id)
            .map(|function| function.properties().keys())
    }

    fn eval_args(&mut self, args: &[Expr]) -> Result<Vec<Value>> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval_expr(arg)?);
        }
        Ok(values)
    }

    fn function_scope(&mut self, params: &[String], args: Vec<Value>) -> Result<BindingScope> {
        let mut scope = BindingScope::new();
        let mut args = args.into_iter();
        for param in params {
            let atom = self.intern_atom(param)?;
            if !scope.contains(atom) {
                self.ensure_extra_binding_capacity(scope.len())?;
            }
            let value = args.next().unwrap_or(Value::Undefined);
            self.checked_value(value.clone())?;
            scope.insert(atom, BindingCell::new(value, true, DeclKind::Var));
        }
        Ok(scope)
    }
}

impl FunctionProperties {
    pub(super) const fn new(prototype: Value) -> Self {
        Self {
            prototype,
            properties: BTreeMap::new(),
            property_order: Vec::new(),
        }
    }

    pub(super) fn prototype(&self) -> Value {
        self.prototype.clone()
    }

    pub(super) fn get(&self, property: &str) -> Value {
        self.properties
            .get(property)
            .map_or(Value::Undefined, FunctionProperty::value)
    }

    pub(super) fn own_property_descriptor(&self, property: &str) -> Option<DataPropertyDescriptor> {
        self.properties
            .get(property)
            .map(FunctionProperty::descriptor)
    }

    pub(super) fn has(&self, property: &str) -> bool {
        self.properties.contains_key(property)
    }

    pub(super) fn set(
        &mut self,
        property: String,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        if matches!(
            property.as_str(),
            FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY
        ) {
            return Ok(());
        }
        if property == FUNCTION_PROTOTYPE_PROPERTY {
            self.prototype = value;
            return Ok(());
        }
        match self.properties.entry(property) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().set_value(value);
            }
            Entry::Vacant(entry) => {
                if self.property_order.len() >= max_properties {
                    return Err(Error::limit(format!(
                        "function property count exceeded {max_properties}"
                    )));
                }
                self.property_order.push(entry.key().clone());
                entry.insert(FunctionProperty::new(value, PropertyEnumerable::Yes));
            }
        }
        Ok(())
    }

    pub(super) fn delete(&mut self, property: &str) -> bool {
        if matches!(property, FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY) {
            return false;
        }
        if property == FUNCTION_PROTOTYPE_PROPERTY {
            return false;
        }
        let Some(existing_property) = self.properties.get(property) else {
            return true;
        };
        if !existing_property.is_configurable() {
            return false;
        }
        let Some(_) = self.properties.remove(property) else {
            return true;
        };
        self.property_order.retain(|key| key != property);
        true
    }

    pub(super) fn keys(&self) -> Vec<String> {
        self.property_order
            .iter()
            .filter_map(|key| {
                self.properties
                    .get(key)
                    .filter(|property| property.is_enumerable())
                    .map(|_| key.clone())
            })
            .collect()
    }

    pub(super) fn define_builtin(
        &mut self,
        property: String,
        value: Value,
        enumerable: PropertyEnumerable,
    ) {
        match self.properties.entry(property) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().set_value(value);
                entry.get_mut().set_enumerable(enumerable);
            }
            Entry::Vacant(entry) => {
                self.property_order.push(entry.key().clone());
                entry.insert(FunctionProperty::new(value, enumerable));
            }
        }
    }

    pub(super) fn define_property(
        &mut self,
        property: String,
        update: DataPropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        if matches!(
            property.as_str(),
            FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY
        ) {
            return Ok(());
        }
        if property == FUNCTION_PROTOTYPE_PROPERTY {
            if let Some(value) = update.value() {
                self.prototype = value;
            }
            return Ok(());
        }
        match self.properties.entry(property) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().define(&update);
            }
            Entry::Vacant(entry) => {
                if self.property_order.len() >= max_properties {
                    return Err(Error::limit(format!(
                        "function property count exceeded {max_properties}"
                    )));
                }
                self.property_order.push(entry.key().clone());
                entry.insert(FunctionProperty::from_update(update));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct FunctionProperty {
    descriptor: DataPropertyDescriptor,
}

impl FunctionProperty {
    const fn new(value: Value, enumerable: PropertyEnumerable) -> Self {
        Self {
            descriptor: DataPropertyDescriptor::new(
                value,
                PropertyWritable::Yes,
                enumerable,
                PropertyConfigurable::Yes,
            ),
        }
    }

    fn from_update(update: DataPropertyUpdate) -> Self {
        Self {
            descriptor: update.complete_for_new(),
        }
    }

    fn value(&self) -> Value {
        self.descriptor.value()
    }

    const fn is_configurable(&self) -> bool {
        self.descriptor.configurable().is_yes()
    }

    const fn is_enumerable(&self) -> bool {
        self.descriptor.enumerable().is_yes()
    }

    fn descriptor(&self) -> DataPropertyDescriptor {
        self.descriptor.clone()
    }

    fn set_value(&mut self, value: Value) {
        if self.descriptor.writable().is_yes() {
            self.descriptor = DataPropertyDescriptor::new(
                value,
                self.descriptor.writable(),
                self.descriptor.enumerable(),
                self.descriptor.configurable(),
            );
        }
    }

    fn define(&mut self, update: &DataPropertyUpdate) {
        let value = update.value().unwrap_or_else(|| self.descriptor.value());
        let writable = update
            .writable()
            .unwrap_or_else(|| self.descriptor.writable());
        let enumerable = update
            .enumerable()
            .unwrap_or_else(|| self.descriptor.enumerable());
        let configurable = update
            .configurable()
            .unwrap_or_else(|| self.descriptor.configurable());
        self.descriptor = DataPropertyDescriptor::new(value, writable, enumerable, configurable);
    }

    fn set_enumerable(&mut self, enumerable: PropertyEnumerable) {
        self.descriptor = DataPropertyDescriptor::new(
            self.descriptor.value(),
            self.descriptor.writable(),
            enumerable,
            self.descriptor.configurable(),
        );
    }
}

impl super::Function {
    fn length(&self) -> Result<f64> {
        let length = u32::try_from(self.params.len())
            .map_err(|_| Error::limit("function parameter count exceeded supported range"))?;
        Ok(f64::from(length))
    }
}

fn function_intrinsic_descriptor(
    function: &super::Function,
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
            Value::String(function.name.clone()),
            PropertyWritable::No,
            PropertyEnumerable::No,
            PropertyConfigurable::Yes,
        ),
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

fn native_function_intrinsic_descriptor(
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
