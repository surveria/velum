use super::date_kind::DateFunctionKind;
use crate::value::{BoundFunctionId, ErrorName};

mod regexp;
mod string;

pub(in crate::runtime::native) use regexp::{
    REGEXP_NAME, REGEXP_PROTOTYPE_EXEC_NAME, REGEXP_PROTOTYPE_TEST_NAME,
    REGEXP_PROTOTYPE_TO_STRING_NAME,
};

const ASYNC_FUNCTION_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const ASYNC_FUNCTION_NAME: &str = "AsyncFunction";
const BOOLEAN_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const BOOLEAN_NAME: &str = "Boolean";
const EVAL_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const EVAL_NAME: &str = "eval";
const ERROR_PROTOTYPE_TO_STRING_LENGTH: f64 = 0.0;
pub(in crate::runtime::native) const ERROR_PROTOTYPE_TO_STRING_NAME: &str = "toString";
const FUNCTION_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const FUNCTION_NAME: &str = "Function";
const GLOBAL_DECODE_URI_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const GLOBAL_DECODE_URI_NAME: &str = "decodeURI";
const GLOBAL_DECODE_URI_COMPONENT_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const GLOBAL_DECODE_URI_COMPONENT_NAME: &str = "decodeURIComponent";
const GLOBAL_ENCODE_URI_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const GLOBAL_ENCODE_URI_NAME: &str = "encodeURI";
const GLOBAL_ENCODE_URI_COMPONENT_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const GLOBAL_ENCODE_URI_COMPONENT_NAME: &str = "encodeURIComponent";
const GLOBAL_IS_FINITE_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const GLOBAL_IS_FINITE_NAME: &str = "isFinite";
const GLOBAL_IS_NAN_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const GLOBAL_IS_NAN_NAME: &str = "isNaN";
const GLOBAL_PARSE_FLOAT_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const GLOBAL_PARSE_FLOAT_NAME: &str = "parseFloat";
const GLOBAL_PARSE_INT_FUNCTION_LENGTH: f64 = 2.0;
pub(in crate::runtime::native) const GLOBAL_PARSE_INT_NAME: &str = "parseInt";
pub(in crate::runtime) const GLOBAL_THIS_NAME: &str = "globalThis";
const FUNCTION_PROTOTYPE_BIND_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const FUNCTION_PROTOTYPE_BIND_NAME: &str = "bind";
const FUNCTION_PROTOTYPE_APPLY_LENGTH: f64 = 2.0;
pub(in crate::runtime::native) const FUNCTION_PROTOTYPE_APPLY_NAME: &str = "apply";
const FUNCTION_PROTOTYPE_HAS_INSTANCE_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const FUNCTION_PROTOTYPE_HAS_INSTANCE_NAME: &str =
    "[Symbol.hasInstance]";
