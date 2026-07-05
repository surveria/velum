use crate::{
    ast::{DeclKind, Expr},
    error::{Error, Result},
    runtime::Context,
    runtime_scope::BindingCell,
    value::{NativeFunctionId, ObjectId, Value},
};

use super::runtime_function::FunctionProperties;

const OBJECT_CONSTRUCTOR_PROPERTY: &str = "constructor";
const ARRAY_INDEX_OF_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_INDEX_OF_NAME: &str = "indexOf";
const ARRAY_JOIN_DEFAULT_SEPARATOR: &str = ",";
const ARRAY_JOIN_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_JOIN_NAME: &str = "join";
const ARRAY_POP_FUNCTION_LENGTH: f64 = 0.0;
const ARRAY_POP_NAME: &str = "pop";
const ARRAY_PROTOTYPE_INDEX_OF_PROPERTY: &str = "indexOf";
const ARRAY_PROTOTYPE_JOIN_PROPERTY: &str = "join";
const ARRAY_PROTOTYPE_POP_PROPERTY: &str = "pop";
const ARRAY_PROTOTYPE_PUSH_PROPERTY: &str = "push";
const ARRAY_PROTOTYPE_SHIFT_PROPERTY: &str = "shift";
const ARRAY_PROTOTYPE_SLICE_PROPERTY: &str = "slice";
const ARRAY_PROTOTYPE_UNSHIFT_PROPERTY: &str = "unshift";
const ARRAY_PUSH_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_PUSH_NAME: &str = "push";
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
            NativeFunctionKind::ArrayIndexOf => ARRAY_INDEX_OF_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayJoin => ARRAY_JOIN_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayPop => ARRAY_POP_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayPush => ARRAY_PUSH_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayShift => ARRAY_SHIFT_FUNCTION_LENGTH,
            NativeFunctionKind::ArraySlice => ARRAY_SLICE_FUNCTION_LENGTH,
            NativeFunctionKind::ArrayUnshift => ARRAY_UNSHIFT_FUNCTION_LENGTH,
            NativeFunctionKind::Object => OBJECT_FUNCTION_LENGTH,
        }
    }

    pub(super) const fn name(&self) -> &'static str {
        match self.kind {
            NativeFunctionKind::Array => ARRAY_NAME,
            NativeFunctionKind::ArrayIndexOf => ARRAY_INDEX_OF_NAME,
            NativeFunctionKind::ArrayJoin => ARRAY_JOIN_NAME,
            NativeFunctionKind::ArrayPop => ARRAY_POP_NAME,
            NativeFunctionKind::ArrayPush => ARRAY_PUSH_NAME,
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
    ArrayIndexOf,
    ArrayJoin,
    ArrayPop,
    ArrayPush,
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
            NativeFunctionKind::ArrayIndexOf => self.eval_array_index_of(args, this_value),
            NativeFunctionKind::ArrayJoin => self.eval_array_join(args, this_value),
            NativeFunctionKind::ArrayPop => self.eval_array_pop(args, this_value),
            NativeFunctionKind::ArrayPush => self.eval_array_push(args, this_value),
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
            NativeFunctionKind::ArrayIndexOf
            | NativeFunctionKind::ArrayJoin
            | NativeFunctionKind::ArrayPop
            | NativeFunctionKind::ArrayPush
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

    fn array_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Array) {
            return Ok(Value::NativeFunction(id));
        }

        self.object_constructor_value()?;
        let id = NativeFunctionId::new(self.native_functions.len());
        let constructor = Value::NativeFunction(id);
        let prototype_id = self.array_prototype_id_with_constructor(constructor.clone())?;
        let prototype = Value::Object(prototype_id);
        self.native_functions
            .push(NativeFunction::new(NativeFunctionKind::Array, prototype));
        self.install_array_prototype_methods(prototype_id)?;
        self.insert_global_builtin(ARRAY_NAME, constructor.clone())?;
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

    fn array_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        self.objects.array_prototype_id_with_constructor(
            constructor,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn install_array_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        let index_of =
            self.create_native_function(NativeFunctionKind::ArrayIndexOf, Value::Undefined);
        self.objects.define_non_enumerable(
            prototype,
            ARRAY_PROTOTYPE_INDEX_OF_PROPERTY.to_owned(),
            index_of,
            self.limits.max_object_properties,
        )?;

        let join = self.create_native_function(NativeFunctionKind::ArrayJoin, Value::Undefined);
        self.objects.define_non_enumerable(
            prototype,
            ARRAY_PROTOTYPE_JOIN_PROPERTY.to_owned(),
            join,
            self.limits.max_object_properties,
        )?;

        let push = self.create_native_function(NativeFunctionKind::ArrayPush, Value::Undefined);
        self.objects.define_non_enumerable(
            prototype,
            ARRAY_PROTOTYPE_PUSH_PROPERTY.to_owned(),
            push,
            self.limits.max_object_properties,
        )?;

        let pop = self.create_native_function(NativeFunctionKind::ArrayPop, Value::Undefined);
        self.objects.define_non_enumerable(
            prototype,
            ARRAY_PROTOTYPE_POP_PROPERTY.to_owned(),
            pop,
            self.limits.max_object_properties,
        )?;

        let shift = self.create_native_function(NativeFunctionKind::ArrayShift, Value::Undefined);
        self.objects.define_non_enumerable(
            prototype,
            ARRAY_PROTOTYPE_SHIFT_PROPERTY.to_owned(),
            shift,
            self.limits.max_object_properties,
        )?;

        let slice = self.create_native_function(NativeFunctionKind::ArraySlice, Value::Undefined);
        self.objects.define_non_enumerable(
            prototype,
            ARRAY_PROTOTYPE_SLICE_PROPERTY.to_owned(),
            slice,
            self.limits.max_object_properties,
        )?;

        let unshift =
            self.create_native_function(NativeFunctionKind::ArrayUnshift, Value::Undefined);
        self.objects.define_non_enumerable(
            prototype,
            ARRAY_PROTOTYPE_UNSHIFT_PROPERTY.to_owned(),
            unshift,
            self.limits.max_object_properties,
        )
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

    fn eval_array_constructor(&mut self, args: &[Expr]) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        if let Some(length) = Self::array_constructor_length(&values)? {
            let prototype = self.array_constructor_prototype()?;
            return self.objects.create_array_with_length(
                length,
                prototype,
                self.limits.max_objects,
            );
        }
        self.create_array_from_elements(values)
    }

    fn eval_array_push(&mut self, args: &[Expr], this_value: &Value) -> Result<Value> {
        let Value::Object(id) = this_value else {
            return Err(Error::runtime(
                "Array.prototype.push requires an array receiver",
            ));
        };
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        self.objects
            .array_push(*id, values, self.limits.max_object_properties)
    }

    fn eval_array_pop(&mut self, args: &[Expr], this_value: &Value) -> Result<Value> {
        args.iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let Value::Object(id) = this_value else {
            return Err(Error::runtime(
                "Array.prototype.pop requires an array receiver",
            ));
        };
        self.objects.array_pop(*id)
    }

    fn eval_array_index_of(&mut self, args: &[Expr], this_value: &Value) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let Value::Object(id) = this_value else {
            return Err(Error::runtime(
                "Array.prototype.indexOf requires an array receiver",
            ));
        };

        let length = self.objects.array_len_for_index_of(*id)?;
        let from_index = Self::array_slice_bound(values.get(1), length, 0)?;
        let search = values
            .first()
            .map_or(Value::Undefined, std::clone::Clone::clone);
        self.objects.array_index_of(*id, &search, from_index)
    }

    fn eval_array_join(&mut self, args: &[Expr], this_value: &Value) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let separator = Self::array_join_separator(values.first());
        let Value::Object(id) = this_value else {
            return Err(Error::runtime(
                "Array.prototype.join requires an array receiver",
            ));
        };

        let length = self.objects.array_len(*id)?;
        let mut joined = String::new();
        for index in 0..length {
            if index > 0 {
                self.push_join_text(&mut joined, &separator)?;
            }
            let value = self.objects.array_get_index(*id, index)?;
            let text = Self::array_join_element_text(&value);
            self.push_join_text(&mut joined, &text)?;
        }
        Ok(Value::String(joined))
    }

    fn eval_array_shift(&mut self, args: &[Expr], this_value: &Value) -> Result<Value> {
        args.iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let Value::Object(id) = this_value else {
            return Err(Error::runtime(
                "Array.prototype.shift requires an array receiver",
            ));
        };
        self.objects
            .array_shift(*id, self.limits.max_object_properties)
    }

    fn eval_array_slice(&mut self, args: &[Expr], this_value: &Value) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let Value::Object(id) = this_value else {
            return Err(Error::runtime(
                "Array.prototype.slice requires an array receiver",
            ));
        };

        let length = self.objects.array_len_for_slice(*id)?;
        let start = Self::array_slice_bound(values.first(), length, 0)?;
        let end = Self::array_slice_bound(values.get(1), length, length)?.max(start);
        let prototype = self.array_constructor_prototype()?;
        self.objects.array_slice(
            *id,
            start,
            end,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn eval_array_unshift(&mut self, args: &[Expr], this_value: &Value) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let Value::Object(id) = this_value else {
            return Err(Error::runtime(
                "Array.prototype.unshift requires an array receiver",
            ));
        };
        self.objects
            .array_unshift(*id, values, self.limits.max_object_properties)
    }

    fn array_join_separator(value: Option<&Value>) -> String {
        match value {
            None | Some(Value::Undefined) => ARRAY_JOIN_DEFAULT_SEPARATOR.to_owned(),
            Some(value) => value.display_for_concat(),
        }
    }

    fn array_join_element_text(value: &Value) -> String {
        match value {
            Value::Undefined | Value::Null => String::new(),
            _ => value.display_for_concat(),
        }
    }

    fn push_join_text(&self, joined: &mut String, text: &str) -> Result<()> {
        let length = joined
            .len()
            .checked_add(text.len())
            .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
        if length > self.limits.max_string_len {
            return Err(Error::limit(format!(
                "string length {} exceeded {}",
                length, self.limits.max_string_len
            )));
        }
        joined.push_str(text);
        Ok(())
    }

    fn array_slice_bound(value: Option<&Value>, length: usize, default: usize) -> Result<usize> {
        let Some(value) = value else {
            return Ok(default);
        };
        if matches!(value, Value::Undefined) {
            return Ok(default);
        }

        let number = Self::array_slice_bound_number(value);
        Self::array_slice_bound_from_number(number, length)
    }

    fn array_slice_bound_number(value: &Value) -> f64 {
        match value {
            Value::Undefined
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::Object(_)
            | Value::Error(_)
            | Value::Null => 0.0,
            Value::Bool(value) => {
                if *value {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Number(value) => *value,
            Value::String(value) => value.trim().parse::<f64>().unwrap_or(0.0),
        }
    }

    fn array_slice_bound_from_number(number: f64, length: usize) -> Result<usize> {
        if number.is_nan() || number == 0.0 {
            return Ok(0);
        }
        if !number.is_finite() {
            return if number.is_sign_negative() {
                Ok(0)
            } else {
                Ok(length)
            };
        }

        let length_f64 = Self::array_slice_length_as_f64(length)?;
        let integer = if number.is_sign_negative() {
            number.ceil()
        } else {
            number.floor()
        };
        let clamped = if integer < 0.0 {
            (length_f64 + integer).clamp(0.0, length_f64)
        } else {
            integer.min(length_f64)
        };
        Self::array_slice_nonnegative_usize(clamped)
    }

    fn array_slice_length_as_f64(length: usize) -> Result<f64> {
        let length = u32::try_from(length)
            .map_err(|_| Error::limit("array length exceeded supported range"))?;
        Ok(f64::from(length))
    }

    fn array_slice_nonnegative_usize(value: f64) -> Result<usize> {
        if value == 0.0 {
            return Ok(0);
        }
        format!("{value:.0}")
            .parse::<usize>()
            .map_err(|_| Error::limit("array index exceeded supported range"))
    }

    pub(crate) fn create_array_from_elements(&mut self, elements: Vec<Value>) -> Result<Value> {
        let prototype = self.array_constructor_prototype()?;
        self.objects.create_array(
            elements,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn create_object_from_constructor(&mut self) -> Result<Value> {
        self.objects.create_with_prototype(
            None,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn array_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.array_constructor_value()? else {
            return Err(Error::runtime("Array constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("Array prototype is not an object")),
        }
    }

    fn array_constructor_length(values: &[Value]) -> Result<Option<usize>> {
        let Some(Value::Number(value)) = values.first() else {
            return Ok(None);
        };
        if values.len() != 1 {
            return Ok(None);
        }
        Self::array_length_from_number(*value).map(Some)
    }

    fn array_length_from_number(value: f64) -> Result<usize> {
        if value == 0.0 {
            return Ok(0);
        }
        if !value.is_finite() || value.is_sign_negative() || value.fract() != 0.0 {
            return Err(Error::runtime("invalid array length"));
        }
        format!("{value:.0}")
            .parse::<usize>()
            .map_err(|_| Error::limit("array length exceeded supported range"))
    }
}
