use super::{
    AtomicsFunctionKind, array_buffer_kind::ArrayBufferFunctionKind,
    data_view_kind::DataViewFunctionKind, date_kind::DateFunctionKind,
    iterator_kind::IteratorFunctionKind, shadow_realm_kind::ShadowRealmFunctionKind,
    shared_array_buffer_kind::SharedArrayBufferFunctionKind,
    typed_array_kind::TypedArrayFunctionKind,
};
use crate::runtime::{
    object::TypedArrayElementKind,
    promise::{PromiseCombinatorElementKind, PromiseCombinatorKind, PromiseFinallyFunctionKind},
};
use crate::value::{BoundFunctionId, ErrorName, ObjectId};

mod core;
mod math;
mod promise;
mod regexp;
mod string;
mod utility;

pub(in crate::runtime::native) use promise::{
    PROMISE_CATCH_NAME, PROMISE_FINALLY_NAME, PROMISE_NAME, PROMISE_REJECT_NAME,
    PROMISE_RESOLVE_NAME, PROMISE_THEN_NAME,
};

pub(in crate::runtime::native) use regexp::{
    REGEXP_NAME, REGEXP_PROTOTYPE_EXEC_NAME, REGEXP_PROTOTYPE_TEST_NAME,
    REGEXP_PROTOTYPE_TO_STRING_NAME,
};

const ARRAY_BUFFER_FUNCTION_LENGTH: f64 = 1.0;
const ASYNC_FUNCTION_FUNCTION_LENGTH: f64 = 1.0;
const ASYNC_GENERATOR_FUNCTION_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const ARRAY_BUFFER_NAME: &str = "ArrayBuffer";
pub(in crate::runtime::native) const ASYNC_FUNCTION_NAME: &str = "AsyncFunction";
pub(in crate::runtime::native) const ASYNC_GENERATOR_FUNCTION_NAME: &str = "AsyncGeneratorFunction";
const BOOLEAN_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const BOOLEAN_NAME: &str = "Boolean";
const BIGINT_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const BIGINT_NAME: &str = "BigInt";
pub(in crate::runtime::native) const BIGINT_AS_INT_N_NAME: &str = "asIntN";
pub(in crate::runtime::native) const BIGINT_AS_UINT_N_NAME: &str = "asUintN";
const EVAL_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const EVAL_NAME: &str = "eval";
const ERROR_PROTOTYPE_TO_STRING_LENGTH: f64 = 0.0;
pub(in crate::runtime::native) const ERROR_PROTOTYPE_TO_STRING_NAME: &str = "toString";
const FUNCTION_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const FUNCTION_NAME: &str = "Function";
const GENERATOR_FUNCTION_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const GENERATOR_FUNCTION_NAME: &str = "GeneratorFunction";
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
const FUNCTION_PROTOTYPE_TO_STRING_LENGTH: f64 = 0.0;
pub(in crate::runtime::native) const FUNCTION_PROTOTYPE_TO_STRING_NAME: &str = "toString";
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
pub(in crate::runtime) const UNDEFINED_NAME: &str = "undefined";
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
const OBJECT_GROUP_BY_NAME: &str = "groupBy";
pub(in crate::runtime::native) const OBJECT_GET_PROTOTYPE_OF_NAME: &str = "getPrototypeOf";
pub(in crate::runtime::native) const OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME: &str =
    "getOwnPropertyDescriptor";
pub(in crate::runtime::native) const OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_NAME: &str =
    "getOwnPropertyDescriptors";
pub(in crate::runtime::native) const OBJECT_GET_OWN_PROPERTY_NAMES_NAME: &str =
    "getOwnPropertyNames";
pub(in crate::runtime::native) const OBJECT_GET_OWN_PROPERTY_SYMBOLS_NAME: &str =
    "getOwnPropertySymbols";
