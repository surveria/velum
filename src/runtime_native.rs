use crate::{
    ast::{DeclKind, Expr},
    error::{Error, Result},
    runtime::Context,
    runtime_scope::BindingCell,
    value::{NativeFunctionId, Value},
};

use super::runtime_function::FunctionProperties;

#[path = "runtime_native_array.rs"]
mod runtime_native_array;

const OBJECT_CONSTRUCTOR_PROPERTY: &str = "constructor";
const ARRAY_CONCAT_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_CONCAT_NAME: &str = "concat";
const ARRAY_INCLUDES_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_INCLUDES_NAME: &str = "includes";
const ARRAY_INDEX_OF_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_INDEX_OF_NAME: &str = "indexOf";
const ARRAY_JOIN_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_JOIN_NAME: &str = "join";
const ARRAY_LAST_INDEX_OF_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_LAST_INDEX_OF_NAME: &str = "lastIndexOf";
const ARRAY_POP_FUNCTION_LENGTH: f64 = 0.0;
const ARRAY_POP_NAME: &str = "pop";
const ARRAY_PUSH_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_PUSH_NAME: &str = "push";
const ARRAY_REVERSE_FUNCTION_LENGTH: f64 = 0.0;
const ARRAY_REVERSE_NAME: &str = "reverse";
const ARRAY_SHIFT_FUNCTION_LENGTH: f64 = 0.0;
const ARRAY_SHIFT_NAME: &str = "shift";
const ARRAY_SLICE_FUNCTION_LENGTH: f64 = 2.0;
const ARRAY_SLICE_NAME: &str = "slice";
const ARRAY_UNSHIFT_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_UNSHIFT_NAME: &str = "unshift";
const ARRAY_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_NAME: &str = "Array";
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
            NativeFunctionKind::Array => ARRAY_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayConcat => ARRAY_CONCAT_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayIncludes => ARRAY_INCLUDES_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayIndexOf => ARRAY_INDEX_OF_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayJoin => ARRAY_JOIN_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayLastIndexOf => ARRAY_LAST_INDEX_OF_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayPop => ARRAY_POP_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayPush => ARRAY_PUSH_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayReverse => ARRAY_REVERSE_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayShift => ARRAY_SHIFT_FUNCTION_LENGTH,
            NativeFunctionKind::ArraySlice => ARRAY_SLICE_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayUnshift => ARRAY_UNSHIFT_FUNCTION_LENGTH,
            NativeFunctionKind::Object => OBJECT_FUNCTION_LENGTH,
        }
    }

    pub(super) const fn name(&self) -> &'static str {
        match self.kind {
            NativeFunctionKind::Array => ARRAY_NAME,
            NativeFunctionKind::ArrayConcat => ARRAY_CONCAT_NAME,
            NativeFunctionKind::ArrayIncludes => ARRAY_INCLUDES_NAME,
            NativeFunctionKind::ArrayIndexOf => ARRAY_INDEX_OF_NAME,
            NativeFunctionKind::ArrayJoin => ARRAY_JOIN_NAME,
            NativeFunctionKind::ArrayLastIndexOf => ARRAY_LAST_INDEX_OF_NAME,
            NativeFunctionKind::ArrayPop => ARRAY_POP_NAME,
            NativeFunctionKind::ArrayPush => ARRAY_PUSH_NAME,
            NativeFunctionKind::ArrayReverse => ARRAY_REVERSE_NAME,
            NativeFunctionKind::ArrayShift => ARRAY_SHIFT_NAME,
            NativeFunctionKind::ArraySlice => ARRAY_SLICE_NAME,
            NativeFunctionKind::ArrayUnshift => ARRAY_UNSHIFT_NAME,
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
    Array,
    ArrayConcat,
    ArrayIncludes,
    ArrayIndexOf,
    ArrayJoin,
    ArrayLastIndexOf,
    ArrayPop,
    ArrayPush,
    ArrayReverse,
    ArrayShift,
    ArraySlice,
    ArrayUnshift,
    Object,
}

impl Context {
    pub(crate) fn builtin_value(&mut self, name: &str) -> Result<Option<Value>> {
        match name {
            ARRAY_NAME => self.array_constructor_value().map(Some),
            OBJECT_NAME => self.object_constructor_value().map(Some),
            _ => Ok(None),
        }
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
        this_value: &Value,
    ) -> Result<Value> {
        match self.native_function(id)?.kind() {
            NativeFunctionKind::Array => self.eval_array_constructor(args),
            NativeFunctionKind::ArrayConcat => self.eval_array_concat(args, this_value),
            NativeFunctionKind::ArrayIncludes => self.eval_array_includes(args, this_value),
            NativeFunctionKind::ArrayIndexOf => self.eval_array_index_of(args, this_value),
            NativeFunctionKind::ArrayJoin => self.eval_array_join(args, this_value),
            NativeFunctionKind::ArrayLastIndexOf => self.eval_array_last_index_of(args, this_value),
            NativeFunctionKind::ArrayPop => self.eval_array_pop(args, this_value),
            NativeFunctionKind::ArrayPush => self.eval_array_push(args, this_value),
            NativeFunctionKind::ArrayReverse => self.eval_array_reverse(args, this_value),
            NativeFunctionKind::ArrayShift => self.eval_array_shift(args, this_value),
            NativeFunctionKind::ArraySlice => self.eval_array_slice(args, this_value),
            NativeFunctionKind::ArrayUnshift => self.eval_array_unshift(args, this_value),
            NativeFunctionKind::Object => self.eval_object_constructor(args),
        }
    }

    pub(crate) fn construct_native_function(
        &mut self,
        id: NativeFunctionId,
        args: &[Expr],
    ) -> Result<Value> {
        match self.native_function(id)?.kind() {
            NativeFunctionKind::Array => self.eval_array_constructor(args),
            NativeFunctionKind::ArrayConcat
            | NativeFunctionKind::ArrayIncludes
            | NativeFunctionKind::ArrayIndexOf
            | NativeFunctionKind::ArrayJoin
            | NativeFunctionKind::ArrayLastIndexOf
            | NativeFunctionKind::ArrayPop
            | NativeFunctionKind::ArrayPush
            | NativeFunctionKind::ArrayReverse
            | NativeFunctionKind::ArrayShift
            | NativeFunctionKind::ArraySlice
            | NativeFunctionKind::ArrayUnshift => {
                Err(Error::runtime("native method is not a constructor"))
            }
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

        let id = NativeFunctionId::new(self.native_functions.len());
        let constructor = Value::NativeFunction(id);
        let prototype = self.object_prototype_id_with_constructor(constructor.clone())?;
        self.native_functions
            .push(NativeFunction::new(NativeFunctionKind::Object, prototype));
        self.insert_global_builtin(OBJECT_NAME, constructor.clone())?;
        Ok(constructor)
    }

    fn insert_global_builtin(&mut self, name: &str, constructor: Value) -> Result<()> {
        if self.globals.contains(name) {
            return Ok(());
        }
        self.ensure_extra_binding_capacity(1)?;
        self.globals.insert(
            name.to_owned(),
            BindingCell::new(constructor, false, DeclKind::Const),
        );
        Ok(())
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

    fn create_native_function(&mut self, kind: NativeFunctionKind, prototype: Value) -> Value {
        let id = NativeFunctionId::new(self.native_functions.len());
        self.native_functions
            .push(NativeFunction::new(kind, prototype));
        Value::NativeFunction(id)
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
