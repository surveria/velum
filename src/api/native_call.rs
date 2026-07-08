use crate::value::ErrorName;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum NativeCallTarget {
    Array,
    ArrayConcat,
    ArrayIncludes,
    ArrayIndexOf,
    ArrayIsArray,
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
    ErrorPrototypeToString,
    Eval,
    Function,
    FunctionPrototypeBind,
    FunctionPrototypeCall,
    GlobalDecodeUri,
    GlobalDecodeUriComponent,
    GlobalEncodeUri,
    GlobalEncodeUriComponent,
    GlobalIsFinite,
    GlobalIsNan,
    GlobalParseFloat,
    GlobalParseInt,
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
    NumberIsFinite,
    NumberIsNan,
    Object,
    ObjectAssign,
    ObjectCreate,
    ObjectDefineProperties,
    ObjectDefineProperty,
    ObjectEntries,
    ObjectGetOwnPropertyDescriptor,
    ObjectGetOwnPropertyDescriptors,
    ObjectGetOwnPropertyNames,
    ObjectGetPrototypeOf,
    ObjectHasOwn,
    ObjectIs,
    ObjectKeys,
    ObjectSetPrototypeOf,
    ObjectValues,
    Promise,
    PromiseResolve,
    PromiseReject,
    PromiseThen,
    PromiseCatch,
    RegExp,
    RegExpPrototypeTest,
    String,
    StringFromCharCode,
    StringFromCodePoint,
    StringRaw,
    StringPrototypeAt,
    StringPrototypeCharAt,
    StringPrototypeCharCodeAt,
    StringPrototypeCodePointAt,
    StringPrototypeConcat,
    StringPrototypeEndsWith,
    StringPrototypeIncludes,
    StringPrototypeIndexOf,
    StringPrototypeLastIndexOf,
    StringPrototypePadEnd,
    StringPrototypePadStart,
    StringPrototypeRepeat,
    StringPrototypeSlice,
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
    Symbol,
}

impl NativeCallTarget {
    pub const fn is_array_target(self) -> bool {
        matches!(
            self,
            Self::Array
                | Self::ArrayConcat
                | Self::ArrayIncludes
                | Self::ArrayIndexOf
                | Self::ArrayIsArray
                | Self::ArrayJoin
                | Self::ArrayLastIndexOf
                | Self::ArrayPop
                | Self::ArrayPush
                | Self::ArrayReverse
                | Self::ArrayShift
                | Self::ArraySlice
                | Self::ArrayUnshift
        )
    }

    pub fn from_binding_name(name: &str) -> Option<Self> {
        match name {
            "Array" => Some(Self::Array),
            "Boolean" => Some(Self::Boolean),
            "eval" => Some(Self::Eval),
            "Function" => Some(Self::Function),
            "decodeURI" => Some(Self::GlobalDecodeUri),
            "decodeURIComponent" => Some(Self::GlobalDecodeUriComponent),
            "encodeURI" => Some(Self::GlobalEncodeUri),
            "encodeURIComponent" => Some(Self::GlobalEncodeUriComponent),
            "isFinite" => Some(Self::GlobalIsFinite),
            "isNaN" => Some(Self::GlobalIsNan),
            "Number" => Some(Self::Number),
            "Object" => Some(Self::Object),
            "parseFloat" => Some(Self::GlobalParseFloat),
            "parseInt" => Some(Self::GlobalParseInt),
            "Promise" => Some(Self::Promise),
            "RegExp" => Some(Self::RegExp),
            "String" => Some(Self::String),
            "Symbol" => Some(Self::Symbol),
            _ => ErrorName::from_constructor_name(name)
                .filter(|name| name.is_standard())
                .map(Self::ErrorConstructor),
        }
    }

    pub fn from_property_name(name: &str) -> Option<Self> {
        Self::from_array_property_name(name)
            .or_else(|| Self::from_string_property_name(name))
            .or_else(|| Self::from_math_property_name(name))
            .or_else(|| Self::from_object_property_name(name))
            .or_else(|| Self::from_core_property_name(name))
    }

    fn from_array_property_name(name: &str) -> Option<Self> {
        match name {
            "concat" => Some(Self::ArrayConcat),
            "includes" => Some(Self::ArrayIncludes),
            "indexOf" => Some(Self::ArrayIndexOf),
            "isArray" => Some(Self::ArrayIsArray),
            "join" => Some(Self::ArrayJoin),
            "lastIndexOf" => Some(Self::ArrayLastIndexOf),
            "pop" => Some(Self::ArrayPop),
            "push" => Some(Self::ArrayPush),
            "reverse" => Some(Self::ArrayReverse),
            "shift" => Some(Self::ArrayShift),
            "slice" => Some(Self::ArraySlice),
            "unshift" => Some(Self::ArrayUnshift),
            _ => None,
        }
    }