const FUNCTION_PROTOTYPE_CALL_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const FUNCTION_PROTOTYPE_CALL_NAME: &str = "call";
const BOUND_FUNCTION_LENGTH: f64 = 0.0;
const BOUND_FUNCTION_NAME: &str = "bound";
pub(in crate::runtime) const INFINITY_NAME: &str = "Infinity";
pub(in crate::runtime::native) const JSON_NAME: &str = "JSON";
const JSON_IS_RAW_JSON_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const JSON_IS_RAW_JSON_NAME: &str = "isRawJSON";
const JSON_PARSE_FUNCTION_LENGTH: f64 = 2.0;
pub(in crate::runtime::native) const JSON_PARSE_NAME: &str = "parse";
const JSON_RAW_JSON_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const JSON_RAW_JSON_NAME: &str = "rawJSON";
const JSON_STRINGIFY_FUNCTION_LENGTH: f64 = 3.0;
pub(in crate::runtime::native) const JSON_STRINGIFY_NAME: &str = "stringify";
pub(in crate::runtime::native) const MATH_ABS_NAME: &str = "abs";
pub(in crate::runtime::native) const MATH_ACOS_NAME: &str = "acos";
pub(in crate::runtime::native) const MATH_ACOSH_NAME: &str = "acosh";
pub(in crate::runtime::native) const MATH_ASIN_NAME: &str = "asin";
pub(in crate::runtime::native) const MATH_ASINH_NAME: &str = "asinh";
pub(in crate::runtime::native) const MATH_ATAN_NAME: &str = "atan";
pub(in crate::runtime::native) const MATH_ATAN2_NAME: &str = "atan2";
pub(in crate::runtime::native) const MATH_ATANH_NAME: &str = "atanh";
pub(in crate::runtime::native) const MATH_CBRT_NAME: &str = "cbrt";
pub(in crate::runtime::native) const MATH_CEIL_NAME: &str = "ceil";
pub(in crate::runtime::native) const MATH_CLZ32_NAME: &str = "clz32";
pub(in crate::runtime::native) const MATH_COS_NAME: &str = "cos";
pub(in crate::runtime::native) const MATH_COSH_NAME: &str = "cosh";
pub(in crate::runtime::native) const MATH_EXP_NAME: &str = "exp";
pub(in crate::runtime::native) const MATH_EXPM1_NAME: &str = "expm1";
pub(in crate::runtime::native) const MATH_F16ROUND_NAME: &str = "f16round";
pub(in crate::runtime::native) const MATH_FLOOR_NAME: &str = "floor";
pub(in crate::runtime::native) const MATH_FROUND_NAME: &str = "fround";
const MATH_FUNCTION_LENGTH_ONE: f64 = 1.0;
const MATH_FUNCTION_LENGTH_TWO: f64 = 2.0;
pub(in crate::runtime::native) const MATH_HYPOT_NAME: &str = "hypot";
pub(in crate::runtime::native) const MATH_IMUL_NAME: &str = "imul";
pub(in crate::runtime::native) const MATH_LOG_NAME: &str = "log";
pub(in crate::runtime::native) const MATH_LOG10_NAME: &str = "log10";
pub(in crate::runtime::native) const MATH_LOG1P_NAME: &str = "log1p";
pub(in crate::runtime::native) const MATH_LOG2_NAME: &str = "log2";
pub(in crate::runtime::native) const MATH_MAX_NAME: &str = "max";
pub(in crate::runtime::native) const MATH_MIN_NAME: &str = "min";
pub(in crate::runtime::native) const MATH_NAME: &str = "Math";
pub(in crate::runtime::native) const MATH_POW_NAME: &str = "pow";
pub(in crate::runtime::native) const MATH_RANDOM_NAME: &str = "random";
pub(in crate::runtime::native) const MATH_ROUND_NAME: &str = "round";
pub(in crate::runtime::native) const MATH_SIGN_NAME: &str = "sign";
pub(in crate::runtime::native) const MATH_SIN_NAME: &str = "sin";
pub(in crate::runtime::native) const MATH_SINH_NAME: &str = "sinh";
pub(in crate::runtime::native) const MATH_SQRT_NAME: &str = "sqrt";
pub(in crate::runtime::native) const MATH_SUM_PRECISE_NAME: &str = "sumPrecise";
pub(in crate::runtime::native) const MATH_TAN_NAME: &str = "tan";
pub(in crate::runtime::native) const MATH_TANH_NAME: &str = "tanh";
pub(in crate::runtime::native) const MATH_TRUNC_NAME: &str = "trunc";
const MATH_FUNCTION_LENGTH_ZERO: f64 = 0.0;
pub(in crate::runtime) const NAN_NAME: &str = "NaN";
const NUMBER_FUNCTION_LENGTH: f64 = 1.0;
const NUMBER_IS_FINITE_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const NUMBER_IS_FINITE_NAME: &str = "isFinite";
const NUMBER_IS_INTEGER_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const NUMBER_IS_INTEGER_NAME: &str = "isInteger";
const NUMBER_IS_NAN_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const NUMBER_IS_NAN_NAME: &str = "isNaN";
const NUMBER_IS_SAFE_INTEGER_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const NUMBER_IS_SAFE_INTEGER_NAME: &str = "isSafeInteger";
pub(in crate::runtime::native) const NUMBER_NAME: &str = "Number";
pub(in crate::runtime::native) const OBJECT_ASSIGN_NAME: &str = "assign";
pub(in crate::runtime::native) const OBJECT_CREATE_NAME: &str = "create";
pub(in crate::runtime::native) const OBJECT_DEFINE_PROPERTIES_NAME: &str = "defineProperties";
pub(in crate::runtime::native) const OBJECT_DEFINE_PROPERTY_NAME: &str = "defineProperty";
pub(in crate::runtime::native) const OBJECT_ENTRIES_NAME: &str = "entries";
pub(in crate::runtime::native) const OBJECT_FREEZE_NAME: &str = "freeze";
pub(in crate::runtime::native) const OBJECT_GET_PROTOTYPE_OF_NAME: &str = "getPrototypeOf";
pub(in crate::runtime::native) const OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME: &str =
    "getOwnPropertyDescriptor";
pub(in crate::runtime::native) const OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_NAME: &str =
    "getOwnPropertyDescriptors";
pub(in crate::runtime::native) const OBJECT_GET_OWN_PROPERTY_NAMES_NAME: &str =
    "getOwnPropertyNames";
