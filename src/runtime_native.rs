use crate::{
    ast::{DeclKind, Expr},
    error::{Error, Result},
    runtime::Context,
    runtime_object::PropertyEnumerable,
    runtime_scope::BindingCell,
    value::{ErrorName, ErrorObject, NativeFunctionId, Value},
};

use super::runtime_function::FunctionProperties;

#[path = "runtime_native_array.rs"]
mod runtime_native_array;
#[path = "runtime_native_boolean.rs"]
mod runtime_native_boolean;
#[path = "runtime_native_math.rs"]
mod runtime_native_math;
#[path = "runtime_native_number.rs"]
mod runtime_native_number;
#[path = "runtime_native_string.rs"]
mod runtime_native_string;

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
const BOOLEAN_FUNCTION_LENGTH: f64 = 1.0;
const BOOLEAN_NAME: &str = "Boolean";
const ERROR_FUNCTION_LENGTH: f64 = 1.0;
const INFINITY_NAME: &str = "Infinity";
const MATH_ABS_NAME: &str = "abs";
const MATH_ACOS_NAME: &str = "acos";
const MATH_ACOSH_NAME: &str = "acosh";
const MATH_ASIN_NAME: &str = "asin";
const MATH_ASINH_NAME: &str = "asinh";
const MATH_ATAN_NAME: &str = "atan";
const MATH_ATAN2_NAME: &str = "atan2";
const MATH_ATANH_NAME: &str = "atanh";
const MATH_CBRT_NAME: &str = "cbrt";
const MATH_CEIL_NAME: &str = "ceil";
const MATH_COS_NAME: &str = "cos";
const MATH_COSH_NAME: &str = "cosh";
const MATH_EXP_NAME: &str = "exp";
const MATH_EXPM1_NAME: &str = "expm1";
const MATH_FLOOR_NAME: &str = "floor";
const MATH_FUNCTION_LENGTH_ONE: f64 = 1.0;
const MATH_FUNCTION_LENGTH_TWO: f64 = 2.0;
const MATH_HYPOT_NAME: &str = "hypot";
const MATH_LOG_NAME: &str = "log";
const MATH_LOG10_NAME: &str = "log10";
const MATH_LOG1P_NAME: &str = "log1p";
const MATH_LOG2_NAME: &str = "log2";
const MATH_MAX_NAME: &str = "max";
const MATH_MIN_NAME: &str = "min";
const MATH_NAME: &str = "Math";
const MATH_POW_NAME: &str = "pow";
const MATH_ROUND_NAME: &str = "round";
const MATH_SIGN_NAME: &str = "sign";
const MATH_SIN_NAME: &str = "sin";
const MATH_SINH_NAME: &str = "sinh";
const MATH_SQRT_NAME: &str = "sqrt";
const MATH_TAN_NAME: &str = "tan";
const MATH_TANH_NAME: &str = "tanh";
const MATH_TRUNC_NAME: &str = "trunc";
const NAN_NAME: &str = "NaN";
const NUMBER_FUNCTION_LENGTH: f64 = 1.0;
const NUMBER_NAME: &str = "Number";
const OBJECT_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_NAME: &str = "Object";
const STRING_FUNCTION_LENGTH: f64 = 1.0;
const STRING_NAME: &str = "String";

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
            NativeFunctionKind::Boolean => BOOLEAN_FUNCTION_LENGTH,
            NativeFunctionKind::ErrorConstructor(_) => ERROR_FUNCTION_LENGTH,
            NativeFunctionKind::MathAbs
            | NativeFunctionKind::MathAcos
            | NativeFunctionKind::MathAcosh
            | NativeFunctionKind::MathAsin
            | NativeFunctionKind::MathAsinh
            | NativeFunctionKind::MathAtan
            | NativeFunctionKind::MathAtanh
            | NativeFunctionKind::MathCbrt
            | NativeFunctionKind::MathCeil
            | NativeFunctionKind::MathCos
            | NativeFunctionKind::MathCosh
            | NativeFunctionKind::MathExp
            | NativeFunctionKind::MathExpm1
            | NativeFunctionKind::MathFloor
            | NativeFunctionKind::MathLog
            | NativeFunctionKind::MathLog10
            | NativeFunctionKind::MathLog1p
            | NativeFunctionKind::MathLog2
            | NativeFunctionKind::MathRound
            | NativeFunctionKind::MathSign
            | NativeFunctionKind::MathSin
            | NativeFunctionKind::MathSinh
            | NativeFunctionKind::MathSqrt
            | NativeFunctionKind::MathTan
            | NativeFunctionKind::MathTanh
            | NativeFunctionKind::MathTrunc => MATH_FUNCTION_LENGTH_ONE,
            NativeFunctionKind::MathAtan2
            | NativeFunctionKind::MathHypot
            | NativeFunctionKind::MathMax
            | NativeFunctionKind::MathMin
            | NativeFunctionKind::MathPow => MATH_FUNCTION_LENGTH_TWO,
            NativeFunctionKind::Number => NUMBER_FUNCTION_LENGTH,
            NativeFunctionKind::Object => OBJECT_FUNCTION_LENGTH,
            NativeFunctionKind::String => STRING_FUNCTION_LENGTH,
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
            NativeFunctionKind::Boolean => BOOLEAN_NAME,
            NativeFunctionKind::ErrorConstructor(name) => name.as_str(),
            NativeFunctionKind::MathAbs => MATH_ABS_NAME,
            NativeFunctionKind::MathAcos => MATH_ACOS_NAME,
            NativeFunctionKind::MathAcosh => MATH_ACOSH_NAME,
            NativeFunctionKind::MathAsin => MATH_ASIN_NAME,
            NativeFunctionKind::MathAsinh => MATH_ASINH_NAME,
            NativeFunctionKind::MathAtan => MATH_ATAN_NAME,
            NativeFunctionKind::MathAtan2 => MATH_ATAN2_NAME,
            NativeFunctionKind::MathAtanh => MATH_ATANH_NAME,
            NativeFunctionKind::MathCbrt => MATH_CBRT_NAME,
            NativeFunctionKind::MathCeil => MATH_CEIL_NAME,
            NativeFunctionKind::MathCos => MATH_COS_NAME,
            NativeFunctionKind::MathCosh => MATH_COSH_NAME,
            NativeFunctionKind::MathExp => MATH_EXP_NAME,
            NativeFunctionKind::MathExpm1 => MATH_EXPM1_NAME,
            NativeFunctionKind::MathFloor => MATH_FLOOR_NAME,
            NativeFunctionKind::MathHypot => MATH_HYPOT_NAME,
            NativeFunctionKind::MathLog => MATH_LOG_NAME,
            NativeFunctionKind::MathLog10 => MATH_LOG10_NAME,
            NativeFunctionKind::MathLog1p => MATH_LOG1P_NAME,
            NativeFunctionKind::MathLog2 => MATH_LOG2_NAME,
            NativeFunctionKind::MathMax => MATH_MAX_NAME,
            NativeFunctionKind::MathMin => MATH_MIN_NAME,
            NativeFunctionKind::MathPow => MATH_POW_NAME,
            NativeFunctionKind::MathRound => MATH_ROUND_NAME,
            NativeFunctionKind::MathSign => MATH_SIGN_NAME,
            NativeFunctionKind::MathSin => MATH_SIN_NAME,
            NativeFunctionKind::MathSinh => MATH_SINH_NAME,
            NativeFunctionKind::MathSqrt => MATH_SQRT_NAME,
            NativeFunctionKind::MathTan => MATH_TAN_NAME,
            NativeFunctionKind::MathTanh => MATH_TANH_NAME,
            NativeFunctionKind::MathTrunc => MATH_TRUNC_NAME,
            NativeFunctionKind::Number => NUMBER_NAME,
            NativeFunctionKind::Object => OBJECT_NAME,
            NativeFunctionKind::String => STRING_NAME,
        }
    }

    pub(super) const fn properties(&self) -> &FunctionProperties {
        &self.properties
    }

    pub(super) const fn properties_mut(&mut self) -> &mut FunctionProperties {
        &mut self.properties
    }

    pub(super) fn intrinsic_property(&self, property: &str) -> Option<Value> {
        match self.kind {
            NativeFunctionKind::Number => {
                runtime_native_number::number_intrinsic_property(property)
            }
            NativeFunctionKind::Array
            | NativeFunctionKind::ArrayConcat
            | NativeFunctionKind::ArrayIncludes
            | NativeFunctionKind::ArrayIndexOf
            | NativeFunctionKind::ArrayJoin
            | NativeFunctionKind::ArrayLastIndexOf
            | NativeFunctionKind::ArrayPop
            | NativeFunctionKind::ArrayPush
            | NativeFunctionKind::ArrayReverse
            | NativeFunctionKind::ArrayShift
            | NativeFunctionKind::ArraySlice
            | NativeFunctionKind::ArrayUnshift
            | NativeFunctionKind::Boolean
            | NativeFunctionKind::ErrorConstructor(_)
            | NativeFunctionKind::MathAbs
            | NativeFunctionKind::MathAcos
            | NativeFunctionKind::MathAcosh
            | NativeFunctionKind::MathAsin
            | NativeFunctionKind::MathAsinh
            | NativeFunctionKind::MathAtan
            | NativeFunctionKind::MathAtan2
            | NativeFunctionKind::MathAtanh
            | NativeFunctionKind::MathCbrt
            | NativeFunctionKind::MathCeil
            | NativeFunctionKind::MathCos
            | NativeFunctionKind::MathCosh
            | NativeFunctionKind::MathExp
            | NativeFunctionKind::MathExpm1
            | NativeFunctionKind::MathFloor
            | NativeFunctionKind::MathHypot
            | NativeFunctionKind::MathLog
            | NativeFunctionKind::MathLog10
            | NativeFunctionKind::MathLog1p
            | NativeFunctionKind::MathLog2
            | NativeFunctionKind::MathMax
            | NativeFunctionKind::MathMin
            | NativeFunctionKind::MathPow
            | NativeFunctionKind::MathRound
            | NativeFunctionKind::MathSign
            | NativeFunctionKind::MathSin
            | NativeFunctionKind::MathSinh
            | NativeFunctionKind::MathSqrt
            | NativeFunctionKind::MathTan
            | NativeFunctionKind::MathTanh
            | NativeFunctionKind::MathTrunc
            | NativeFunctionKind::Object
            | NativeFunctionKind::String => None,
        }
    }

    pub(super) fn has_intrinsic_property(&self, property: &str) -> bool {
        self.intrinsic_property(property).is_some()
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
    Boolean,
    ErrorConstructor(ErrorName),
    MathAbs,
    MathAcos,
    MathAcosh,
    MathAsin,
    MathAsinh,
    MathAtan,
    MathAtan2,
    MathAtanh,
    MathCbrt,
    MathCeil,
    MathCos,
    MathCosh,
    MathExp,
    MathExpm1,
    MathFloor,
    MathHypot,
    MathLog,
    MathLog10,
    MathLog1p,
    MathLog2,
    MathMax,
    MathMin,
    MathPow,
    MathRound,
    MathSign,
    MathSin,
    MathSinh,
    MathSqrt,
    MathTan,
    MathTanh,
    MathTrunc,
    Number,
    Object,
    String,
}