    fn from_string_property_name(name: &str) -> Option<Self> {
        match name {
            "at" => Some(Self::StringPrototypeAt),
            "charAt" => Some(Self::StringPrototypeCharAt),
            "charCodeAt" => Some(Self::StringPrototypeCharCodeAt),
            "codePointAt" => Some(Self::StringPrototypeCodePointAt),
            "endsWith" => Some(Self::StringPrototypeEndsWith),
            "fromCharCode" => Some(Self::StringFromCharCode),
            "fromCodePoint" => Some(Self::StringFromCodePoint),
            "padEnd" => Some(Self::StringPrototypePadEnd),
            "padStart" => Some(Self::StringPrototypePadStart),
            "raw" => Some(Self::StringRaw),
            "repeat" => Some(Self::StringPrototypeRepeat),
            "startsWith" => Some(Self::StringPrototypeStartsWith),
            "substring" => Some(Self::StringPrototypeSubstring),
            "toLocaleLowerCase" => Some(Self::StringPrototypeToLocaleLowerCase),
            "toLocaleUpperCase" => Some(Self::StringPrototypeToLocaleUpperCase),
            "toLowerCase" => Some(Self::StringPrototypeToLowerCase),
            "toUpperCase" => Some(Self::StringPrototypeToUpperCase),
            "trim" => Some(Self::StringPrototypeTrim),
            "trimEnd" | "trimRight" => Some(Self::StringPrototypeTrimEnd),
            "trimLeft" | "trimStart" => Some(Self::StringPrototypeTrimStart),
            "valueOf" => Some(Self::StringPrototypeValueOf),
            _ => None,
        }
    }

    fn from_math_property_name(name: &str) -> Option<Self> {
        match name {
            "abs" => Some(Self::MathAbs),
            "acos" => Some(Self::MathAcos),
            "acosh" => Some(Self::MathAcosh),
            "asin" => Some(Self::MathAsin),
            "asinh" => Some(Self::MathAsinh),
            "atan" => Some(Self::MathAtan),
            "atan2" => Some(Self::MathAtan2),
            "atanh" => Some(Self::MathAtanh),
            "cbrt" => Some(Self::MathCbrt),
            "ceil" => Some(Self::MathCeil),
            "clz32" => Some(Self::MathClz32),
            "cos" => Some(Self::MathCos),
            "cosh" => Some(Self::MathCosh),
            "exp" => Some(Self::MathExp),
            "expm1" => Some(Self::MathExpm1),
            "floor" => Some(Self::MathFloor),
            "fround" => Some(Self::MathFround),
            "hypot" => Some(Self::MathHypot),
            "imul" => Some(Self::MathImul),
            "log" => Some(Self::MathLog),
            "log10" => Some(Self::MathLog10),
            "log1p" => Some(Self::MathLog1p),
            "log2" => Some(Self::MathLog2),
            "max" => Some(Self::MathMax),
            "min" => Some(Self::MathMin),
            "pow" => Some(Self::MathPow),
            "random" => Some(Self::MathRandom),
            "round" => Some(Self::MathRound),
            "sign" => Some(Self::MathSign),
            "sin" => Some(Self::MathSin),
            "sinh" => Some(Self::MathSinh),
            "sqrt" => Some(Self::MathSqrt),
            "tan" => Some(Self::MathTan),
            "tanh" => Some(Self::MathTanh),
            "trunc" => Some(Self::MathTrunc),
            _ => None,
        }
    }

    fn from_object_property_name(name: &str) -> Option<Self> {
        match name {
            "assign" => Some(Self::ObjectAssign),
            "create" => Some(Self::ObjectCreate),
            "defineProperties" => Some(Self::ObjectDefineProperties),
            "defineProperty" => Some(Self::ObjectDefineProperty),
            "entries" => Some(Self::ObjectEntries),
            "getOwnPropertyDescriptor" => Some(Self::ObjectGetOwnPropertyDescriptor),
            "getOwnPropertyDescriptors" => Some(Self::ObjectGetOwnPropertyDescriptors),
            "getOwnPropertyNames" => Some(Self::ObjectGetOwnPropertyNames),
            "getPrototypeOf" => Some(Self::ObjectGetPrototypeOf),
            "hasOwn" => Some(Self::ObjectHasOwn),
            "is" => Some(Self::ObjectIs),
            "keys" => Some(Self::ObjectKeys),
            "setPrototypeOf" => Some(Self::ObjectSetPrototypeOf),
            "values" => Some(Self::ObjectValues),
            _ => None,
        }
    }

    fn from_core_property_name(name: &str) -> Option<Self> {
        match name {
            "bind" => Some(Self::FunctionPrototypeBind),
            "call" => Some(Self::FunctionPrototypeCall),
            "catch" => Some(Self::PromiseCatch),
            "isFinite" => Some(Self::NumberIsFinite),
            "isNaN" => Some(Self::NumberIsNan),
            "parse" => Some(Self::JsonParse),
            "parseFloat" => Some(Self::GlobalParseFloat),
            "parseInt" => Some(Self::GlobalParseInt),
            "reject" => Some(Self::PromiseReject),
            "resolve" => Some(Self::PromiseResolve),
            "stringify" => Some(Self::JsonStringify),
            "test" => Some(Self::RegExpPrototypeTest),
            "then" => Some(Self::PromiseThen),
            "toString" => Some(Self::ErrorPrototypeToString),
            _ => None,
        }
    }
}
