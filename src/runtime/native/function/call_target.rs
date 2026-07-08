use crate::api::native_call::NativeCallTarget;

use super::NativeFunctionKind;

impl NativeFunctionKind {
    pub(super) const fn from_call_target(target: NativeCallTarget) -> Self {
        match target {
            NativeCallTarget::Array => Self::Array,
            NativeCallTarget::ArrayConcat => Self::ArrayConcat,
            NativeCallTarget::ArrayIncludes => Self::ArrayIncludes,
            NativeCallTarget::ArrayIndexOf => Self::ArrayIndexOf,
            NativeCallTarget::ArrayIsArray => Self::ArrayIsArray,
            NativeCallTarget::ArrayJoin => Self::ArrayJoin,
            NativeCallTarget::ArrayLastIndexOf => Self::ArrayLastIndexOf,
            NativeCallTarget::ArrayPop => Self::ArrayPop,
            NativeCallTarget::ArrayPush => Self::ArrayPush,
            NativeCallTarget::ArrayReverse => Self::ArrayReverse,
            NativeCallTarget::ArrayShift => Self::ArrayShift,
            NativeCallTarget::ArraySlice => Self::ArraySlice,
            NativeCallTarget::ArrayUnshift => Self::ArrayUnshift,
            NativeCallTarget::Boolean => Self::Boolean,
            NativeCallTarget::Eval => Self::Eval,
            NativeCallTarget::ErrorConstructor(name) => Self::ErrorConstructor(name),
            NativeCallTarget::ErrorPrototypeToString => Self::ErrorPrototypeToString,
            NativeCallTarget::Function => Self::Function,
            NativeCallTarget::FunctionPrototypeBind => Self::FunctionPrototypeBind,
            NativeCallTarget::FunctionPrototypeCall => Self::FunctionPrototypeCall,
            NativeCallTarget::GlobalDecodeUri => Self::GlobalDecodeUri,
            NativeCallTarget::GlobalDecodeUriComponent => Self::GlobalDecodeUriComponent,
            NativeCallTarget::GlobalEncodeUri => Self::GlobalEncodeUri,
            NativeCallTarget::GlobalEncodeUriComponent => Self::GlobalEncodeUriComponent,
            NativeCallTarget::GlobalIsFinite => Self::GlobalIsFinite,
            NativeCallTarget::GlobalIsNan => Self::GlobalIsNan,
            NativeCallTarget::GlobalParseFloat => Self::GlobalParseFloat,
            NativeCallTarget::GlobalParseInt => Self::GlobalParseInt,
            NativeCallTarget::JsonParse => Self::JsonParse,
            NativeCallTarget::JsonStringify => Self::JsonStringify,
            NativeCallTarget::MathAbs => Self::MathAbs,
            NativeCallTarget::MathAcos => Self::MathAcos,
            NativeCallTarget::MathAcosh => Self::MathAcosh,
            NativeCallTarget::MathAsin => Self::MathAsin,
            NativeCallTarget::MathAsinh => Self::MathAsinh,
            NativeCallTarget::MathAtan => Self::MathAtan,
            NativeCallTarget::MathAtan2 => Self::MathAtan2,
            NativeCallTarget::MathAtanh => Self::MathAtanh,
            NativeCallTarget::MathCbrt => Self::MathCbrt,
            NativeCallTarget::MathCeil => Self::MathCeil,
            NativeCallTarget::MathClz32 => Self::MathClz32,
            NativeCallTarget::MathCos => Self::MathCos,
            NativeCallTarget::MathCosh => Self::MathCosh,
            NativeCallTarget::MathExp => Self::MathExp,
            NativeCallTarget::MathExpm1 => Self::MathExpm1,
            NativeCallTarget::MathFloor => Self::MathFloor,
            NativeCallTarget::MathFround => Self::MathFround,
            NativeCallTarget::MathHypot => Self::MathHypot,
            NativeCallTarget::MathImul => Self::MathImul,
            NativeCallTarget::MathLog => Self::MathLog,
            NativeCallTarget::MathLog10 => Self::MathLog10,
            NativeCallTarget::MathLog1p => Self::MathLog1p,
            NativeCallTarget::MathLog2 => Self::MathLog2,
            NativeCallTarget::MathMax => Self::MathMax,
            NativeCallTarget::MathMin => Self::MathMin,
            NativeCallTarget::MathPow => Self::MathPow,
            NativeCallTarget::MathRandom => Self::MathRandom,
            NativeCallTarget::MathRound => Self::MathRound,
            NativeCallTarget::MathSign => Self::MathSign,
            NativeCallTarget::MathSin => Self::MathSin,
            NativeCallTarget::MathSinh => Self::MathSinh,
            NativeCallTarget::MathSqrt => Self::MathSqrt,
            NativeCallTarget::MathTan => Self::MathTan,
            NativeCallTarget::MathTanh => Self::MathTanh,
            NativeCallTarget::MathTrunc => Self::MathTrunc,
            NativeCallTarget::Number => Self::Number,
            NativeCallTarget::NumberIsFinite => Self::NumberIsFinite,
            NativeCallTarget::NumberIsNan => Self::NumberIsNan,
            NativeCallTarget::Object => Self::Object,
            NativeCallTarget::ObjectAssign => Self::ObjectAssign,
            NativeCallTarget::ObjectCreate => Self::ObjectCreate,
            NativeCallTarget::ObjectDefineProperties => Self::ObjectDefineProperties,
            NativeCallTarget::ObjectDefineProperty => Self::ObjectDefineProperty,
            NativeCallTarget::ObjectEntries => Self::ObjectEntries,
            NativeCallTarget::ObjectGetOwnPropertyDescriptor => {
                Self::ObjectGetOwnPropertyDescriptor
            }
            NativeCallTarget::ObjectGetOwnPropertyDescriptors => {
                Self::ObjectGetOwnPropertyDescriptors
            }
            NativeCallTarget::ObjectGetOwnPropertyNames => Self::ObjectGetOwnPropertyNames,
            NativeCallTarget::ObjectGetPrototypeOf => Self::ObjectGetPrototypeOf,
            NativeCallTarget::ObjectHasOwn => Self::ObjectHasOwn,
            NativeCallTarget::ObjectIs => Self::ObjectIs,
            NativeCallTarget::ObjectKeys => Self::ObjectKeys,
            NativeCallTarget::ObjectSetPrototypeOf => Self::ObjectSetPrototypeOf,
            NativeCallTarget::ObjectValues => Self::ObjectValues,
            NativeCallTarget::Promise => Self::Promise,
            NativeCallTarget::PromiseResolve => Self::PromiseResolve,
            NativeCallTarget::PromiseReject => Self::PromiseReject,
            NativeCallTarget::PromiseThen => Self::PromiseThen,
            NativeCallTarget::PromiseCatch => Self::PromiseCatch,
            NativeCallTarget::RegExp => Self::RegExp,
            NativeCallTarget::RegExpPrototypeTest => Self::RegExpPrototypeTest,
            NativeCallTarget::Symbol => Self::Symbol,
            target => Self::from_string_call_target(target),
        }
    }

