use std::collections::{BTreeMap, btree_map::Entry};

use crate::{
    ast::{DeclKind, Expr, Stmt},
    error::{Error, Result},
    runtime::Context,
    runtime_completion::Completion,
    runtime_scope::{BindingCell, BindingScope},
    value::{FunctionId, ObjectId, Value},
};

const FUNCTION_LENGTH_PROPERTY: &str = "length";
const FUNCTION_NAME_PROPERTY: &str = "name";
const FUNCTION_PROTOTYPE_PROPERTY: &str = "prototype";

#[derive(Debug, Clone)]
pub(super) struct FunctionProperties {
    prototype: Value,
    properties: BTreeMap<String, Value>,
    property_order: Vec<String>,
}

impl Context {
    pub(crate) fn create_function(
        &mut self,
        name: Option<&str>,
        params: &[String],
        body: &[Stmt],
    ) -> Result<Value> {
        let id = FunctionId::new(self.functions.len());
        let function = Value::Function(id);
        let prototype = self
            .objects
            .create_with_prototype(None, self.limits.max_objects)?;
        self.functions.push(super::Function {
            name: name.unwrap_or_default().to_owned(),
            params: params.to_vec(),
            body: body.to_vec(),
            captures: self.locals.clone(),
            properties: FunctionProperties::new(prototype),
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
        let function = self.function(id)?.clone();
        let args = self.eval_args(args)?;
        let caller_locals = std::mem::replace(&mut self.locals, function.captures);
        let scope = match self.function_scope(&function.params, args) {
            Ok(scope) => scope,
            Err(error) => {
                self.locals = caller_locals;
                return Err(error);
            }
        };
        self.locals.push(scope);
        self.this_values.push(this_value);
        let result = self
            .hoist_var_declarations(&function.body)
            .and_then(|()| self.eval_block(&function.body));
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

    pub(crate) fn has_function_property(&self, id: FunctionId, property: &str) -> Result<bool> {
        let function = self.function(id)?;
        Ok(matches!(
            property,
            FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY | FUNCTION_PROTOTYPE_PROPERTY
        ) || function.properties.has(property))
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
        match function.properties.prototype() {
            Value::Object(id) => Ok(Some(id)),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Function(_)
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

    fn eval_args(&mut self, args: &[Expr]) -> Result<Vec<Value>> {
        args.iter().map(|arg| self.eval_expr(arg)).collect()
    }

    fn function_scope(&self, params: &[String], args: Vec<Value>) -> Result<BindingScope> {
        let mut scope = BindingScope::new();
        let mut args = args.into_iter();
        for param in params {
            if !scope.contains(param) {
                self.ensure_extra_binding_capacity(scope.len())?;
            }
            let value = args.next().unwrap_or(Value::Undefined);
            self.checked_value(value.clone())?;
            scope.insert(param.clone(), BindingCell::new(value, true, DeclKind::Var));
        }
        Ok(scope)
    }
}

impl FunctionProperties {
    const fn new(prototype: Value) -> Self {
        Self {
            prototype,
            properties: BTreeMap::new(),
            property_order: Vec::new(),
        }
    }

    fn prototype(&self) -> Value {
        self.prototype.clone()
    }

    fn get(&self, property: &str) -> Value {
        self.properties
            .get(property)
            .cloned()
            .unwrap_or(Value::Undefined)
    }

    fn has(&self, property: &str) -> bool {
        self.properties.contains_key(property)
    }

    fn set(&mut self, property: String, value: Value, max_properties: usize) -> Result<()> {
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
                entry.insert(value);
            }
            Entry::Vacant(entry) => {
                if self.property_order.len() >= max_properties {
                    return Err(Error::limit(format!(
                        "function property count exceeded {max_properties}"
                    )));
                }
                self.property_order.push(entry.key().clone());
                entry.insert(value);
            }
        }
        Ok(())
    }

    fn delete(&mut self, property: &str) -> bool {
        if matches!(property, FUNCTION_LENGTH_PROPERTY | FUNCTION_NAME_PROPERTY) {
            return true;
        }
        if property == FUNCTION_PROTOTYPE_PROPERTY {
            return false;
        }
        let removed_property = self.properties.remove(property);
        if removed_property.is_some() {
            self.property_order.retain(|key| key != property);
        }
        true
    }

    fn keys(&self) -> Vec<String> {
        self.property_order.clone()
    }
}

impl super::Function {
    fn length(&self) -> Result<f64> {
        let length = u32::try_from(self.params.len())
            .map_err(|_| Error::limit("function parameter count exceeded supported range"))?;
        Ok(f64::from(length))
    }
}