pub(in crate::runtime::native) const OBJECT_HAS_OWN_NAME: &str = "hasOwn";
pub(in crate::runtime::native) const OBJECT_IS_NAME: &str = "is";
pub(in crate::runtime::native) const OBJECT_IS_EXTENSIBLE_NAME: &str = "isExtensible";
pub(in crate::runtime::native) const OBJECT_IS_FROZEN_NAME: &str = "isFrozen";
pub(in crate::runtime::native) const OBJECT_IS_SEALED_NAME: &str = "isSealed";
pub(in crate::runtime::native) const OBJECT_KEYS_NAME: &str = "keys";
pub(in crate::runtime::native) const OBJECT_NAME: &str = "Object";
pub(in crate::runtime::native) const OBJECT_PREVENT_EXTENSIONS_NAME: &str = "preventExtensions";
pub(in crate::runtime) const OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME: &str = "hasOwnProperty";
pub(in crate::runtime) const OBJECT_PROTOTYPE_DEFINE_GETTER_NAME: &str = "__defineGetter__";
pub(in crate::runtime) const OBJECT_PROTOTYPE_DEFINE_SETTER_NAME: &str = "__defineSetter__";
pub(in crate::runtime) const OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME: &str =
    "propertyIsEnumerable";
pub(in crate::runtime) const OBJECT_PROTOTYPE_LOOKUP_GETTER_NAME: &str = "__lookupGetter__";
pub(in crate::runtime) const OBJECT_PROTOTYPE_LOOKUP_SETTER_NAME: &str = "__lookupSetter__";
pub(in crate::runtime) const OBJECT_PROTOTYPE_PROTO_GETTER_NAME: &str = "get __proto__";
pub(in crate::runtime) const OBJECT_PROTOTYPE_PROTO_SETTER_NAME: &str = "set __proto__";
pub(in crate::runtime) const OBJECT_PROTOTYPE_TO_STRING_NAME: &str = "toString";
pub(in crate::runtime) const OBJECT_PROTOTYPE_VALUE_OF_NAME: &str = "valueOf";
pub(in crate::runtime) const OBJECT_PROTOTYPE_TO_LOCALE_STRING_NAME: &str = "toLocaleString";
pub(in crate::runtime) const OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_NAME: &str = "isPrototypeOf";
pub(in crate::runtime::native) const OBJECT_FROM_ENTRIES_NAME: &str = "fromEntries";
pub(in crate::runtime::native) const OBJECT_SET_PROTOTYPE_OF_NAME: &str = "setPrototypeOf";
pub(in crate::runtime::native) const OBJECT_SEAL_NAME: &str = "seal";
pub(in crate::runtime::native) const OBJECT_VALUES_NAME: &str = "values";
pub(in crate::runtime::native) const PROXY_NAME: &str = "Proxy";
const PROXY_FUNCTION_LENGTH: f64 = 2.0;
pub(in crate::runtime::native) const PROXY_REVOCABLE_NAME: &str = "revocable";
const PROXY_REVOCABLE_FUNCTION_LENGTH: f64 = 2.0;
const PROXY_REVOKE_NAME: &str = "";
const PROXY_REVOKE_FUNCTION_LENGTH: f64 = 0.0;
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
pub(in crate::runtime::native) const STRING_PROTOTYPE_ITERATOR_NAME: &str = "[Symbol.iterator]";
pub(in crate::runtime::native) const STRING_PROTOTYPE_MATCH_NAME: &str = "match";
pub(in crate::runtime::native) const STRING_PROTOTYPE_MATCH_ALL_NAME: &str = "matchAll";
pub(in crate::runtime::native) const STRING_PROTOTYPE_IS_WELL_FORMED_NAME: &str = "isWellFormed";
pub(in crate::runtime::native) const STRING_PROTOTYPE_REPEAT_NAME: &str = "repeat";
pub(in crate::runtime::native) const STRING_PROTOTYPE_REPLACE_NAME: &str = "replace";
pub(in crate::runtime::native) const STRING_PROTOTYPE_REPLACE_ALL_NAME: &str = "replaceAll";
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
pub(in crate::runtime::native) const STRING_PROTOTYPE_TO_WELL_FORMED_NAME: &str = "toWellFormed";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TO_STRING_NAME: &str = "toString";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TO_UPPER_CASE_NAME: &str = "toUpperCase";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_NAME: &str = "trim";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_END_NAME: &str = "trimEnd";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_LEFT_NAME: &str = "trimLeft";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_RIGHT_NAME: &str = "trimRight";
pub(in crate::runtime::native) const STRING_PROTOTYPE_TRIM_START_NAME: &str = "trimStart";
pub(in crate::runtime::native) const STRING_PROTOTYPE_VALUE_OF_NAME: &str = "valueOf";
const SYMBOL_FUNCTION_LENGTH: f64 = 0.0;
const SYMBOL_FOR_FUNCTION_LENGTH: f64 = 1.0;
const SYMBOL_KEY_FOR_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const SYMBOL_NAME: &str = "Symbol";
const SPECIES_GETTER_FUNCTION_LENGTH: f64 = 0.0;
const SPECIES_GETTER_NAME: &str = "get [Symbol.species]";
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
    ArrayFrom,
    ArrayFromAsync,
    ArrayIncludes,
    ArrayIndexOf,
    ArrayIsArray,
    ArrayOf,
    ArrayJoin,
    ArrayToString,
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
    ArrayEntries,
    ArrayKeys,
    ArrayValues,
    ArrayIteratorNext,
    ArrayBuffer,
    ArrayBufferPrototype(ArrayBufferFunctionKind),
    Atomics(AtomicsFunctionKind),
    SharedArrayBuffer,
    SharedArrayBufferPrototype(SharedArrayBufferFunctionKind),
    ShadowRealm(ShadowRealmFunctionKind),
    AsyncFunction,
    AsyncGeneratorFunction,
    AsyncGeneratorNext,
    AsyncGeneratorReturn,
    AsyncGeneratorThrow,
    AsyncDisposableStack(super::async_disposable_stack_kind::AsyncDisposableStackFunctionKind),
    Boolean,
    BooleanPrototypeToString,
    BooleanPrototypeValueOf,
    BigInt,
    BigIntAsIntN,
    BigIntAsUintN,
    BigIntPrototypeToLocaleString,
    BigIntPrototypeToString,
    BigIntPrototypeValueOf,
    BoundFunction(BoundFunctionId),
    DataView(DataViewFunctionKind),
    Date(DateFunctionKind),
    DisposableStack(super::disposable_stack_kind::DisposableStackFunctionKind),
    CollectionIteratorNext(crate::runtime::collections::CollectionIteratorId),
    Iterator(IteratorFunctionKind),
    IteratorSelf,
    Eval,
    ErrorConstructor(ErrorName),
    ErrorPrototypeToString,
    Function,
    GeneratorFunction,
    FunctionPrototypeBind,
    FunctionPrototypeCall,
    FunctionPrototypeApply,
    FunctionPrototypeHasInstance,
    FunctionPrototypeToString,
    GeneratorNext,
    GeneratorReturn,
    GeneratorThrow,
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
    MapGetOrInsert,
    MapGetOrInsertComputed,
    MapGroupBy,
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
    ObjectGroupBy,
    ObjectGetPrototypeOf,
    ObjectGetOwnPropertyDescriptor,
    ObjectGetOwnPropertyDescriptors,
    ObjectGetOwnPropertyNames,
    ObjectGetOwnPropertySymbols,
    ObjectHasOwn,
    ObjectIs,
    ObjectIsExtensible,
    ObjectIsFrozen,
    ObjectIsSealed,
    ObjectKeys,
    ObjectPreventExtensions,
    ObjectPrototypeDefineGetter,
    ObjectPrototypeDefineSetter,
    ObjectPrototypeHasOwnProperty,
    ObjectPrototypeLookupGetter,
    ObjectPrototypeLookupSetter,
    ObjectPrototypePropertyIsEnumerable,
    ObjectPrototypeProtoGetter,
    ObjectPrototypeProtoSetter,
    ObjectPrototypeToString,
    ObjectPrototypeValueOf,
    ObjectPrototypeToLocaleString,
    ObjectPrototypeIsPrototypeOf,
    ObjectFromEntries,
    ObjectSetPrototypeOf,
    ObjectSeal,
    ObjectValues,
    PerformanceNow,
    Print,
    Promise,
    PromiseCombinator(PromiseCombinatorKind),
    PromiseCombinatorElement {
        state: ObjectId,
        index: usize,
        kind: PromiseCombinatorElementKind,
    },
    PromiseCapabilityExecutor {
        capability_state: ObjectId,
    },
    PromiseResolve,
    PromiseReject,
    PromiseThen,
    PromiseCatch,
    PromiseFinally,
    PromiseFinallyFunction {
        state: ObjectId,
        kind: PromiseFinallyFunctionKind,
    },
    PromiseResolver {
        promise: crate::runtime::promise::PromiseId,
        kind: crate::runtime::promise::PromiseResolverKind,
    },
    Proxy,
    ProxyRevocable,
    ProxyRevoke(ObjectId),
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
    RegExpEscape,
    RegExpPrototypeCompile,
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
    RegExpPrototypeSymbolMatch,
    RegExpPrototypeSymbolMatchAll,
    RegExpPrototypeSymbolReplace,
    RegExpPrototypeSymbolSearch,
    RegExpPrototypeSymbolSplit,
    String,
    StringFromCharCode,
    StringFromCodePoint,
    StringPrototypeCharAt,
    StringPrototypeCharCodeAt,
    StringPrototypeAt,
    StringPrototypeAnnexB(super::string_annexb_kind::StringAnnexBFunctionKind),
    StringPrototypeCodePointAt,
    StringPrototypeConcat,
    StringPrototypeEndsWith,
    StringPrototypeIncludes,
    StringPrototypeIndexOf,
    StringPrototypeLastIndexOf,
    StringPrototypeIterator,
    StringPrototypeMatch,
    StringPrototypeMatchAll,
    StringPrototypeIsWellFormed,
    StringPrototypePadEnd,
    StringPrototypePadStart,
    StringPrototypeRepeat,
    StringPrototypeReplace,
    StringPrototypeReplaceAll,
    StringPrototypeSearch,
    StringPrototypeSlice,
    StringPrototypeSplit,
    StringPrototypeStartsWith,
    StringPrototypeSubstring,
    StringPrototypeToLocaleLowerCase,
    StringPrototypeToLocaleUpperCase,
    StringPrototypeToLowerCase,
    StringPrototypeToWellFormed,
    StringPrototypeToString,
    StringPrototypeToUpperCase,
    StringPrototypeTrim,
    StringPrototypeTrimEnd,
    StringPrototypeTrimStart,
    StringPrototypeValueOf,
    StringIteratorNext,
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
    SpeciesGetter,
    Symbol,
    SymbolFor,
    SymbolKeyFor,
    SymbolPrototypeDescriptionGetter,
    SymbolPrototypeToPrimitive,
    SymbolPrototypeToString,
    SymbolPrototypeValueOf,
    ThrowTypeError,
    TypedArrayIntrinsic,
    TypedArrayPrototype(TypedArrayFunctionKind),
    TypedArray(TypedArrayElementKind),
    WeakMap,
    WeakMapDelete,
    WeakMapGet,
    WeakMapGetOrInsert,
    WeakMapGetOrInsertComputed,
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
        if let Self::DataView(kind) = self {
            return kind.length();
        }
        if let Self::DisposableStack(kind) = self {
            return kind.length();
        }
        if let Self::AsyncDisposableStack(kind) = self {
            return kind.length();
        }
        if let Self::ArrayBufferPrototype(kind) = self {
            return kind.length();
        }
        if let Self::Atomics(kind) = self {
            return kind.length();
        }
        if let Self::SharedArrayBufferPrototype(kind) = self {
            return kind.length();
        }
        if let Self::Iterator(kind) = self {
            return kind.length();
        }
        if let Self::TypedArrayPrototype(kind) = self {
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
        if let Some(length) = self.performance_length() {
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

    pub(in crate::runtime) const fn name(self) -> &'static str {
        if let Self::Date(kind) = self {
            return kind.name();
        }
        if let Self::DataView(kind) = self {
            return kind.name();
        }
        if let Self::DisposableStack(kind) = self {
            return kind.name();
        }
        if let Self::AsyncDisposableStack(kind) = self {
            return kind.name();
        }
        if let Self::ArrayBufferPrototype(kind) = self {
            return kind.name();
        }
        if let Self::Atomics(kind) = self {
            return kind.name();
        }
        if let Self::SharedArrayBufferPrototype(kind) = self {
            return kind.name();
        }
        if let Self::Iterator(kind) = self {
            return kind.name();
        }
        if let Self::TypedArrayPrototype(kind) = self {
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
        if let Some(name) = self.performance_name() {
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

    const fn object_name(self) -> Option<&'static str> {
        match self {
            Self::Object => Some(OBJECT_NAME),
            Self::ObjectAssign => Some(OBJECT_ASSIGN_NAME),
            Self::ObjectCreate => Some(OBJECT_CREATE_NAME),
            Self::ObjectDefineProperties => Some(OBJECT_DEFINE_PROPERTIES_NAME),
            Self::ObjectDefineProperty => Some(OBJECT_DEFINE_PROPERTY_NAME),
            Self::ObjectEntries => Some(OBJECT_ENTRIES_NAME),
            Self::ObjectFreeze => Some(OBJECT_FREEZE_NAME),
            Self::ObjectGroupBy => Some(OBJECT_GROUP_BY_NAME),
            Self::ObjectGetPrototypeOf => Some(OBJECT_GET_PROTOTYPE_OF_NAME),
            Self::ObjectGetOwnPropertyDescriptor => Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME),
            Self::ObjectGetOwnPropertyDescriptors => Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_NAME),
            Self::ObjectGetOwnPropertyNames => Some(OBJECT_GET_OWN_PROPERTY_NAMES_NAME),
            Self::ObjectGetOwnPropertySymbols => Some(OBJECT_GET_OWN_PROPERTY_SYMBOLS_NAME),
            Self::ObjectHasOwn => Some(OBJECT_HAS_OWN_NAME),
            Self::ObjectIs => Some(OBJECT_IS_NAME),
            Self::ObjectIsExtensible => Some(OBJECT_IS_EXTENSIBLE_NAME),
            Self::ObjectIsFrozen => Some(OBJECT_IS_FROZEN_NAME),
            Self::ObjectIsSealed => Some(OBJECT_IS_SEALED_NAME),
            Self::ObjectKeys => Some(OBJECT_KEYS_NAME),
            Self::ObjectPreventExtensions => Some(OBJECT_PREVENT_EXTENSIONS_NAME),
            Self::ObjectPrototypeDefineGetter => Some(OBJECT_PROTOTYPE_DEFINE_GETTER_NAME),
            Self::ObjectPrototypeDefineSetter => Some(OBJECT_PROTOTYPE_DEFINE_SETTER_NAME),
            Self::ObjectPrototypeHasOwnProperty => Some(OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME),
            Self::ObjectPrototypeLookupGetter => Some(OBJECT_PROTOTYPE_LOOKUP_GETTER_NAME),
            Self::ObjectPrototypeLookupSetter => Some(OBJECT_PROTOTYPE_LOOKUP_SETTER_NAME),
            Self::ObjectPrototypePropertyIsEnumerable => {
                Some(OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME)
            }
            Self::ObjectPrototypeProtoGetter => Some(OBJECT_PROTOTYPE_PROTO_GETTER_NAME),
            Self::ObjectPrototypeProtoSetter => Some(OBJECT_PROTOTYPE_PROTO_SETTER_NAME),
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
}
