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
    ObjectGetOwnPropertyDescriptor,
    ObjectGetPrototypeOf,
    ObjectHasOwn,
    ObjectKeys,
    Promise,
    PromiseResolve,
    PromiseReject,
    PromiseThen,
    PromiseCatch,
    RegExp,
    String,
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
            "Function" => Some(Self::Function),
            "Number" => Some(Self::Number),
            "Object" => Some(Self::Object),
            "Promise" => Some(Self::Promise),
            "RegExp" => Some(Self::RegExp),
            "String" => Some(Self::String),
            _ => ErrorName::from_constructor_name(name)
                .filter(|name| name.is_standard())
                .map(Self::ErrorConstructor),
        }
    }

    pub fn from_property_name(name: &str) -> Option<Self> {
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
            "concat" => Some(Self::ArrayConcat),
            "cos" => Some(Self::MathCos),
            "cosh" => Some(Self::MathCosh),
            "defineProperty" => Some(Self::ObjectDefineProperty),
            "exp" => Some(Self::MathExp),
            "expm1" => Some(Self::MathExpm1),
            "floor" => Some(Self::MathFloor),
            "fround" => Some(Self::MathFround),
            "getOwnPropertyDescriptor" => Some(Self::ObjectGetOwnPropertyDescriptor),
            "getPrototypeOf" => Some(Self::ObjectGetPrototypeOf),
            "hasOwn" => Some(Self::ObjectHasOwn),
            "hypot" => Some(Self::MathHypot),
            "imul" => Some(Self::MathImul),
            "includes" => Some(Self::ArrayIncludes),
            "indexOf" => Some(Self::ArrayIndexOf),
            "isArray" => Some(Self::ArrayIsArray),
            "join" => Some(Self::ArrayJoin),
            "keys" => Some(Self::ObjectKeys),
            "lastIndexOf" => Some(Self::ArrayLastIndexOf),
            "log" => Some(Self::MathLog),
            "log10" => Some(Self::MathLog10),
            "log1p" => Some(Self::MathLog1p),
            "log2" => Some(Self::MathLog2),
            "max" => Some(Self::MathMax),
            "min" => Some(Self::MathMin),
            "parse" => Some(Self::JsonParse),
            "pop" => Some(Self::ArrayPop),
            "pow" => Some(Self::MathPow),
            "reject" => Some(Self::PromiseReject),
            "push" => Some(Self::ArrayPush),
            "random" => Some(Self::MathRandom),
            "reverse" => Some(Self::ArrayReverse),
            "round" => Some(Self::MathRound),
            "then" => Some(Self::PromiseThen),
            "catch" => Some(Self::PromiseCatch),
            "resolve" => Some(Self::PromiseResolve),
            "shift" => Some(Self::ArrayShift),
            "sign" => Some(Self::MathSign),
            "sin" => Some(Self::MathSin),
            "sinh" => Some(Self::MathSinh),
            "slice" => Some(Self::ArraySlice),
            "sqrt" => Some(Self::MathSqrt),
            "stringify" => Some(Self::JsonStringify),
            "tan" => Some(Self::MathTan),
            "tanh" => Some(Self::MathTanh),
            "trunc" => Some(Self::MathTrunc),
            "unshift" => Some(Self::ArrayUnshift),
            _ => None,
        }
    }
}
