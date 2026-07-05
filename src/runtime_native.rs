use crate::{
    ast::{DeclKind, Expr},
    error::{Error, Result},
    runtime::Context,
    runtime_scope::BindingCell,
    value::{NativeFunctionId, Value},
};

use super::runtime_function::FunctionProperties;

const OBJECT_CONSTRUCTOR_PROPERTY: &str = "constructor";
const OBJECT_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_NAME: &str = "Object";

#[derive(Debug, Clone)]
pub(super) struct NativeFunction {
    kind: NativeFunctionKind,
    properties: FunctionProperties,
}

impl NativeFunction {
    const fn new(kind: NativeFunctionKind, prototype: Value) -> Self {
        Self {
            kind,
            properties: FunctionProperties::new(prototype),
        }
    }

    pub(super) const fn kind(&self) -> NativeFunctionKind {
        self.kind
    }

    pub(super) const fn length(&self) -> f64 {
        match self.kind {
            NativeFunctionKind::Object => OBJECT_FUNCTION_LENGTH,
        }
    }

    pub(super) const fn name(&self) -> &'static str {
        match self.kind {
            NativeFunctionKind::Object => OBJECT_NAME,
        }
    }

    pub(super) const fn properties(&self) -> &FunctionProperties {
        &self.properties
    }

    pub(super) const fn properties_mut(&mut self) -> &mut FunctionProperties {
        &mut self.properties
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum NativeFunctionKind {
    Object,
}

impl Context {
    pub(crate) fn builtin_value(&mut self, name: &str) -> Result<Option<Value>> {
        if name != OBJECT_NAME {
            return Ok(None);
        }
        self.object_constructor_value().map(Some)
    }

    pub(crate) fn constructor_binding(&mut self, name: &str) -> Result<Option<Value>> {
        if let Some(binding) = self.get_binding(name) {
            return Ok(Some(binding.value()));
        }
        self.builtin_value(name)
    }

    pub(crate) fn eval_native_function(
        &mut self,
        id: NativeFunctionId,
        args: &[Expr],
        _this_value: Value,
    ) -> Result<Value> {
        match self.native_function(id)?.kind() {
            NativeFunctionKind::Object => self.eval_object_constructor(args),
        }
    }

    pub(crate) fn construct_native_function(
        &mut self,
        id: NativeFunctionId,
        args: &[Expr],
    ) -> Result<Value> {
        match self.native_function(id)?.kind() {
            NativeFunctionKind::Object => self.eval_object_constructor(args),
        }
    }

    pub(super) fn native_function(&self, id: NativeFunctionId) -> Result<&NativeFunction> {
        self.native_functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("native function id is not defined"))
    }

    pub(super) fn native_function_mut(
        &mut self,
        id: NativeFunctionId,
    ) -> Result<&mut NativeFunction> {
        self.native_functions
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("native function id is not defined"))
    }

    fn object_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Object) {
            return Ok(Value::NativeFunction(id));
        }

        self.ensure_extra_binding_capacity(1)?;
        let id = NativeFunctionId::new(self.native_functions.len());
        let constructor = Value::NativeFunction(id);
        let prototype = self.object_prototype_id_with_constructor(constructor.clone())?;
        self.native_functions
            .push(NativeFunction::new(NativeFunctionKind::Object, prototype));
        self.globals.insert(
            OBJECT_NAME.to_owned(),
            BindingCell::new(constructor.clone(), false, DeclKind::Const),
        );
        Ok(constructor)
    }

    fn object_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<Value> {
        let prototype = self
            .objects
            .object_prototype_id(self.limits.max_objects, self.limits.max_object_properties)?;
        self.objects.define_non_enumerable(
            prototype,
            OBJECT_CONSTRUCTOR_PROPERTY.to_owned(),
            constructor,
            self.limits.max_object_properties,
        )?;
        Ok(Value::Object(prototype))
    }

    fn native_function_id(&self, kind: NativeFunctionKind) -> Option<NativeFunctionId> {
        self.native_functions
            .iter()
            .enumerate()
            .find_map(|(index, function)| {
                if function.kind() == kind {
                    return Some(NativeFunctionId::new(index));
                }
                None
            })
    }

    fn eval_object_constructor(&mut self, args: &[Expr]) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let Some(value) = values.first() else {
            return self.create_object_from_constructor();
        };

        match value {
            Value::Object(_) | Value::Function(_) | Value::NativeFunction(_) | Value::Error(_) => {
                Ok(value.clone())
            }
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_) => self.create_object_from_constructor(),
        }
    }

    fn create_object_from_constructor(&mut self) -> Result<Value> {
        self.objects.create_with_prototype(
            None,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}