pub(in crate::runtime::native) const OBJECT_HAS_OWN_NAME: &str = "hasOwn";
pub(in crate::runtime::native) const OBJECT_IS_NAME: &str = "is";
pub(in crate::runtime::native) const OBJECT_IS_EXTENSIBLE_NAME: &str = "isExtensible";
pub(in crate::runtime::native) const OBJECT_IS_FROZEN_NAME: &str = "isFrozen";
pub(in crate::runtime::native) const OBJECT_IS_SEALED_NAME: &str = "isSealed";
pub(in crate::runtime::native) const OBJECT_KEYS_NAME: &str = "keys";
pub(in crate::runtime::native) const OBJECT_NAME: &str = "Object";
pub(in crate::runtime::native) const OBJECT_PREVENT_EXTENSIONS_NAME: &str = "preventExtensions";
pub(in crate::runtime) const OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME: &str = "hasOwnProperty";
pub(in crate::runtime) const OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME: &str =
    "propertyIsEnumerable";
pub(in crate::runtime) const OBJECT_PROTOTYPE_TO_STRING_NAME: &str = "toString";
pub(in crate::runtime) const OBJECT_PROTOTYPE_VALUE_OF_NAME: &str = "valueOf";
pub(in crate::runtime) const OBJECT_PROTOTYPE_TO_LOCALE_STRING_NAME: &str = "toLocaleString";
pub(in crate::runtime) const OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_NAME: &str = "isPrototypeOf";
pub(in crate::runtime::native) const OBJECT_FROM_ENTRIES_NAME: &str = "fromEntries";
pub(in crate::runtime::native) const OBJECT_SET_PROTOTYPE_OF_NAME: &str = "setPrototypeOf";
pub(in crate::runtime::native) const OBJECT_SEAL_NAME: &str = "seal";
pub(in crate::runtime::native) const OBJECT_VALUES_NAME: &str = "values";
const PROMISE_CATCH_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_CATCH_NAME: &str = "catch";
const PROMISE_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_NAME: &str = "Promise";
const PROMISE_REJECT_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_REJECT_NAME: &str = "reject";
const PROMISE_RESOLVE_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const PROMISE_RESOLVE_NAME: &str = "resolve";
const PROMISE_RESOLVER_FUNCTION_LENGTH: f64 = 1.0;
const PROMISE_THEN_FUNCTION_LENGTH: f64 = 2.0;
pub(in crate::runtime::native) const PROMISE_THEN_NAME: &str = "then";
const REJECT_NAME: &str = "reject";
const RESOLVE_NAME: &str = "resolve";
const STRING_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const STRING_FROM_CHAR_CODE_NAME: &str = "fromCharCode";
pub(in crate::runtime::native) const STRING_FROM_CODE_POINT_NAME: &str = "fromCodePoint";
pub(in crate::runtime::native) const STRING_NAME: &str = "String";
pub(in crate::runtime::native) const STRING_RAW_NAME: &str = "raw";
const STRING_PROTOTYPE_FUNCTION_LENGTH_ONE: f64 = 1.0;
pub(in crate::runtime::native) const STRING_PROTOTYPE_AT_NAME: &str = "at";
pub(in crate::runtime::native) const STRING_PROTOTYPE_CHAR_AT_NAME: &str = "charAt";
pub(in crate::runtime::native) const STRING_PROTOTYPE_CHAR_CODE_AT_NAME: &str = "charCodeAt";
pub(in crate::runtime::native) const STRING_PROTOTYPE_CODE_POINT_AT_NAME: &str = "codePointAt";
pub(in crate::runtime::native) const STRING_PROTOTYPE_CONCAT_NAME: &str = "concat";
pub(in crate::runtime::native) const STRING_PROTOTYPE_ENDS_WITH_NAME: &str = "endsWith";
pub(in crate::runtime::native) const STRING_PROTOTYPE_INCLUDES_NAME: &str = "includes";
pub(in crate::runtime::native) const STRING_PROTOTYPE_INDEX_OF_NAME: &str = "indexOf";
pub(in crate::runtime::native) const STRING_PROTOTYPE_LAST_INDEX_OF_NAME: &str = "lastIndexOf";
pub(in crate::runtime::native) const STRING_PROTOTYPE_MATCH_NAME: &str = "match";
pub(in crate::runtime::native) const STRING_PROTOTYPE_REPEAT_NAME: &str = "repeat";
pub(in crate::runtime::native) const STRING_PROTOTYPE_REPLACE_NAME: &str = "replace";
pub(in crate::runtime::native) const STRING_PROTOTYPE_SEARCH_NAME: &str = "search";
const STRING_PROTOTYPE_FUNCTION_LENGTH_TWO: f64 = 2.0;
pub(in crate::runtime::native) const STRING_PROTOTYPE_PAD_END_NAME: &str = "padEnd";
pub(in crate::runtime::native) const STRING_PROTOTYPE_PAD_START_NAME: &str = "padStart";
pub(in crate::runtime::native) const STRING_PROTOTYPE_SLICE_NAME: &str = "slice";
pub(in crate::runtime::native) const STRING_PROTOTYPE_SPLIT_NAME: &str = "split";
pub(in crate::runtime::native) const STRING_PROTOTYPE_STARTS_WITH_NAME: &str = "startsWith";
pub(in crate::runtime::native) const STRING_PROTOTYPE_SUBSTRING_NAME: &str = "substring";
const STRING_PROTOTYPE_FUNCTION_LENGTH_ZERO: f64 = 0.0;
pub(in crate::runtime::native) const STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_NAME: &str =
    "toLocaleLowerCase";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_NAME: &str =
    "toLocaleUpperCase";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TO_LOWER_CASE_NAME: &str = "toLowerCase";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TO_STRING_NAME: &str = "toString";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TO_UPPER_CASE_NAME: &str = "toUpperCase";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_NAME: &str = "trim";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_END_NAME: &str = "trimEnd";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_LEFT_NAME: &str = "trimLeft";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_RIGHT_NAME: &str = "trimRight";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_START_NAME: &str = "trimStart";
