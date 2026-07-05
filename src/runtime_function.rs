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
        DataPropertyDescriptor, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
        PropertyEnumerable, PropertyWritable,
    },
    runtime_scope::{BindingCell, BindingScope},
    value::{FunctionId, NativeFunctionId, ObjectId, Value},
};

use super::runtime_function_intrinsic::{FunctionIntrinsicProperty, FunctionProperty};

const FUNCTION_LENGTH_PROPERTY: &str = "length";
const FUNCTION_NAME_PROPERTY: &str = "name";
const FUNCTION_PROTOTYPE_PROPERTY: &str = "prototype";
const PROTOTYPE_CONSTRUCTOR_PROPERTY: &str = "constructor";

#[derive(Debug, Clone)]
pub(super) struct FunctionProperties {
    prototype: Value,
    length: FunctionIntrinsicProperty,
    name: FunctionIntrinsicProperty,
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
        let value = function_default_intrinsic_descriptor(function, property)?.map_or_else(
            || match property {
                FUNCTION_PROTOTYPE_PROPERTY => function.properties.prototype(),
                _ => function.properties.get(property),
            },
            |default_descriptor| {
                function
                    .properties
                    .intrinsic_value_or_property(property, default_descriptor)
            },
        );
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
        if matches!(property, FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY)
            && function.properties.has_intrinsic(property)
        {
            return Ok(true);
        }
        Ok(
            (property == FUNCTION_PROTOTYPE_PROPERTY && function.constructable)
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
        let default_intrinsic = function_default_intrinsic_descriptor(function, &property)?;
        function
            .properties
            .set(property, value, max_properties, default_intrinsic)
    }

    pub(crate) fn define_function_property(
        &mut self,
        id: FunctionId,
        property: String,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        let max_properties = self.limits.max_object_properties;
        let function = self.function_mut(id)?;
        let default_intrinsic = function_default_intrinsic_descriptor(function, &property)?;
        function
            .properties
            .define_property(property, update, max_properties, default_intrinsic)
    }

    pub(crate) fn delete_function_property(
        &mut self,
        id: FunctionId,
        property: &str,
    ) -> Result<bool> {
        let function = self.function_mut(id)?;
        let default_intrinsic = function_default_intrinsic_descriptor(function, property)?;
        Ok(function.properties.delete(property, default_intrinsic))
    }