    pub(super) const fn to_call_target(self) -> Option<NativeCallTarget> {
        if let Some(target) = self.to_array_call_target() {
            return Some(target);
        }
        if let Some(target) = self.to_global_utility_call_target() {
            return Some(target);
        }
        if let Some(target) = self.to_math_call_target() {
            return Some(target);
        }
        if let Some(target) = self.to_object_call_target() {
            return Some(target);
        }
        if let Some(target) = self.to_core_call_target() {
            return Some(target);
        }
        self.to_string_call_target()
    }

    const fn to_array_call_target(self) -> Option<NativeCallTarget> {
        match self {
            Self::Array => Some(NativeCallTarget::Array),
            Self::ArrayConcat => Some(NativeCallTarget::ArrayConcat),
            Self::ArrayIncludes => Some(NativeCallTarget::ArrayIncludes),
            Self::ArrayIndexOf => Some(NativeCallTarget::ArrayIndexOf),
            Self::ArrayIsArray => Some(NativeCallTarget::ArrayIsArray),
            Self::ArrayJoin => Some(NativeCallTarget::ArrayJoin),
            Self::ArrayLastIndexOf => Some(NativeCallTarget::ArrayLastIndexOf),
            Self::ArrayPop => Some(NativeCallTarget::ArrayPop),
            Self::ArrayPush => Some(NativeCallTarget::ArrayPush),
            Self::ArrayReverse => Some(NativeCallTarget::ArrayReverse),
            Self::ArrayShift => Some(NativeCallTarget::ArrayShift),
            Self::ArraySlice => Some(NativeCallTarget::ArraySlice),
            Self::ArrayUnshift => Some(NativeCallTarget::ArrayUnshift),
            _ => None,
        }
    }

