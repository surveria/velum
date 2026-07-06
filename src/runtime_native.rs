use crate::{
    ast::{DeclKind, Expr, StaticBinding},
    error::{Error, Result},
    runtime::Context,
    runtime_object::{
        DataPropertyDescriptor, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
        PropertyWritable,
    },
    runtime_scope::BindingCell,
    value::{ErrorName, ErrorObject, NativeFunctionId, Value},
};

use super::runtime_function_properties::{FunctionIntrinsicDefaults, FunctionProperties};

#[path = "runtime_native_array.rs"]
mod runtime_native_array;
#[path = "runtime_native_boolean.rs"]
mod runtime_native_boolean;
#[path = "runtime_native_json.rs"]
mod runtime_native_json;
#[path = "runtime_native_math.rs"]
mod runtime_native_math;
#[path = "runtime_native_number.rs"]
mod runtime_native_number;
#[path = "runtime_native_object.rs"]
mod runtime_native_object;
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
const JSON_NAME: &str = "JSON";
const JSON_PARSE_FUNCTION_LENGTH: f64 = 2.0;
const JSON_PARSE_NAME: &str = "parse";
const JSON_STRINGIFY_FUNCTION_LENGTH: f64 = 3.0;
const JSON_STRINGIFY_NAME: &str = "stringify";
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
const MATH_CLZ32_NAME: &str = "clz32";
const MATH_COS_NAME: &str = "cos";
const MATH_COSH_NAME: &str = "cosh";
const MATH_EXP_NAME: &str = "exp";
const MATH_EXPM1_NAME: &str = "expm1";
const MATH_FLOOR_NAME: &str = "floor";
const MATH_FROUND_NAME: &str = "fround";
const MATH_FUNCTION_LENGTH_ONE: f64 = 1.0;
const MATH_FUNCTION_LENGTH_TWO: f64 = 2.0;
const MATH_HYPOT_NAME: &str = "hypot";
const MATH_IMUL_NAME: &str = "imul";
const MATH_LOG_NAME: &str = "log";
const MATH_LOG10_NAME: &str = "log10";
const MATH_LOG1P_NAME: &str = "log1p";
const MATH_LOG2_NAME: &str = "log2";
const MATH_MAX_NAME: &str = "max";
const MATH_MIN_NAME: &str = "min";
const MATH_NAME: &str = "Math";
const MATH_POW_NAME: &str = "pow";
const MATH_RANDOM_NAME: &str = "random";
const MATH_ROUND_NAME: &str = "round";
const MATH_SIGN_NAME: &str = "sign";
const MATH_SIN_NAME: &str = "sin";
const MATH_SINH_NAME: &str = "sinh";
const MATH_SQRT_NAME: &str = "sqrt";
const MATH_TAN_NAME: &str = "tan";
const MATH_TANH_NAME: &str = "tanh";
const MATH_TRUNC_NAME: &str = "trunc";
const MATH_FUNCTION_LENGTH_ZERO: f64 = 0.0;
const NAN_NAME: &str = "NaN";
const NUMBER_FUNCTION_LENGTH: f64 = 1.0;
const NUMBER_NAME: &str = "Number";
const OBJECT_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_DEFINE_PROPERTY_FUNCTION_LENGTH: f64 = 3.0;
const OBJECT_DEFINE_PROPERTY_NAME: &str = "defineProperty";
const OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME: &str = "getOwnPropertyDescriptor";
const OBJECT_HAS_OWN_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_HAS_OWN_NAME: &str = "hasOwn";
const OBJECT_KEYS_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_KEYS_NAME: &str = "keys";
const OBJECT_NAME: &str = "Object";
const STRING_FUNCTION_LENGTH: f64 = 1.0;
const STRING_NAME: &str = "String";

#[derive(Debug, Clone)]
pub(super) struct NativeFunction {
    kind: NativeFunctionKind,
    properties: FunctionProperties,
}

impl NativeFunction {
    fn new(kind: NativeFunctionKind, prototype: Value) -> Self {
        let prototype_default = DataPropertyDescriptor::new(
            prototype.clone(),
            PropertyWritable::No,
            PropertyEnumerable::No,
            PropertyConfigurable::No,
        );
        let intrinsic_defaults = FunctionIntrinsicDefaults::new(
            Value::Number(kind.length()),
            Value::String(kind.name().to_owned()),
            Some(prototype_default),
        );
        Self {
            kind,
            properties: FunctionProperties::new(prototype, intrinsic_defaults),
        }
    }