pub(in crate::runtime::native) const STRING_PROTOTYPE_VALUE_OF_NAME: &str = "valueOf";
const SYMBOL_FUNCTION_LENGTH: f64 = 0.0;
pub(in crate::runtime::native) const SYMBOL_NAME: &str = "Symbol";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum NativeFunctionKind {
    Array,
    ArrayConcat,
    ArrayEvery,
    ArrayFilter,
    ArrayFind,
    ArrayFindIndex,
    ArrayFlat,
    ArrayFlatMap,
    ArrayForEach,
    ArrayIncludes,
    ArrayIndexOf,
    ArrayIsArray,
    ArrayJoin,
    ArrayLastIndexOf,
    ArrayMap,
    ArrayPop,
    ArrayPush,
    ArrayReduce,
    ArrayReduceRight,
    ArrayReverse,
    ArrayShift,
    ArraySlice,
    ArraySome,
    ArrayUnshift,
    ArraySort,
    ArraySplice,
    ArrayFill,
    ArrayCopyWithin,
    ArrayAt,
    ArrayFindLast,
    ArrayFindLastIndex,
    ArrayToSorted,
    ArrayToReversed,
    ArrayToSpliced,
    ArrayWith,
    AsyncFunction,
    Boolean,
    BooleanPrototypeToString,
    BooleanPrototypeValueOf,
    BoundFunction(BoundFunctionId),
    Date(DateFunctionKind),
    CollectionIteratorNext(crate::runtime::collections::CollectionIteratorId),
    IteratorSelf,
    Eval,
    ErrorConstructor(ErrorName),
    ErrorPrototypeToString,
    Function,
    FunctionPrototypeBind,
    FunctionPrototypeCall,
    FunctionPrototypeApply,
    FunctionPrototypeHasInstance,
    GlobalDecodeUri,
    GlobalDecodeUriComponent,
    GlobalEncodeUri,
    GlobalEncodeUriComponent,
    GlobalIsFinite,
    GlobalIsNan,
    GlobalParseFloat,
    GlobalParseInt,
    JsonIsRawJson,
    JsonParse,
    JsonRawJson,
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
    MathF16round,
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
    MathSumPrecise,
    MathTan,
    MathTanh,
    MathTrunc,
    Map,
    MapClear,
    MapDelete,
    MapEntries,
    MapForEach,
    MapGet,
    MapHas,
    MapKeys,
    MapSet,
    MapSizeGetter,
    MapValues,
    Number,
    NumberIsFinite,
    NumberIsInteger,
    NumberIsNan,
    NumberIsSafeInteger,
    NumberPrototypeToLocaleString,
    NumberPrototypeToString,
    NumberPrototypeValueOf,
    NumberPrototypeToFixed,
    NumberPrototypeToExponential,
    NumberPrototypeToPrecision,
    Object,
    ObjectAssign,
    ObjectCreate,
    ObjectDefineProperties,
    ObjectDefineProperty,
    ObjectEntries,
    ObjectFreeze,
    ObjectGetPrototypeOf,
    ObjectGetOwnPropertyDescriptor,
    ObjectGetOwnPropertyDescriptors,
    ObjectGetOwnPropertyNames,
    ObjectHasOwn,
    ObjectIs,
    ObjectIsExtensible,
    ObjectIsFrozen,
    ObjectIsSealed,
    ObjectKeys,
    ObjectPreventExtensions,
    ObjectPrototypeHasOwnProperty,
    ObjectPrototypePropertyIsEnumerable,
    ObjectPrototypeToString,
    ObjectPrototypeValueOf,
    ObjectPrototypeToLocaleString,
    ObjectPrototypeIsPrototypeOf,
    ObjectFromEntries,
    ObjectSetPrototypeOf,
    ObjectSeal,
    ObjectValues,
    Promise,
    PromiseResolve,
    PromiseReject,
    PromiseThen,
    PromiseCatch,
    PromiseResolver {
        promise: crate::runtime::promise::PromiseId,
        kind: crate::runtime::promise::PromiseResolverKind,
    },
    ReflectApply,
    ReflectConstruct,
    ReflectDefineProperty,
    ReflectDeleteProperty,
    ReflectGet,
    ReflectGetOwnPropertyDescriptor,
    ReflectGetPrototypeOf,
    ReflectHas,
    ReflectIsExtensible,
    ReflectOwnKeys,
    ReflectPreventExtensions,
    ReflectSet,
    ReflectSetPrototypeOf,
    RegExp,
    RegExpPrototypeDotAllGetter,
    RegExpPrototypeExec,
    RegExpPrototypeFlagsGetter,
    RegExpPrototypeGlobalGetter,
    RegExpPrototypeHasIndicesGetter,
    RegExpPrototypeIgnoreCaseGetter,
    RegExpPrototypeMultilineGetter,
    RegExpPrototypeSourceGetter,
    RegExpPrototypeStickyGetter,
    RegExpPrototypeTest,
    RegExpPrototypeToString,
    RegExpPrototypeUnicodeGetter,
    RegExpPrototypeUnicodeSetsGetter,
    String,
    StringFromCharCode,
    StringFromCodePoint,
    StringPrototypeCharAt,
    StringPrototypeCharCodeAt,
    StringPrototypeAt,
    StringPrototypeCodePointAt,
    StringPrototypeConcat,
    StringPrototypeEndsWith,
    StringPrototypeIncludes,
    StringPrototypeIndexOf,
    StringPrototypeLastIndexOf,
    StringPrototypeMatch,
    StringPrototypePadEnd,
    StringPrototypePadStart,
    StringPrototypeRepeat,
    StringPrototypeReplace,
    StringPrototypeSearch,
    StringPrototypeSlice,
    StringPrototypeSplit,
    StringPrototypeStartsWith,
    StringPrototypeSubstring,
    StringPrototypeToLocaleLowerCase,
    StringPrototypeToLocaleUpperCase,
    StringPrototypeToLowerCase,
    StringPrototypeToString,
    StringPrototypeToUpperCase,
    StringPrototypeTrim,
    StringPrototypeTrimEnd,
    StringPrototypeTrimStart,
    StringPrototypeValueOf,
    StringRaw,
    Set,
    SetAdd,
    SetClear,
    SetDelete,
    SetEntries,
    SetForEach,
    SetHas,
    SetSizeGetter,
    SetValues,
    SetUnion,
    SetIntersection,
    SetDifference,
    SetSymmetricDifference,
    SetIsSubsetOf,
    SetIsSupersetOf,
    SetIsDisjointFrom,
    Symbol,
    SymbolPrototypeDescriptionGetter,
    SymbolPrototypeToString,
    SymbolPrototypeValueOf,
    WeakMap,
    WeakMapDelete,
    WeakMapGet,
    WeakMapHas,
    WeakMapSet,
    WeakSet,
    WeakSetAdd,
    WeakSetDelete,
    WeakSetHas,
}

