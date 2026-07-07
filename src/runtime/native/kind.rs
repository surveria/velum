use crate::value::ErrorName;

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
const ASYNC_FUNCTION_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const ASYNC_FUNCTION_NAME: &str = "AsyncFunction";
const ARRAY_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const ARRAY_NAME: &str = "Array";
const BOOLEAN_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const BOOLEAN_NAME: &str = "Boolean";
const EVAL_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const EVAL_NAME: &str = "eval";
const ERROR_FUNCTION_LENGTH: f64 = 1.0;
const FUNCTION_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const FUNCTION_NAME: &str = "Function";
pub(super) const INFINITY_NAME: &str = "Infinity";
pub(super) const JSON_NAME: &str = "JSON";
const JSON_PARSE_FUNCTION_LENGTH: f64 = 2.0;
pub(super) const JSON_PARSE_NAME: &str = "parse";
const JSON_STRINGIFY_FUNCTION_LENGTH: f64 = 3.0;
pub(super) const JSON_STRINGIFY_NAME: &str = "stringify";
pub(super) const MATH_ABS_NAME: &str = "abs";
pub(super) const MATH_ACOS_NAME: &str = "acos";
pub(super) const MATH_ACOSH_NAME: &str = "acosh";
pub(super) const MATH_ASIN_NAME: &str = "asin";
pub(super) const MATH_ASINH_NAME: &str = "asinh";
pub(super) const MATH_ATAN_NAME: &str = "atan";
pub(super) const MATH_ATAN2_NAME: &str = "atan2";
pub(super) const MATH_ATANH_NAME: &str = "atanh";
pub(super) const MATH_CBRT_NAME: &str = "cbrt";
pub(super) const MATH_CEIL_NAME: &str = "ceil";
pub(super) const MATH_CLZ32_NAME: &str = "clz32";
pub(super) const MATH_COS_NAME: &str = "cos";
pub(super) const MATH_COSH_NAME: &str = "cosh";
pub(super) const MATH_EXP_NAME: &str = "exp";
pub(super) const MATH_EXPM1_NAME: &str = "expm1";
pub(super) const MATH_FLOOR_NAME: &str = "floor";
pub(super) const MATH_FROUND_NAME: &str = "fround";
const MATH_FUNCTION_LENGTH_ONE: f64 = 1.0;
const MATH_FUNCTION_LENGTH_TWO: f64 = 2.0;
pub(super) const MATH_HYPOT_NAME: &str = "hypot";
pub(super) const MATH_IMUL_NAME: &str = "imul";
pub(super) const MATH_LOG_NAME: &str = "log";
pub(super) const MATH_LOG10_NAME: &str = "log10";
pub(super) const MATH_LOG1P_NAME: &str = "log1p";
pub(super) const MATH_LOG2_NAME: &str = "log2";
pub(super) const MATH_MAX_NAME: &str = "max";
pub(super) const MATH_MIN_NAME: &str = "min";
pub(super) const MATH_NAME: &str = "Math";
pub(super) const MATH_POW_NAME: &str = "pow";
pub(super) const MATH_RANDOM_NAME: &str = "random";
pub(super) const MATH_ROUND_NAME: &str = "round";
pub(super) const MATH_SIGN_NAME: &str = "sign";
pub(super) const MATH_SIN_NAME: &str = "sin";
pub(super) const MATH_SINH_NAME: &str = "sinh";
pub(super) const MATH_SQRT_NAME: &str = "sqrt";
pub(super) const MATH_TAN_NAME: &str = "tan";
pub(super) const MATH_TANH_NAME: &str = "tanh";
pub(super) const MATH_TRUNC_NAME: &str = "trunc";
const MATH_FUNCTION_LENGTH_ZERO: f64 = 0.0;
pub(super) const NAN_NAME: &str = "NaN";
const NUMBER_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const NUMBER_NAME: &str = "Number";
const OBJECT_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_DEFINE_PROPERTY_FUNCTION_LENGTH: f64 = 3.0;
pub(super) const OBJECT_DEFINE_PROPERTY_NAME: &str = "defineProperty";
const OBJECT_GET_PROTOTYPE_OF_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const OBJECT_GET_PROTOTYPE_OF_NAME: &str = "getPrototypeOf";
const OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_FUNCTION_LENGTH: f64 = 2.0;
pub(super) const OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME: &str = "getOwnPropertyDescriptor";
const OBJECT_HAS_OWN_FUNCTION_LENGTH: f64 = 2.0;
pub(super) const OBJECT_HAS_OWN_NAME: &str = "hasOwn";
const OBJECT_KEYS_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const OBJECT_KEYS_NAME: &str = "keys";
pub(super) const OBJECT_NAME: &str = "Object";
const PROMISE_CATCH_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const PROMISE_CATCH_NAME: &str = "catch";
const PROMISE_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const PROMISE_NAME: &str = "Promise";
const PROMISE_REJECT_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const PROMISE_REJECT_NAME: &str = "reject";
const PROMISE_RESOLVE_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const PROMISE_RESOLVE_NAME: &str = "resolve";
const PROMISE_RESOLVER_FUNCTION_LENGTH: f64 = 1.0;
const PROMISE_THEN_FUNCTION_LENGTH: f64 = 2.0;
pub(super) const PROMISE_THEN_NAME: &str = "then";
const REJECT_NAME: &str = "reject";
const RESOLVE_NAME: &str = "resolve";
const STRING_FUNCTION_LENGTH: f64 = 1.0;
pub(super) const STRING_NAME: &str = "String";
const SYMBOL_FUNCTION_LENGTH: f64 = 0.0;
pub(super) const SYMBOL_NAME: &str = "Symbol";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum NativeFunctionKind {
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
    AsyncFunction,
    Boolean,
    Eval,
    ErrorConstructor(ErrorName),
    Function,
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
    ObjectGetPrototypeOf,
    ObjectGetOwnPropertyDescriptor,
    ObjectHasOwn,
    ObjectKeys,
    Promise,
    PromiseResolve,
    PromiseReject,
    PromiseThen,
    PromiseCatch,
    PromiseResolver {
        promise: crate::runtime::promise::PromiseId,
        kind: crate::runtime::promise::PromiseResolverKind,
    },
    String,
    Symbol,
}