    pub(crate) fn function_enumerable_keys(&self, id: FunctionId) -> Result<Vec<String>> {
        let function = self.function(id)?;
        let length = function_default_intrinsic_descriptor(function, FUNCTION_LENGTH_PROPERTY)?;
        let name = function_default_intrinsic_descriptor(function, FUNCTION_NAME_PROPERTY)?;
        Ok(function.properties.keys(length, name))
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
        let value = native_function_default_intrinsic_descriptor(function, property).map_or_else(
            || match property {
                FUNCTION_PROTOTYPE_PROPERTY => function.properties().prototype(),
                _ => function
                    .intrinsic_property(property)
                    .unwrap_or_else(|| function.properties().get(property)),
            },
            |default_descriptor| {
                function
                    .properties()
                    .intrinsic_value_or_property(property, default_descriptor)
            },
        );
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
        if matches!(property, FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY)
            && function.properties().has_intrinsic(property)
        {
            return Ok(true);
        }
        Ok((property == FUNCTION_PROTOTYPE_PROPERTY)
            || function.has_intrinsic_property(property)
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
        let default_intrinsic =
            native_function_default_intrinsic_descriptor(self.native_function(id)?, &property);
        if self.native_function(id)?.has_intrinsic_property(&property) {
            return Ok(());
        }
        let max_properties = self.limits.max_object_properties;
        let function = self.native_function_mut(id)?;
        function
            .properties_mut()
            .set(property, value, max_properties, default_intrinsic)
    }

    pub(crate) fn define_native_function_property(
        &mut self,
        id: NativeFunctionId,
        property: String,
        update: DataPropertyUpdate,
    ) -> Result<()> {
        let default_intrinsic =
            native_function_default_intrinsic_descriptor(self.native_function(id)?, &property);
        if self.native_function(id)?.has_intrinsic_property(&property) {
            return Ok(());
        }
        let max_properties = self.limits.max_object_properties;
        let function = self.native_function_mut(id)?;
        function.properties_mut().define_property(
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
        Ok(function.properties().keys(length, name))
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
            length: FunctionIntrinsicProperty::new(),
            name: FunctionIntrinsicProperty::new(),
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

    pub(super) fn intrinsic_descriptor(
        &self,
        property: &str,
        default: DataPropertyDescriptor,
    ) -> Option<DataPropertyDescriptor> {
        self.intrinsic(property)
            .and_then(|intrinsic| intrinsic.descriptor(default))
    }

    pub(super) fn intrinsic_value_or_property(
        &self,
        property: &str,
        default: DataPropertyDescriptor,
    ) -> Value {
        self.intrinsic(property)
            .and_then(|intrinsic| intrinsic.value(default))
            .unwrap_or_else(|| self.get(property))
    }

    pub(super) fn has(&self, property: &str) -> bool {
        self.properties.contains_key(property)
    }

    pub(super) fn has_intrinsic(&self, property: &str) -> bool {
        self.intrinsic(property)
            .is_some_and(FunctionIntrinsicProperty::has)
    }

    pub(super) fn set(
        &mut self,
        property: String,
        value: Value,
        max_properties: usize,
        default_intrinsic: Option<DataPropertyDescriptor>,
    ) -> Result<()> {
        if let Some(default) = default_intrinsic
            && self.set_intrinsic_value(&property, default, value.clone())
        {
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

    pub(super) fn delete(
        &mut self,
        property: &str,
        default_intrinsic: Option<DataPropertyDescriptor>,
    ) -> bool {
        if let Some(default) = default_intrinsic
            && let Some(deleted) = self.delete_intrinsic(property, default)
        {
            return deleted;
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

    pub(super) fn keys(
        &self,
        length: Option<DataPropertyDescriptor>,
        name: Option<DataPropertyDescriptor>,
    ) -> Vec<String> {
        let mut keys = Vec::new();
        self.push_intrinsic_key(&mut keys, FUNCTION_LENGTH_PROPERTY, length);
        self.push_intrinsic_key(&mut keys, FUNCTION_NAME_PROPERTY, name);
        keys.extend(self.property_order.iter().filter_map(|key| {
            self.properties
                .get(key)
                .filter(|property| property.is_enumerable())
                .map(|_| key.clone())
        }));
        keys
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
        default_intrinsic: Option<DataPropertyDescriptor>,
    ) -> Result<()> {
        if let Some(default) = default_intrinsic
            && self.define_intrinsic(&property, default, &update)
        {
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

    fn intrinsic(&self, property: &str) -> Option<&FunctionIntrinsicProperty> {
        match property {
            FUNCTION_LENGTH_PROPERTY => Some(&self.length),
            FUNCTION_NAME_PROPERTY => Some(&self.name),
            _ => None,
        }
    }

    fn intrinsic_mut(&mut self, property: &str) -> Option<&mut FunctionIntrinsicProperty> {
        match property {
            FUNCTION_LENGTH_PROPERTY => Some(&mut self.length),
            FUNCTION_NAME_PROPERTY => Some(&mut self.name),
            _ => None,
        }
    }

    fn set_intrinsic_value(
        &mut self,
        property: &str,
        default: DataPropertyDescriptor,
        value: Value,
    ) -> bool {
        let Some(intrinsic) = self.intrinsic_mut(property) else {
            return false;
        };
        intrinsic.set_value(default, value)
    }

    fn define_intrinsic(
        &mut self,
        property: &str,
        default: DataPropertyDescriptor,
        update: &DataPropertyUpdate,
    ) -> bool {
        let Some(intrinsic) = self.intrinsic_mut(property) else {
            return false;
        };
        intrinsic.define(default, update)
    }

    fn delete_intrinsic(
        &mut self,
        property: &str,
        default: DataPropertyDescriptor,
    ) -> Option<bool> {
        self.intrinsic_mut(property)
            .and_then(|intrinsic| intrinsic.delete(default))
    }

    fn push_intrinsic_key(
        &self,
        keys: &mut Vec<String>,
        property: &str,
        descriptor: Option<DataPropertyDescriptor>,
    ) {
        let Some(descriptor) =
            descriptor.and_then(|default| self.intrinsic_descriptor(property, default))
        else {
            return;
        };
        if descriptor.enumerable().is_yes() {
            keys.push(property.to_owned());
        }
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
    if let Some(default) = function_default_intrinsic_descriptor(function, property)? {
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