    pub(super) const fn kind(&self) -> NativeFunctionKind {
        self.kind
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
            | NativeFunctionKind::JsonParse
            | NativeFunctionKind::JsonStringify
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
            | NativeFunctionKind::MathClz32
            | NativeFunctionKind::MathCos
            | NativeFunctionKind::MathCosh
            | NativeFunctionKind::MathExp
            | NativeFunctionKind::MathExpm1
            | NativeFunctionKind::MathFloor
            | NativeFunctionKind::MathFround
            | NativeFunctionKind::MathHypot
            | NativeFunctionKind::MathImul
            | NativeFunctionKind::MathLog
            | NativeFunctionKind::MathLog10
            | NativeFunctionKind::MathLog1p
            | NativeFunctionKind::MathLog2
            | NativeFunctionKind::MathMax
            | NativeFunctionKind::MathMin
            | NativeFunctionKind::MathPow
            | NativeFunctionKind::MathRandom
            | NativeFunctionKind::MathRound
            | NativeFunctionKind::MathSign
            | NativeFunctionKind::MathSin
            | NativeFunctionKind::MathSinh
            | NativeFunctionKind::MathSqrt
            | NativeFunctionKind::MathTan
            | NativeFunctionKind::MathTanh
            | NativeFunctionKind::MathTrunc
            | NativeFunctionKind::Object
            | NativeFunctionKind::ObjectDefineProperty
            | NativeFunctionKind::ObjectGetOwnPropertyDescriptor
            | NativeFunctionKind::ObjectHasOwn
            | NativeFunctionKind::ObjectKeys
            | NativeFunctionKind::String => None,
        }
    }

    pub(super) fn has_intrinsic_property(&self, property: &str) -> bool {
        self.intrinsic_property(property).is_some()
    }
}