impl Context {
    pub(crate) fn builtin_value(&mut self, name: &str) -> Result<Option<Value>> {
        match name {
            ARRAY_NAME => self.array_constructor_value().map(Some),
            BOOLEAN_NAME => self.boolean_constructor_value().map(Some),
            INFINITY_NAME => self
                .global_constant_value(INFINITY_NAME, Value::Number(f64::INFINITY))
                .map(Some),
            MATH_NAME => self.math_object_value().map(Some),
            NAN_NAME => self
                .global_constant_value(NAN_NAME, Value::Number(f64::NAN))
                .map(Some),
            NUMBER_NAME => self.number_constructor_value().map(Some),
            OBJECT_NAME => self.object_constructor_value().map(Some),
            STRING_NAME => self.string_constructor_value().map(Some),
            _ => {
                let Some(name) =
                    ErrorName::from_constructor_name(name).filter(|name| name.is_standard())
                else {
                    return Ok(None);
                };
                self.error_constructor_value(name).map(Some)
            }
        }
    }

    pub(crate) fn materialize_builtin_binding(&mut self, name: &str) -> Result<bool> {
        self.builtin_value(name).map(|value| value.is_some())
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
            NativeFunctionKind::Boolean => self.eval_boolean_constructor(args),
            NativeFunctionKind::ErrorConstructor(name) => self.eval_error_constructor(name, args),
            NativeFunctionKind::MathAbs => self.eval_math_abs(args),
            NativeFunctionKind::MathAcos => self.eval_math_acos(args),
            NativeFunctionKind::MathAcosh => self.eval_math_acosh(args),
            NativeFunctionKind::MathAsin => self.eval_math_asin(args),
            NativeFunctionKind::MathAsinh => self.eval_math_asinh(args),
            NativeFunctionKind::MathAtan => self.eval_math_atan(args),
            NativeFunctionKind::MathAtan2 => self.eval_math_atan2(args),
            NativeFunctionKind::MathAtanh => self.eval_math_atanh(args),
            NativeFunctionKind::MathCbrt => self.eval_math_cbrt(args),
            NativeFunctionKind::MathCeil => self.eval_math_ceil(args),
            NativeFunctionKind::MathCos => self.eval_math_cos(args),
            NativeFunctionKind::MathCosh => self.eval_math_cosh(args),
            NativeFunctionKind::MathExp => self.eval_math_exp(args),
            NativeFunctionKind::MathExpm1 => self.eval_math_expm1(args),
            NativeFunctionKind::MathFloor => self.eval_math_floor(args),
            NativeFunctionKind::MathHypot => self.eval_math_hypot(args),
            NativeFunctionKind::MathLog => self.eval_math_log(args),
            NativeFunctionKind::MathLog10 => self.eval_math_log10(args),
            NativeFunctionKind::MathLog1p => self.eval_math_log1p(args),
            NativeFunctionKind::MathLog2 => self.eval_math_log2(args),
            NativeFunctionKind::MathMax => self.eval_math_max(args),
            NativeFunctionKind::MathMin => self.eval_math_min(args),
            NativeFunctionKind::MathPow => self.eval_math_pow(args),
            NativeFunctionKind::MathRound => self.eval_math_round(args),
            NativeFunctionKind::MathSign => self.eval_math_sign(args),
            NativeFunctionKind::MathSin => self.eval_math_sin(args),
            NativeFunctionKind::MathSinh => self.eval_math_sinh(args),
            NativeFunctionKind::MathSqrt => self.eval_math_sqrt(args),
            NativeFunctionKind::MathTan => self.eval_math_tan(args),
            NativeFunctionKind::MathTanh => self.eval_math_tanh(args),
            NativeFunctionKind::MathTrunc => self.eval_math_trunc(args),
            NativeFunctionKind::Number => self.eval_number_constructor(args),
            NativeFunctionKind::Object => self.eval_object_constructor(args),
            NativeFunctionKind::String => self.eval_string_constructor(args),
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
            | NativeFunctionKind::ArrayUnshift
            | NativeFunctionKind::MathAbs
            | NativeFunctionKind::MathAcos
            | NativeFunctionKind::MathAcosh
            | NativeFunctionKind::MathAsin
            | NativeFunctionKind::MathAsinh
            | NativeFunctionKind::MathAtan
            | NativeFunctionKind::MathAtan2
            | NativeFunctionKind::MathAtanh
            | NativeFunctionKind::MathCbrt
            | NativeFunctionKind::MathCeil
            | NativeFunctionKind::MathCos
            | NativeFunctionKind::MathCosh
            | NativeFunctionKind::MathExp
            | NativeFunctionKind::MathExpm1
            | NativeFunctionKind::MathFloor
            | NativeFunctionKind::MathHypot
            | NativeFunctionKind::MathLog
            | NativeFunctionKind::MathLog10
            | NativeFunctionKind::MathLog1p
            | NativeFunctionKind::MathLog2
            | NativeFunctionKind::MathMax
            | NativeFunctionKind::MathMin
            | NativeFunctionKind::MathPow
            | NativeFunctionKind::MathRound
            | NativeFunctionKind::MathSign
            | NativeFunctionKind::MathSin
            | NativeFunctionKind::MathSinh
            | NativeFunctionKind::MathSqrt
            | NativeFunctionKind::MathTan
            | NativeFunctionKind::MathTanh
            | NativeFunctionKind::MathTrunc => {
                Err(Error::runtime("native method is not a constructor"))
            }
            NativeFunctionKind::Boolean => self.construct_boolean_object(args),
            NativeFunctionKind::ErrorConstructor(name) => self.eval_error_constructor(name, args),
            NativeFunctionKind::Number => self.construct_number_object(args),
            NativeFunctionKind::Object => self.eval_object_constructor(args),
            NativeFunctionKind::String => self.construct_string_object(args),
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

    fn error_constructor_value(&mut self, name: ErrorName) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::ErrorConstructor(name)) {
            return Ok(Value::NativeFunction(id));
        }