    const fn to_global_utility_call_target(self) -> Option<NativeCallTarget> {
        match self {
            Self::GlobalDecodeUri => Some(NativeCallTarget::GlobalDecodeUri),
            Self::GlobalDecodeUriComponent => Some(NativeCallTarget::GlobalDecodeUriComponent),
            Self::GlobalEncodeUri => Some(NativeCallTarget::GlobalEncodeUri),
            Self::GlobalEncodeUriComponent => Some(NativeCallTarget::GlobalEncodeUriComponent),
            Self::GlobalIsFinite => Some(NativeCallTarget::GlobalIsFinite),
            Self::GlobalIsNan => Some(NativeCallTarget::GlobalIsNan),
            Self::GlobalParseFloat => Some(NativeCallTarget::GlobalParseFloat),
            Self::GlobalParseInt => Some(NativeCallTarget::GlobalParseInt),
            Self::NumberIsFinite => Some(NativeCallTarget::NumberIsFinite),
            Self::NumberIsNan => Some(NativeCallTarget::NumberIsNan),
            _ => None,
        }
    }

    const fn to_math_call_target(self) -> Option<NativeCallTarget> {
        match self {
            Self::MathAbs => Some(NativeCallTarget::MathAbs),
            Self::MathAcos => Some(NativeCallTarget::MathAcos),
            Self::MathAcosh => Some(NativeCallTarget::MathAcosh),
            Self::MathAsin => Some(NativeCallTarget::MathAsin),
            Self::MathAsinh => Some(NativeCallTarget::MathAsinh),
            Self::MathAtan => Some(NativeCallTarget::MathAtan),
            Self::MathAtan2 => Some(NativeCallTarget::MathAtan2),
            Self::MathAtanh => Some(NativeCallTarget::MathAtanh),
            Self::MathCbrt => Some(NativeCallTarget::MathCbrt),
            Self::MathCeil => Some(NativeCallTarget::MathCeil),
            Self::MathClz32 => Some(NativeCallTarget::MathClz32),
            Self::MathCos => Some(NativeCallTarget::MathCos),
            Self::MathCosh => Some(NativeCallTarget::MathCosh),
            Self::MathExp => Some(NativeCallTarget::MathExp),
            Self::MathExpm1 => Some(NativeCallTarget::MathExpm1),
            Self::MathFloor => Some(NativeCallTarget::MathFloor),
            Self::MathFround => Some(NativeCallTarget::MathFround),
            Self::MathHypot => Some(NativeCallTarget::MathHypot),
            Self::MathImul => Some(NativeCallTarget::MathImul),
            Self::MathLog => Some(NativeCallTarget::MathLog),
            Self::MathLog10 => Some(NativeCallTarget::MathLog10),
            Self::MathLog1p => Some(NativeCallTarget::MathLog1p),
            Self::MathLog2 => Some(NativeCallTarget::MathLog2),
            Self::MathMax => Some(NativeCallTarget::MathMax),
            Self::MathMin => Some(NativeCallTarget::MathMin),
            Self::MathPow => Some(NativeCallTarget::MathPow),
            Self::MathRandom => Some(NativeCallTarget::MathRandom),
            Self::MathRound => Some(NativeCallTarget::MathRound),
            Self::MathSign => Some(NativeCallTarget::MathSign),
            Self::MathSin => Some(NativeCallTarget::MathSin),
            Self::MathSinh => Some(NativeCallTarget::MathSinh),
            Self::MathSqrt => Some(NativeCallTarget::MathSqrt),
            Self::MathTan => Some(NativeCallTarget::MathTan),
            Self::MathTanh => Some(NativeCallTarget::MathTanh),
            Self::MathTrunc => Some(NativeCallTarget::MathTrunc),
            _ => None,
        }
    }