impl NativeFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
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
            Self::AsyncFunction => ASYNC_FUNCTION_FUNCTION_LENGTH,
            Self::Boolean => BOOLEAN_FUNCTION_LENGTH,
            Self::Eval => EVAL_FUNCTION_LENGTH,
            Self::ErrorConstructor(_) => ERROR_FUNCTION_LENGTH,
            Self::Function => FUNCTION_FUNCTION_LENGTH,
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
            Self::ObjectGetPrototypeOf => OBJECT_GET_PROTOTYPE_OF_FUNCTION_LENGTH,
            Self::ObjectGetOwnPropertyDescriptor => {
                OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_FUNCTION_LENGTH
            }
            Self::ObjectHasOwn => OBJECT_HAS_OWN_FUNCTION_LENGTH,
            Self::ObjectKeys => OBJECT_KEYS_FUNCTION_LENGTH,
            Self::Promise => PROMISE_FUNCTION_LENGTH,
            Self::PromiseResolve => PROMISE_RESOLVE_FUNCTION_LENGTH,
            Self::PromiseReject => PROMISE_REJECT_FUNCTION_LENGTH,
            Self::PromiseThen => PROMISE_THEN_FUNCTION_LENGTH,
            Self::PromiseCatch => PROMISE_CATCH_FUNCTION_LENGTH,
            Self::PromiseResolver { .. } => PROMISE_RESOLVER_FUNCTION_LENGTH,
            Self::String => STRING_FUNCTION_LENGTH,
            Self::Symbol => SYMBOL_FUNCTION_LENGTH,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
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
            Self::AsyncFunction => ASYNC_FUNCTION_NAME,
            Self::Boolean => BOOLEAN_NAME,
            Self::Eval => EVAL_NAME,
            Self::ErrorConstructor(name) => name.as_str(),
            Self::Function => FUNCTION_NAME,
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
            Self::ObjectGetPrototypeOf => OBJECT_GET_PROTOTYPE_OF_NAME,
            Self::ObjectGetOwnPropertyDescriptor => OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME,
            Self::ObjectHasOwn => OBJECT_HAS_OWN_NAME,
            Self::ObjectKeys => OBJECT_KEYS_NAME,
            Self::Promise => PROMISE_NAME,
            Self::PromiseResolve => PROMISE_RESOLVE_NAME,
            Self::PromiseReject => PROMISE_REJECT_NAME,
            Self::PromiseThen => PROMISE_THEN_NAME,
            Self::PromiseCatch => PROMISE_CATCH_NAME,
            Self::PromiseResolver {
                kind: crate::runtime::promise::PromiseResolverKind::Resolve,
                ..
            } => RESOLVE_NAME,
            Self::PromiseResolver {
                kind: crate::runtime::promise::PromiseResolverKind::Reject,
                ..
            } => REJECT_NAME,
            Self::String => STRING_NAME,
            Self::Symbol => SYMBOL_NAME,
        }
    }
}