impl NativeFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
        if let Self::Date(kind) = self {
            return kind.length();
        }
        if let Some(length) = self.collection_length() {
            return length;
        }
        if let Some(length) = self.array_length() {
            return length;
        }
        if let Some(length) = self.global_utility_length() {
            return length;
        }
        if let Some(length) = self.math_length() {
            return length;
        }
        if let Some(length) = self.object_length() {
            return length;
        }
        if let Some(length) = self.core_length() {
            return length;
        }
        if let Some(length) = self.primitive_prototype_length() {
            return length;
        }
        if let Some(length) = self.string_static_length() {
            return length;
        }
        if let Some(length) = self.string_prototype_length() {
            return length;
        }
        if let Some(length) = self.reflect_length() {
            return length;
        }
        if let Some(length) = self.regexp_length() {
            return length;
        }
        FUNCTION_FUNCTION_LENGTH
    }

    const fn global_utility_length(self) -> Option<f64> {
        match self {
            Self::GlobalDecodeUri => Some(GLOBAL_DECODE_URI_FUNCTION_LENGTH),
            Self::GlobalDecodeUriComponent => Some(GLOBAL_DECODE_URI_COMPONENT_FUNCTION_LENGTH),
            Self::GlobalEncodeUri => Some(GLOBAL_ENCODE_URI_FUNCTION_LENGTH),
            Self::GlobalEncodeUriComponent => Some(GLOBAL_ENCODE_URI_COMPONENT_FUNCTION_LENGTH),
            Self::GlobalIsFinite => Some(GLOBAL_IS_FINITE_FUNCTION_LENGTH),
            Self::GlobalIsNan => Some(GLOBAL_IS_NAN_FUNCTION_LENGTH),
            Self::GlobalParseFloat => Some(GLOBAL_PARSE_FLOAT_FUNCTION_LENGTH),
            Self::GlobalParseInt => Some(GLOBAL_PARSE_INT_FUNCTION_LENGTH),
            Self::NumberIsFinite => Some(NUMBER_IS_FINITE_FUNCTION_LENGTH),
            Self::NumberIsInteger => Some(NUMBER_IS_INTEGER_FUNCTION_LENGTH),
            Self::NumberIsNan => Some(NUMBER_IS_NAN_FUNCTION_LENGTH),
            Self::NumberIsSafeInteger => Some(NUMBER_IS_SAFE_INTEGER_FUNCTION_LENGTH),
            _ => None,
        }
    }

    const fn math_length(self) -> Option<f64> {
        match self {
            Self::MathRandom => Some(MATH_FUNCTION_LENGTH_ZERO),
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
            | Self::MathF16round
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
            | Self::MathSumPrecise
            | Self::MathTan
            | Self::MathTanh
            | Self::MathTrunc => Some(MATH_FUNCTION_LENGTH_ONE),
            Self::MathAtan2
            | Self::MathHypot
            | Self::MathImul
            | Self::MathMax
            | Self::MathMin
            | Self::MathPow => Some(MATH_FUNCTION_LENGTH_TWO),
            _ => None,
        }
    }

    const fn core_length(self) -> Option<f64> {
        match self {
            Self::AsyncFunction => Some(ASYNC_FUNCTION_FUNCTION_LENGTH),
            Self::Boolean => Some(BOOLEAN_FUNCTION_LENGTH),
            Self::BoundFunction(_) => Some(BOUND_FUNCTION_LENGTH),
            Self::Eval => Some(EVAL_FUNCTION_LENGTH),
            Self::ErrorConstructor(name) => Some(name.constructor_length()),
            Self::ErrorPrototypeToString => Some(ERROR_PROTOTYPE_TO_STRING_LENGTH),
            Self::Function => Some(FUNCTION_FUNCTION_LENGTH),
            Self::FunctionPrototypeBind => Some(FUNCTION_PROTOTYPE_BIND_LENGTH),
            Self::FunctionPrototypeCall => Some(FUNCTION_PROTOTYPE_CALL_LENGTH),
            Self::FunctionPrototypeApply => Some(FUNCTION_PROTOTYPE_APPLY_LENGTH),
            Self::FunctionPrototypeHasInstance => Some(FUNCTION_PROTOTYPE_HAS_INSTANCE_LENGTH),
            Self::JsonIsRawJson => Some(JSON_IS_RAW_JSON_FUNCTION_LENGTH),
            Self::JsonParse => Some(JSON_PARSE_FUNCTION_LENGTH),
            Self::JsonRawJson => Some(JSON_RAW_JSON_FUNCTION_LENGTH),
            Self::JsonStringify => Some(JSON_STRINGIFY_FUNCTION_LENGTH),
            Self::Number => Some(NUMBER_FUNCTION_LENGTH),
            Self::Promise => Some(PROMISE_FUNCTION_LENGTH),
            Self::PromiseResolve => Some(PROMISE_RESOLVE_FUNCTION_LENGTH),
            Self::PromiseReject => Some(PROMISE_REJECT_FUNCTION_LENGTH),
            Self::PromiseThen => Some(PROMISE_THEN_FUNCTION_LENGTH),
            Self::PromiseCatch => Some(PROMISE_CATCH_FUNCTION_LENGTH),
            Self::PromiseResolver { .. } => Some(PROMISE_RESOLVER_FUNCTION_LENGTH),
            Self::String => Some(STRING_FUNCTION_LENGTH),
            Self::Symbol => Some(SYMBOL_FUNCTION_LENGTH),
            _ => None,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        if let Self::Date(kind) = self {
            return kind.name();
        }
        if let Some(name) = self.collection_name() {
            return name;
        }
        if let Some(name) = self.array_name() {
            return name;
        }
        if let Some(name) = self.global_utility_name() {
            return name;
        }
        if let Some(name) = self.math_name() {
            return name;
        }
        if let Some(name) = self.object_name() {
            return name;
        }
        if let Some(name) = self.core_name() {
            return name;
        }
        if let Some(name) = self.primitive_prototype_name() {
            return name;
        }
        if let Some(name) = self.string_static_name() {
            return name;
        }
        if let Some(name) = self.string_prototype_name() {
            return name;
        }
        if let Some(name) = self.reflect_name() {
            return name;
        }
        if let Some(name) = self.regexp_name() {
            return name;
        }
        FUNCTION_NAME
    }

    const fn math_name(self) -> Option<&'static str> {
        match self {
            Self::MathAbs => Some(MATH_ABS_NAME),
            Self::MathAcos => Some(MATH_ACOS_NAME),
            Self::MathAcosh => Some(MATH_ACOSH_NAME),
            Self::MathAsin => Some(MATH_ASIN_NAME),
            Self::MathAsinh => Some(MATH_ASINH_NAME),
            Self::MathAtan => Some(MATH_ATAN_NAME),
            Self::MathAtan2 => Some(MATH_ATAN2_NAME),
            Self::MathAtanh => Some(MATH_ATANH_NAME),
            Self::MathCbrt => Some(MATH_CBRT_NAME),
            Self::MathCeil => Some(MATH_CEIL_NAME),
            Self::MathClz32 => Some(MATH_CLZ32_NAME),
            Self::MathCos => Some(MATH_COS_NAME),
            Self::MathCosh => Some(MATH_COSH_NAME),
            Self::MathExp => Some(MATH_EXP_NAME),
            Self::MathExpm1 => Some(MATH_EXPM1_NAME),
            Self::MathF16round => Some(MATH_F16ROUND_NAME),
            Self::MathFloor => Some(MATH_FLOOR_NAME),
            Self::MathFround => Some(MATH_FROUND_NAME),
            Self::MathHypot => Some(MATH_HYPOT_NAME),
            Self::MathImul => Some(MATH_IMUL_NAME),
            Self::MathLog => Some(MATH_LOG_NAME),
            Self::MathLog10 => Some(MATH_LOG10_NAME),
            Self::MathLog1p => Some(MATH_LOG1P_NAME),
            Self::MathLog2 => Some(MATH_LOG2_NAME),
            Self::MathMax => Some(MATH_MAX_NAME),
            Self::MathMin => Some(MATH_MIN_NAME),
            Self::MathPow => Some(MATH_POW_NAME),
            Self::MathRandom => Some(MATH_RANDOM_NAME),
            Self::MathRound => Some(MATH_ROUND_NAME),
            Self::MathSign => Some(MATH_SIGN_NAME),
            Self::MathSin => Some(MATH_SIN_NAME),
            Self::MathSinh => Some(MATH_SINH_NAME),
            Self::MathSqrt => Some(MATH_SQRT_NAME),
            Self::MathSumPrecise => Some(MATH_SUM_PRECISE_NAME),
            Self::MathTan => Some(MATH_TAN_NAME),
            Self::MathTanh => Some(MATH_TANH_NAME),
            Self::MathTrunc => Some(MATH_TRUNC_NAME),
            _ => None,
        }
    }

    const fn object_name(self) -> Option<&'static str> {
        match self {
            Self::Object => Some(OBJECT_NAME),
            Self::ObjectAssign => Some(OBJECT_ASSIGN_NAME),
            Self::ObjectCreate => Some(OBJECT_CREATE_NAME),
            Self::ObjectDefineProperties => Some(OBJECT_DEFINE_PROPERTIES_NAME),
            Self::ObjectDefineProperty => Some(OBJECT_DEFINE_PROPERTY_NAME),
            Self::ObjectEntries => Some(OBJECT_ENTRIES_NAME),
            Self::ObjectFreeze => Some(OBJECT_FREEZE_NAME),
            Self::ObjectGetPrototypeOf => Some(OBJECT_GET_PROTOTYPE_OF_NAME),
            Self::ObjectGetOwnPropertyDescriptor => Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME),
            Self::ObjectGetOwnPropertyDescriptors => Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_NAME),
            Self::ObjectGetOwnPropertyNames => Some(OBJECT_GET_OWN_PROPERTY_NAMES_NAME),
            Self::ObjectHasOwn => Some(OBJECT_HAS_OWN_NAME),
            Self::ObjectIs => Some(OBJECT_IS_NAME),
            Self::ObjectIsExtensible => Some(OBJECT_IS_EXTENSIBLE_NAME),
            Self::ObjectIsFrozen => Some(OBJECT_IS_FROZEN_NAME),
            Self::ObjectIsSealed => Some(OBJECT_IS_SEALED_NAME),
            Self::ObjectKeys => Some(OBJECT_KEYS_NAME),
            Self::ObjectPreventExtensions => Some(OBJECT_PREVENT_EXTENSIONS_NAME),
            Self::ObjectPrototypeHasOwnProperty => Some(OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME),
            Self::ObjectPrototypePropertyIsEnumerable => {
                Some(OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME)
            }
            Self::ObjectPrototypeToString => Some(OBJECT_PROTOTYPE_TO_STRING_NAME),
            Self::ObjectPrototypeValueOf => Some(OBJECT_PROTOTYPE_VALUE_OF_NAME),
            Self::ObjectPrototypeToLocaleString => Some(OBJECT_PROTOTYPE_TO_LOCALE_STRING_NAME),
            Self::ObjectPrototypeIsPrototypeOf => Some(OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_NAME),
            Self::ObjectFromEntries => Some(OBJECT_FROM_ENTRIES_NAME),
            Self::ObjectSetPrototypeOf => Some(OBJECT_SET_PROTOTYPE_OF_NAME),
            Self::ObjectSeal => Some(OBJECT_SEAL_NAME),
            Self::ObjectValues => Some(OBJECT_VALUES_NAME),
            _ => None,
        }
    }

    const fn core_name(self) -> Option<&'static str> {
        match self {
            Self::AsyncFunction => Some(ASYNC_FUNCTION_NAME),
            Self::Boolean => Some(BOOLEAN_NAME),
            Self::BoundFunction(_) => Some(BOUND_FUNCTION_NAME),
            Self::Eval => Some(EVAL_NAME),
            Self::ErrorConstructor(name) => Some(name.as_str()),
            Self::ErrorPrototypeToString => Some(ERROR_PROTOTYPE_TO_STRING_NAME),
            Self::Function => Some(FUNCTION_NAME),
            Self::FunctionPrototypeBind => Some(FUNCTION_PROTOTYPE_BIND_NAME),
            Self::FunctionPrototypeCall => Some(FUNCTION_PROTOTYPE_CALL_NAME),
            Self::FunctionPrototypeApply => Some(FUNCTION_PROTOTYPE_APPLY_NAME),
            Self::FunctionPrototypeHasInstance => Some(FUNCTION_PROTOTYPE_HAS_INSTANCE_NAME),
            Self::JsonIsRawJson => Some(JSON_IS_RAW_JSON_NAME),
            Self::JsonParse => Some(JSON_PARSE_NAME),
            Self::JsonRawJson => Some(JSON_RAW_JSON_NAME),
            Self::JsonStringify => Some(JSON_STRINGIFY_NAME),
            Self::Number => Some(NUMBER_NAME),
            Self::Promise => Some(PROMISE_NAME),
            Self::PromiseResolve => Some(PROMISE_RESOLVE_NAME),
            Self::PromiseReject => Some(PROMISE_REJECT_NAME),
            Self::PromiseThen => Some(PROMISE_THEN_NAME),
            Self::PromiseCatch => Some(PROMISE_CATCH_NAME),
            Self::PromiseResolver {
                kind: crate::runtime::promise::PromiseResolverKind::Resolve,
                ..
            } => Some(RESOLVE_NAME),
            Self::PromiseResolver {
                kind: crate::runtime::promise::PromiseResolverKind::Reject,
                ..
            } => Some(REJECT_NAME),
            Self::String => Some(STRING_NAME),
            Self::Symbol => Some(SYMBOL_NAME),
            _ => None,
        }
    }

    const fn global_utility_name(self) -> Option<&'static str> {
        match self {
            Self::GlobalDecodeUri => Some(GLOBAL_DECODE_URI_NAME),
            Self::GlobalDecodeUriComponent => Some(GLOBAL_DECODE_URI_COMPONENT_NAME),
            Self::GlobalEncodeUri => Some(GLOBAL_ENCODE_URI_NAME),
            Self::GlobalEncodeUriComponent => Some(GLOBAL_ENCODE_URI_COMPONENT_NAME),
            Self::GlobalIsFinite | Self::NumberIsFinite => Some(GLOBAL_IS_FINITE_NAME),
            Self::GlobalIsNan | Self::NumberIsNan => Some(GLOBAL_IS_NAN_NAME),
            Self::NumberIsInteger => Some(NUMBER_IS_INTEGER_NAME),
            Self::NumberIsSafeInteger => Some(NUMBER_IS_SAFE_INTEGER_NAME),
            Self::GlobalParseFloat => Some(GLOBAL_PARSE_FLOAT_NAME),
            Self::GlobalParseInt => Some(GLOBAL_PARSE_INT_NAME),
            _ => None,
        }
    }
}