    const fn to_object_call_target(self) -> Option<NativeCallTarget> {
        match self {
            Self::Object => Some(NativeCallTarget::Object),
            Self::ObjectAssign => Some(NativeCallTarget::ObjectAssign),
            Self::ObjectCreate => Some(NativeCallTarget::ObjectCreate),
            Self::ObjectDefineProperties => Some(NativeCallTarget::ObjectDefineProperties),
            Self::ObjectDefineProperty => Some(NativeCallTarget::ObjectDefineProperty),
            Self::ObjectEntries => Some(NativeCallTarget::ObjectEntries),
            Self::ObjectGetPrototypeOf => Some(NativeCallTarget::ObjectGetPrototypeOf),
            Self::ObjectGetOwnPropertyDescriptor => {
                Some(NativeCallTarget::ObjectGetOwnPropertyDescriptor)
            }
            Self::ObjectGetOwnPropertyDescriptors => {
                Some(NativeCallTarget::ObjectGetOwnPropertyDescriptors)
            }
            Self::ObjectGetOwnPropertyNames => Some(NativeCallTarget::ObjectGetOwnPropertyNames),
            Self::ObjectHasOwn => Some(NativeCallTarget::ObjectHasOwn),
            Self::ObjectIs => Some(NativeCallTarget::ObjectIs),
            Self::ObjectKeys => Some(NativeCallTarget::ObjectKeys),
            Self::ObjectSetPrototypeOf => Some(NativeCallTarget::ObjectSetPrototypeOf),
            Self::ObjectValues => Some(NativeCallTarget::ObjectValues),
            _ => None,
        }
    }

    const fn to_core_call_target(self) -> Option<NativeCallTarget> {
        match self {
            Self::Boolean => Some(NativeCallTarget::Boolean),
            Self::Eval => Some(NativeCallTarget::Eval),
            Self::ErrorConstructor(name) => Some(NativeCallTarget::ErrorConstructor(name)),
            Self::ErrorPrototypeToString => Some(NativeCallTarget::ErrorPrototypeToString),
            Self::Function => Some(NativeCallTarget::Function),
            Self::FunctionPrototypeBind => Some(NativeCallTarget::FunctionPrototypeBind),
            Self::FunctionPrototypeCall => Some(NativeCallTarget::FunctionPrototypeCall),
            Self::JsonParse => Some(NativeCallTarget::JsonParse),
            Self::JsonStringify => Some(NativeCallTarget::JsonStringify),
            Self::Number => Some(NativeCallTarget::Number),
            Self::Promise => Some(NativeCallTarget::Promise),
            Self::PromiseResolve => Some(NativeCallTarget::PromiseResolve),
            Self::PromiseReject => Some(NativeCallTarget::PromiseReject),
            Self::PromiseThen => Some(NativeCallTarget::PromiseThen),
            Self::PromiseCatch => Some(NativeCallTarget::PromiseCatch),
            Self::RegExp => Some(NativeCallTarget::RegExp),
            Self::RegExpPrototypeTest => Some(NativeCallTarget::RegExpPrototypeTest),
            Self::Symbol => Some(NativeCallTarget::Symbol),
            _ => None,
        }
    }