        let id = NativeFunctionId::new(self.native_functions.len());
        let constructor = Value::NativeFunction(id);
        let prototype = self.error_prototype_with_constructor(constructor.clone())?;
        self.native_functions.push(NativeFunction::new(
            NativeFunctionKind::ErrorConstructor(name),
            prototype,
        ));
        self.insert_global_builtin(name.as_str(), constructor.clone())?;
        Ok(constructor)
    }

    fn global_constant_value(&mut self, name: &str, value: Value) -> Result<Value> {
        self.insert_global_builtin(name, value.clone())?;
        Ok(value)
    }

    fn insert_global_builtin(&mut self, name: &str, value: Value) -> Result<()> {
        let atom = self.intern_atom(name)?;
        if self.globals.contains(atom) {
            return Ok(());
        }
        self.ensure_extra_binding_capacity(1)?;
        self.globals
            .insert(atom, BindingCell::new(value, false, DeclKind::Const));
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

    fn error_prototype_with_constructor(&mut self, constructor: Value) -> Result<Value> {
        self.objects
            .create_with_prototype_property(
                None,
                OBJECT_CONSTRUCTOR_PROPERTY.to_owned(),
                constructor,
                PropertyEnumerable::No,
                self.limits.max_objects,
                self.limits.max_object_properties,
            )
            .map(Value::Object)
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
            Value::Object(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => Ok(value.clone()),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_) => self.create_object_from_constructor(),
        }
    }

    pub(super) fn eval_error_constructor(
        &mut self,
        name: ErrorName,
        args: &[Expr],
    ) -> Result<Value> {
        let values = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>>>()?;
        let message = values
            .first()
            .map_or_else(String::new, Value::display_for_concat);
        self.check_string_len(&message)?;
        Ok(Value::Error(ErrorObject::new(name, message)))
    }

    fn create_object_from_constructor(&mut self) -> Result<Value> {
        self.objects.create_with_prototype(
            None,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}