impl NativeFunctionKind {
    const fn length(self) -> f64 {
        match self {
            Self::Array => ARRAY_FUNCTION_LENGTH,
            Self::ArrayConcat => ARRAY_CONCAT_FUNCTION_LENGTH,
            Self::ArrayIncludes => ARRAY_INCLUDES_FUNCTION_LENGTH,
            Self::ArrayIndexOf => ARRAY_INDEX_OF_FUNCTION_LENGTH,
            Self::ArrayJoin => ARRAY_JOIN_FUNCTION_LENGTH,
            Self::ArrayLastIndexOf => ARRAY_LAST_INDEX_OF_FUNCTION_LENGTH,
            Self::ArrayPop => ARRAY_POP_FUNCTION_LENGTH,
            Self::ArrayPush => ARRAY_PUSH_FUNCTION_LENGTH,
            Self::ArrayReverse => ARRAY_REVERSE_FUNCTION_LENGTH,
            Self::ArrayShift => ARRAY_SHIFT_FUNCTION_LENGTH,
            Self::ArraySlice => ARRAY_SLICE_FUNCTION_LENGTH,
            Self::ArrayUnshift => ARRAY_UNSHIFT_FUNCTION_LENGTH,
            Self::Boolean => BOOLEAN_FUNCTION_LENGTH,
            Self::ErrorConstructor(_) => ERROR_FUNCTION_LENGTH,
            Self::JsonParse => JSON_PARSE_FUNCTION_LENGTH,
            Self::JsonStringify => JSON_STRINGIFY_FUNCTION_LENGTH,
            Self::MathRandom => MATH_FUNCTION_LENGTH_ZERO,
            Self::MathAbs
            | Self::MathAcos
            | Self::MathAcosh
            | Self::MathAsin
            | Self::MathAsinh
            | Self::MathAtan
            | Self::MathAtanh
            | Self::MathCbrt
            | Self::MathCeil
            | Self::MathClz32
            | Self::MathCos
            | Self::MathCosh
            | Self::MathExp
            | Self::MathExpm1
            | Self::MathFloor
            | Self::MathFround
            | Self::MathLog
            | Self::MathLog10
            | Self::MathLog1p
            | Self::MathLog2
            | Self::MathRound
            | Self::MathSign
            | Self::MathSin
            | Self::MathSinh
            | Self::MathSqrt
            | Self::MathTan
            | Self::MathTanh
            | Self::MathTrunc => MATH_FUNCTION_LENGTH_ONE,
            Self::MathAtan2
            | Self::MathHypot
            | Self::MathImul
            | Self::MathMax
            | Self::MathMin
            | Self::MathPow => MATH_FUNCTION_LENGTH_TWO,
            Self::Number => NUMBER_FUNCTION_LENGTH,
            Self::Object => OBJECT_FUNCTION_LENGTH,
            Self::ObjectDefineProperty => OBJECT_DEFINE_PROPERTY_FUNCTION_LENGTH,
            Self::ObjectGetOwnPropertyDescriptor => {
                OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_FUNCTION_LENGTH
            }
            Self::ObjectHasOwn => OBJECT_HAS_OWN_FUNCTION_LENGTH,
            Self::ObjectKeys => OBJECT_KEYS_FUNCTION_LENGTH,
            Self::String => STRING_FUNCTION_LENGTH,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Array => ARRAY_NAME,
            Self::ArrayConcat => ARRAY_CONCAT_NAME,
            Self::ArrayIncludes => ARRAY_INCLUDES_NAME,
            Self::ArrayIndexOf => ARRAY_INDEX_OF_NAME,
            Self::ArrayJoin => ARRAY_JOIN_NAME,
            Self::ArrayLastIndexOf => ARRAY_LAST_INDEX_OF_NAME,
            Self::ArrayPop => ARRAY_POP_NAME,
            Self::ArrayPush => ARRAY_PUSH_NAME,
            Self::ArrayReverse => ARRAY_REVERSE_NAME,
            Self::ArrayShift => ARRAY_SHIFT_NAME,
            Self::ArraySlice => ARRAY_SLICE_NAME,
            Self::ArrayUnshift => ARRAY_UNSHIFT_NAME,
            Self::Boolean => BOOLEAN_NAME,
            Self::ErrorConstructor(name) => name.as_str(),
            Self::JsonParse => JSON_PARSE_NAME,
            Self::JsonStringify => JSON_STRINGIFY_NAME,
            Self::MathAbs => MATH_ABS_NAME,
            Self::MathAcos => MATH_ACOS_NAME,
            Self::MathAcosh => MATH_ACOSH_NAME,
            Self::MathAsin => MATH_ASIN_NAME,
            Self::MathAsinh => MATH_ASINH_NAME,
            Self::MathAtan => MATH_ATAN_NAME,
            Self::MathAtan2 => MATH_ATAN2_NAME,
            Self::MathAtanh => MATH_ATANH_NAME,
            Self::MathCbrt => MATH_CBRT_NAME,
            Self::MathCeil => MATH_CEIL_NAME,
            Self::MathClz32 => MATH_CLZ32_NAME,
            Self::MathCos => MATH_COS_NAME,
            Self::MathCosh => MATH_COSH_NAME,
            Self::MathExp => MATH_EXP_NAME,
            Self::MathExpm1 => MATH_EXPM1_NAME,
            Self::MathFloor => MATH_FLOOR_NAME,
            Self::MathFround => MATH_FROUND_NAME,
            Self::MathHypot => MATH_HYPOT_NAME,
            Self::MathImul => MATH_IMUL_NAME,
            Self::MathLog => MATH_LOG_NAME,
            Self::MathLog10 => MATH_LOG10_NAME,
            Self::MathLog1p => MATH_LOG1P_NAME,
            Self::MathLog2 => MATH_LOG2_NAME,
            Self::MathMax => MATH_MAX_NAME,
            Self::MathMin => MATH_MIN_NAME,
            Self::MathPow => MATH_POW_NAME,
            Self::MathRandom => MATH_RANDOM_NAME,
            Self::MathRound => MATH_ROUND_NAME,
            Self::MathSign => MATH_SIGN_NAME,
            Self::MathSin => MATH_SIN_NAME,
            Self::MathSinh => MATH_SINH_NAME,
            Self::MathSqrt => MATH_SQRT_NAME,
            Self::MathTan => MATH_TAN_NAME,
            Self::MathTanh => MATH_TANH_NAME,
            Self::MathTrunc => MATH_TRUNC_NAME,
            Self::Number => NUMBER_NAME,
            Self::Object => OBJECT_NAME,
            Self::ObjectDefineProperty => OBJECT_DEFINE_PROPERTY_NAME,
            Self::ObjectGetOwnPropertyDescriptor => OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME,
            Self::ObjectHasOwn => OBJECT_HAS_OWN_NAME,
            Self::ObjectKeys => OBJECT_KEYS_NAME,
            Self::String => STRING_NAME,
        }
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
    JsonParse,
    JsonStringify,
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
    MathClz32,
    MathCos,
    MathCosh,
    MathExp,
    MathExpm1,
    MathFloor,
    MathFround,
    MathHypot,
    MathImul,
    MathLog,
    MathLog10,
    MathLog1p,
    MathLog2,
    MathMax,
    MathMin,
    MathPow,
    MathRandom,
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
    ObjectDefineProperty,
    ObjectGetOwnPropertyDescriptor,
    ObjectHasOwn,
    ObjectKeys,
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
            JSON_NAME => self.json_object_value().map(Some),
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

    pub(crate) fn constructor_binding_static(
        &mut self,
        name: &StaticBinding,
    ) -> Result<Option<Value>> {
        if let Some(binding) = self.get_or_materialize_binding_static(name)? {
            return Ok(Some(binding.value()));
        }
        Ok(None)
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
            NativeFunctionKind::JsonParse => self.eval_json_parse(args),
            NativeFunctionKind::JsonStringify => self.eval_json_stringify(args),
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
            NativeFunctionKind::MathClz32 => self.eval_math_clz32(args),
            NativeFunctionKind::MathCos => self.eval_math_cos(args),
            NativeFunctionKind::MathCosh => self.eval_math_cosh(args),
            NativeFunctionKind::MathExp => self.eval_math_exp(args),
            NativeFunctionKind::MathExpm1 => self.eval_math_expm1(args),
            NativeFunctionKind::MathFloor => self.eval_math_floor(args),
            NativeFunctionKind::MathFround => self.eval_math_fround(args),
            NativeFunctionKind::MathHypot => self.eval_math_hypot(args),
            NativeFunctionKind::MathImul => self.eval_math_imul(args),
            NativeFunctionKind::MathLog => self.eval_math_log(args),
            NativeFunctionKind::MathLog10 => self.eval_math_log10(args),
            NativeFunctionKind::MathLog1p => self.eval_math_log1p(args),
            NativeFunctionKind::MathLog2 => self.eval_math_log2(args),
            NativeFunctionKind::MathMax => self.eval_math_max(args),
            NativeFunctionKind::MathMin => self.eval_math_min(args),
            NativeFunctionKind::MathPow => self.eval_math_pow(args),
            NativeFunctionKind::MathRandom => self.eval_math_random(args),
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
            NativeFunctionKind::ObjectDefineProperty => self.eval_object_define_property(args),
            NativeFunctionKind::ObjectGetOwnPropertyDescriptor => {
                self.eval_object_get_own_property_descriptor(args)
            }
            NativeFunctionKind::ObjectHasOwn => self.eval_object_has_own(args),
            NativeFunctionKind::ObjectKeys => self.eval_object_keys(args),
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
            | NativeFunctionKind::JsonParse
            | NativeFunctionKind::JsonStringify
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
            | NativeFunctionKind::MathClz32
            | NativeFunctionKind::MathCos
            | NativeFunctionKind::MathCosh
            | NativeFunctionKind::MathExp
            | NativeFunctionKind::MathExpm1
            | NativeFunctionKind::MathFloor
            | NativeFunctionKind::MathFround
            | NativeFunctionKind::MathHypot
            | NativeFunctionKind::MathImul
            | NativeFunctionKind::MathLog
            | NativeFunctionKind::MathLog10
            | NativeFunctionKind::MathLog1p
            | NativeFunctionKind::MathLog2
            | NativeFunctionKind::MathMax
            | NativeFunctionKind::MathMin
            | NativeFunctionKind::MathPow
            | NativeFunctionKind::MathRandom
            | NativeFunctionKind::MathRound
            | NativeFunctionKind::MathSign
            | NativeFunctionKind::MathSin
            | NativeFunctionKind::MathSinh
            | NativeFunctionKind::MathSqrt
            | NativeFunctionKind::MathTan
            | NativeFunctionKind::MathTanh
            | NativeFunctionKind::MathTrunc
            | NativeFunctionKind::ObjectDefineProperty
            | NativeFunctionKind::ObjectGetOwnPropertyDescriptor
            | NativeFunctionKind::ObjectHasOwn
            | NativeFunctionKind::ObjectKeys => {
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
        if self.builtin_globals.contains(atom) {
            return Ok(());
        }
        self.ensure_extra_binding_capacity(1)?;
        self.builtin_globals
            .insert(atom, BindingCell::new(value, false, DeclKind::Const));
        Ok(())
    }

    fn object_prototype_id_with_constructor(&mut self, constructor: Value) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.define_non_enumerable_object_property(
            prototype,
            OBJECT_CONSTRUCTOR_PROPERTY,
            constructor,
        )?;
        Ok(Value::Object(prototype))
    }

    fn error_prototype_with_constructor(&mut self, constructor: Value) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects
            .create_with_prototype_property(
                None,
                ObjectPropertyInit::new(
                    constructor_key,
                    OBJECT_CONSTRUCTOR_PROPERTY,
                    constructor,
                    PropertyEnumerable::No,
                ),
                constructor_key,
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
}