    const fn from_string_call_target(target: NativeCallTarget) -> Self {
        match target {
            NativeCallTarget::StringPrototypeCharAt => Self::StringPrototypeCharAt,
            NativeCallTarget::StringPrototypeCharCodeAt => Self::StringPrototypeCharCodeAt,
            NativeCallTarget::StringPrototypeConcat => Self::StringPrototypeConcat,
            NativeCallTarget::StringPrototypeEndsWith => Self::StringPrototypeEndsWith,
            NativeCallTarget::StringPrototypeIncludes => Self::StringPrototypeIncludes,
            NativeCallTarget::StringPrototypeIndexOf => Self::StringPrototypeIndexOf,
            NativeCallTarget::StringPrototypeLastIndexOf => Self::StringPrototypeLastIndexOf,
            NativeCallTarget::StringPrototypeRepeat => Self::StringPrototypeRepeat,
            NativeCallTarget::StringPrototypeSlice => Self::StringPrototypeSlice,
            NativeCallTarget::StringPrototypeStartsWith => Self::StringPrototypeStartsWith,
            NativeCallTarget::StringPrototypeSubstring => Self::StringPrototypeSubstring,
            NativeCallTarget::StringPrototypeToLowerCase => Self::StringPrototypeToLowerCase,
            NativeCallTarget::StringPrototypeToUpperCase => Self::StringPrototypeToUpperCase,
            NativeCallTarget::StringPrototypeTrim => Self::StringPrototypeTrim,
            NativeCallTarget::StringPrototypeTrimEnd => Self::StringPrototypeTrimEnd,
            NativeCallTarget::StringPrototypeTrimStart => Self::StringPrototypeTrimStart,
            _ => Self::String,
        }
    }

    const fn to_string_call_target(self) -> Option<NativeCallTarget> {
        match self {
            Self::String => Some(NativeCallTarget::String),
            Self::StringPrototypeCharAt => Some(NativeCallTarget::StringPrototypeCharAt),
            Self::StringPrototypeCharCodeAt => Some(NativeCallTarget::StringPrototypeCharCodeAt),
            Self::StringPrototypeConcat => Some(NativeCallTarget::StringPrototypeConcat),
            Self::StringPrototypeEndsWith => Some(NativeCallTarget::StringPrototypeEndsWith),
            Self::StringPrototypeIncludes => Some(NativeCallTarget::StringPrototypeIncludes),
            Self::StringPrototypeIndexOf => Some(NativeCallTarget::StringPrototypeIndexOf),
            Self::StringPrototypeLastIndexOf => Some(NativeCallTarget::StringPrototypeLastIndexOf),
            Self::StringPrototypeRepeat => Some(NativeCallTarget::StringPrototypeRepeat),
            Self::StringPrototypeSlice => Some(NativeCallTarget::StringPrototypeSlice),
            Self::StringPrototypeStartsWith => Some(NativeCallTarget::StringPrototypeStartsWith),
            Self::StringPrototypeSubstring => Some(NativeCallTarget::StringPrototypeSubstring),
            Self::StringPrototypeToLowerCase => Some(NativeCallTarget::StringPrototypeToLowerCase),
            Self::StringPrototypeToUpperCase => Some(NativeCallTarget::StringPrototypeToUpperCase),
            Self::StringPrototypeTrim => Some(NativeCallTarget::StringPrototypeTrim),
            Self::StringPrototypeTrimEnd => Some(NativeCallTarget::StringPrototypeTrimEnd),
            Self::StringPrototypeTrimStart => Some(NativeCallTarget::StringPrototypeTrimStart),
            _ => None,
        }
    }
}
