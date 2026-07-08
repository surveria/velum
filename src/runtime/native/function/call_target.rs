use crate::api::native_call::NativeCallTarget;

use super::{DateFunctionKind, NativeFunctionKind};

impl NativeFunctionKind {
    pub(super) const fn from_call_target(target: NativeCallTarget) -> Self {
        if let Some(kind) = Self::from_array_call_target(target) {
            return kind;
        }
        if let Some(kind) = Self::from_global_utility_call_target(target) {
            return kind;
        }
        if let Some(kind) = Self::from_math_call_target(target) {
            return kind;
        }
        if let Some(kind) = Self::from_object_call_target(target) {
            return kind;
        }
        if let Some(kind) = Self::from_core_call_target(target) {
            return kind;
        }
        Self::from_string_call_target(target)
    }

    const fn from_array_call_target(target: NativeCallTarget) -> Option<Self> {
        match target {
            NativeCallTarget::Array => Some(Self::Array),
            NativeCallTarget::ArrayConcat => Some(Self::ArrayConcat),
            NativeCallTarget::ArrayEvery => Some(Self::ArrayEvery),
            NativeCallTarget::ArrayFilter => Some(Self::ArrayFilter),
            NativeCallTarget::ArrayFind => Some(Self::ArrayFind),
            NativeCallTarget::ArrayFindIndex => Some(Self::ArrayFindIndex),
            NativeCallTarget::ArrayFlat => Some(Self::ArrayFlat),
            NativeCallTarget::ArrayFlatMap => Some(Self::ArrayFlatMap),
            NativeCallTarget::ArrayForEach => Some(Self::ArrayForEach),
            NativeCallTarget::ArrayIncludes => Some(Self::ArrayIncludes),
            NativeCallTarget::ArrayIndexOf => Some(Self::ArrayIndexOf),
            NativeCallTarget::ArrayIsArray => Some(Self::ArrayIsArray),
            NativeCallTarget::ArrayJoin => Some(Self::ArrayJoin),
            NativeCallTarget::ArrayLastIndexOf => Some(Self::ArrayLastIndexOf),
            NativeCallTarget::ArrayMap => Some(Self::ArrayMap),
            NativeCallTarget::ArrayPop => Some(Self::ArrayPop),
            NativeCallTarget::ArrayPush => Some(Self::ArrayPush),
            NativeCallTarget::ArrayReduce => Some(Self::ArrayReduce),
            NativeCallTarget::ArrayReduceRight => Some(Self::ArrayReduceRight),
            NativeCallTarget::ArrayReverse => Some(Self::ArrayReverse),
            NativeCallTarget::ArrayShift => Some(Self::ArrayShift),
            NativeCallTarget::ArraySlice => Some(Self::ArraySlice),
            NativeCallTarget::ArraySome => Some(Self::ArraySome),
            NativeCallTarget::ArrayUnshift => Some(Self::ArrayUnshift),
            _ => None,
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

    const fn from_global_utility_call_target(target: NativeCallTarget) -> Option<Self> {
        match target {
            NativeCallTarget::GlobalDecodeUri => Some(Self::GlobalDecodeUri),
            NativeCallTarget::GlobalDecodeUriComponent => Some(Self::GlobalDecodeUriComponent),
            NativeCallTarget::GlobalEncodeUri => Some(Self::GlobalEncodeUri),
            NativeCallTarget::GlobalEncodeUriComponent => Some(Self::GlobalEncodeUriComponent),
            NativeCallTarget::GlobalIsFinite => Some(Self::GlobalIsFinite),
            NativeCallTarget::GlobalIsNan => Some(Self::GlobalIsNan),
            NativeCallTarget::GlobalParseFloat => Some(Self::GlobalParseFloat),
            NativeCallTarget::GlobalParseInt => Some(Self::GlobalParseInt),
            NativeCallTarget::NumberIsFinite => Some(Self::NumberIsFinite),
            NativeCallTarget::NumberIsInteger => Some(Self::NumberIsInteger),
            NativeCallTarget::NumberIsNan => Some(Self::NumberIsNan),
            NativeCallTarget::NumberIsSafeInteger => Some(Self::NumberIsSafeInteger),
            _ => None,
        }
    }

    const fn from_math_call_target(target: NativeCallTarget) -> Option<Self> {
        match target {
            NativeCallTarget::MathAbs => Some(Self::MathAbs),
            NativeCallTarget::MathAcos => Some(Self::MathAcos),
            NativeCallTarget::MathAcosh => Some(Self::MathAcosh),
            NativeCallTarget::MathAsin => Some(Self::MathAsin),
            NativeCallTarget::MathAsinh => Some(Self::MathAsinh),
            NativeCallTarget::MathAtan => Some(Self::MathAtan),
            NativeCallTarget::MathAtan2 => Some(Self::MathAtan2),
            NativeCallTarget::MathAtanh => Some(Self::MathAtanh),
            NativeCallTarget::MathCbrt => Some(Self::MathCbrt),
            NativeCallTarget::MathCeil => Some(Self::MathCeil),
            NativeCallTarget::MathClz32 => Some(Self::MathClz32),
            NativeCallTarget::MathCos => Some(Self::MathCos),
            NativeCallTarget::MathCosh => Some(Self::MathCosh),
            NativeCallTarget::MathExp => Some(Self::MathExp),
            NativeCallTarget::MathExpm1 => Some(Self::MathExpm1),
            NativeCallTarget::MathF16round => Some(Self::MathF16round),
            NativeCallTarget::MathFloor => Some(Self::MathFloor),
            NativeCallTarget::MathFround => Some(Self::MathFround),
            NativeCallTarget::MathHypot => Some(Self::MathHypot),
            NativeCallTarget::MathImul => Some(Self::MathImul),
            NativeCallTarget::MathLog => Some(Self::MathLog),
            NativeCallTarget::MathLog10 => Some(Self::MathLog10),
            NativeCallTarget::MathLog1p => Some(Self::MathLog1p),
            NativeCallTarget::MathLog2 => Some(Self::MathLog2),
            NativeCallTarget::MathMax => Some(Self::MathMax),
            NativeCallTarget::MathMin => Some(Self::MathMin),
            NativeCallTarget::MathPow => Some(Self::MathPow),
            NativeCallTarget::MathRandom => Some(Self::MathRandom),
            NativeCallTarget::MathRound => Some(Self::MathRound),
            NativeCallTarget::MathSign => Some(Self::MathSign),
            NativeCallTarget::MathSin => Some(Self::MathSin),
            NativeCallTarget::MathSinh => Some(Self::MathSinh),
            NativeCallTarget::MathSqrt => Some(Self::MathSqrt),
            NativeCallTarget::MathSumPrecise => Some(Self::MathSumPrecise),
            NativeCallTarget::MathTan => Some(Self::MathTan),
            NativeCallTarget::MathTanh => Some(Self::MathTanh),
            NativeCallTarget::MathTrunc => Some(Self::MathTrunc),
            _ => None,
        }
    }

    const fn from_object_call_target(target: NativeCallTarget) -> Option<Self> {
        match target {
            NativeCallTarget::Object => Some(Self::Object),
            NativeCallTarget::ObjectAssign => Some(Self::ObjectAssign),
            NativeCallTarget::ObjectCreate => Some(Self::ObjectCreate),
            NativeCallTarget::ObjectDefineProperties => Some(Self::ObjectDefineProperties),
            NativeCallTarget::ObjectDefineProperty => Some(Self::ObjectDefineProperty),
            NativeCallTarget::ObjectEntries => Some(Self::ObjectEntries),
            NativeCallTarget::ObjectFreeze => Some(Self::ObjectFreeze),
            NativeCallTarget::ObjectGetPrototypeOf => Some(Self::ObjectGetPrototypeOf),
            NativeCallTarget::ObjectGetOwnPropertyDescriptor => {
                Some(Self::ObjectGetOwnPropertyDescriptor)
            }
            NativeCallTarget::ObjectGetOwnPropertyDescriptors => {
                Some(Self::ObjectGetOwnPropertyDescriptors)
            }
            NativeCallTarget::ObjectGetOwnPropertyNames => Some(Self::ObjectGetOwnPropertyNames),
            NativeCallTarget::ObjectHasOwn => Some(Self::ObjectHasOwn),
            NativeCallTarget::ObjectIs => Some(Self::ObjectIs),
            NativeCallTarget::ObjectIsExtensible => Some(Self::ObjectIsExtensible),
            NativeCallTarget::ObjectIsFrozen => Some(Self::ObjectIsFrozen),
            NativeCallTarget::ObjectIsSealed => Some(Self::ObjectIsSealed),
            NativeCallTarget::ObjectKeys => Some(Self::ObjectKeys),
            NativeCallTarget::ObjectPreventExtensions => Some(Self::ObjectPreventExtensions),
            NativeCallTarget::ObjectSetPrototypeOf => Some(Self::ObjectSetPrototypeOf),
            NativeCallTarget::ObjectSeal => Some(Self::ObjectSeal),
            NativeCallTarget::ObjectValues => Some(Self::ObjectValues),
            _ => None,
        }
    }

    const fn from_core_call_target(target: NativeCallTarget) -> Option<Self> {
        match target {
            NativeCallTarget::Boolean => Some(Self::Boolean),
            NativeCallTarget::BooleanPrototypeToString => Some(Self::BooleanPrototypeToString),
            NativeCallTarget::BooleanPrototypeValueOf => Some(Self::BooleanPrototypeValueOf),
            NativeCallTarget::Eval => Some(Self::Eval),
            NativeCallTarget::ErrorConstructor(name) => Some(Self::ErrorConstructor(name)),
            NativeCallTarget::ErrorPrototypeToString => Some(Self::ErrorPrototypeToString),
            NativeCallTarget::Function => Some(Self::Function),
            NativeCallTarget::FunctionPrototypeBind => Some(Self::FunctionPrototypeBind),
            NativeCallTarget::FunctionPrototypeCall => Some(Self::FunctionPrototypeCall),
            NativeCallTarget::Date => Some(Self::Date(DateFunctionKind::Constructor)),
            NativeCallTarget::JsonParse => Some(Self::JsonParse),
            NativeCallTarget::JsonStringify => Some(Self::JsonStringify),
            NativeCallTarget::Number => Some(Self::Number),
            NativeCallTarget::NumberPrototypeToLocaleString => {
                Some(Self::NumberPrototypeToLocaleString)
            }
            NativeCallTarget::NumberPrototypeToString => Some(Self::NumberPrototypeToString),
            NativeCallTarget::NumberPrototypeValueOf => Some(Self::NumberPrototypeValueOf),
            NativeCallTarget::Promise => Some(Self::Promise),
            NativeCallTarget::PromiseResolve => Some(Self::PromiseResolve),
            NativeCallTarget::PromiseReject => Some(Self::PromiseReject),
            NativeCallTarget::PromiseThen => Some(Self::PromiseThen),
            NativeCallTarget::PromiseCatch => Some(Self::PromiseCatch),
            NativeCallTarget::RegExp => Some(Self::RegExp),
            NativeCallTarget::RegExpPrototypeExec => Some(Self::RegExpPrototypeExec),
            NativeCallTarget::RegExpPrototypeTest => Some(Self::RegExpPrototypeTest),
            NativeCallTarget::Symbol => Some(Self::Symbol),
            NativeCallTarget::SymbolPrototypeDescriptionGetter => {
                Some(Self::SymbolPrototypeDescriptionGetter)
            }
            NativeCallTarget::SymbolPrototypeToString => Some(Self::SymbolPrototypeToString),
            NativeCallTarget::SymbolPrototypeValueOf => Some(Self::SymbolPrototypeValueOf),
            _ => None,
        }
    }

    const fn to_array_call_target(self) -> Option<NativeCallTarget> {
        match self {
            Self::Array => Some(NativeCallTarget::Array),
            Self::ArrayConcat => Some(NativeCallTarget::ArrayConcat),
            Self::ArrayEvery => Some(NativeCallTarget::ArrayEvery),
            Self::ArrayFilter => Some(NativeCallTarget::ArrayFilter),
            Self::ArrayFind => Some(NativeCallTarget::ArrayFind),
            Self::ArrayFindIndex => Some(NativeCallTarget::ArrayFindIndex),
            Self::ArrayFlat => Some(NativeCallTarget::ArrayFlat),
            Self::ArrayFlatMap => Some(NativeCallTarget::ArrayFlatMap),
            Self::ArrayForEach => Some(NativeCallTarget::ArrayForEach),
            Self::ArrayIncludes => Some(NativeCallTarget::ArrayIncludes),
            Self::ArrayIndexOf => Some(NativeCallTarget::ArrayIndexOf),
            Self::ArrayIsArray => Some(NativeCallTarget::ArrayIsArray),
            Self::ArrayJoin => Some(NativeCallTarget::ArrayJoin),
            Self::ArrayLastIndexOf => Some(NativeCallTarget::ArrayLastIndexOf),
            Self::ArrayMap => Some(NativeCallTarget::ArrayMap),
            Self::ArrayPop => Some(NativeCallTarget::ArrayPop),
            Self::ArrayPush => Some(NativeCallTarget::ArrayPush),
            Self::ArrayReduce => Some(NativeCallTarget::ArrayReduce),
            Self::ArrayReduceRight => Some(NativeCallTarget::ArrayReduceRight),
            Self::ArrayReverse => Some(NativeCallTarget::ArrayReverse),
            Self::ArrayShift => Some(NativeCallTarget::ArrayShift),
            Self::ArraySlice => Some(NativeCallTarget::ArraySlice),
            Self::ArraySome => Some(NativeCallTarget::ArraySome),
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
            Self::NumberIsInteger => Some(NativeCallTarget::NumberIsInteger),
            Self::NumberIsNan => Some(NativeCallTarget::NumberIsNan),
            Self::NumberIsSafeInteger => Some(NativeCallTarget::NumberIsSafeInteger),
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
            Self::MathF16round => Some(NativeCallTarget::MathF16round),
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
            Self::MathSumPrecise => Some(NativeCallTarget::MathSumPrecise),
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
            Self::ObjectFreeze => Some(NativeCallTarget::ObjectFreeze),
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
            Self::ObjectIsExtensible => Some(NativeCallTarget::ObjectIsExtensible),
            Self::ObjectIsFrozen => Some(NativeCallTarget::ObjectIsFrozen),
            Self::ObjectIsSealed => Some(NativeCallTarget::ObjectIsSealed),
            Self::ObjectKeys => Some(NativeCallTarget::ObjectKeys),
            Self::ObjectPreventExtensions => Some(NativeCallTarget::ObjectPreventExtensions),
            Self::ObjectSetPrototypeOf => Some(NativeCallTarget::ObjectSetPrototypeOf),
            Self::ObjectSeal => Some(NativeCallTarget::ObjectSeal),
            Self::ObjectValues => Some(NativeCallTarget::ObjectValues),
            _ => None,
        }
    }

    const fn to_core_call_target(self) -> Option<NativeCallTarget> {
        match self {
            Self::Boolean => Some(NativeCallTarget::Boolean),
            Self::BooleanPrototypeToString => Some(NativeCallTarget::BooleanPrototypeToString),
            Self::BooleanPrototypeValueOf => Some(NativeCallTarget::BooleanPrototypeValueOf),
            Self::Eval => Some(NativeCallTarget::Eval),
            Self::ErrorConstructor(name) => Some(NativeCallTarget::ErrorConstructor(name)),
            Self::ErrorPrototypeToString => Some(NativeCallTarget::ErrorPrototypeToString),
            Self::Function => Some(NativeCallTarget::Function),
            Self::FunctionPrototypeBind => Some(NativeCallTarget::FunctionPrototypeBind),
            Self::FunctionPrototypeCall => Some(NativeCallTarget::FunctionPrototypeCall),
            Self::Date(DateFunctionKind::Constructor) => Some(NativeCallTarget::Date),
            Self::JsonParse => Some(NativeCallTarget::JsonParse),
            Self::JsonStringify => Some(NativeCallTarget::JsonStringify),
            Self::Number => Some(NativeCallTarget::Number),
            Self::NumberPrototypeToLocaleString => {
                Some(NativeCallTarget::NumberPrototypeToLocaleString)
            }
            Self::NumberPrototypeToString => Some(NativeCallTarget::NumberPrototypeToString),
            Self::NumberPrototypeValueOf => Some(NativeCallTarget::NumberPrototypeValueOf),
            Self::Promise => Some(NativeCallTarget::Promise),
            Self::PromiseResolve => Some(NativeCallTarget::PromiseResolve),
            Self::PromiseReject => Some(NativeCallTarget::PromiseReject),
            Self::PromiseThen => Some(NativeCallTarget::PromiseThen),
            Self::PromiseCatch => Some(NativeCallTarget::PromiseCatch),
            Self::RegExp => Some(NativeCallTarget::RegExp),
            Self::RegExpPrototypeExec => Some(NativeCallTarget::RegExpPrototypeExec),
            Self::RegExpPrototypeTest => Some(NativeCallTarget::RegExpPrototypeTest),
            Self::Symbol => Some(NativeCallTarget::Symbol),
            Self::SymbolPrototypeDescriptionGetter => {
                Some(NativeCallTarget::SymbolPrototypeDescriptionGetter)
            }
            Self::SymbolPrototypeToString => Some(NativeCallTarget::SymbolPrototypeToString),
            Self::SymbolPrototypeValueOf => Some(NativeCallTarget::SymbolPrototypeValueOf),
            _ => None,
        }
    }

    const fn from_string_call_target(target: NativeCallTarget) -> Self {
        match target {
            NativeCallTarget::StringFromCharCode => Self::StringFromCharCode,
            NativeCallTarget::StringFromCodePoint => Self::StringFromCodePoint,
            NativeCallTarget::StringRaw => Self::StringRaw,
            NativeCallTarget::StringPrototypeAt => Self::StringPrototypeAt,
            NativeCallTarget::StringPrototypeCharAt => Self::StringPrototypeCharAt,
            NativeCallTarget::StringPrototypeCharCodeAt => Self::StringPrototypeCharCodeAt,
            NativeCallTarget::StringPrototypeCodePointAt => Self::StringPrototypeCodePointAt,
            NativeCallTarget::StringPrototypeConcat => Self::StringPrototypeConcat,
            NativeCallTarget::StringPrototypeEndsWith => Self::StringPrototypeEndsWith,
            NativeCallTarget::StringPrototypeIncludes => Self::StringPrototypeIncludes,
            NativeCallTarget::StringPrototypeIndexOf => Self::StringPrototypeIndexOf,
            NativeCallTarget::StringPrototypeLastIndexOf => Self::StringPrototypeLastIndexOf,
            NativeCallTarget::StringPrototypePadEnd => Self::StringPrototypePadEnd,
            NativeCallTarget::StringPrototypePadStart => Self::StringPrototypePadStart,
            NativeCallTarget::StringPrototypeRepeat => Self::StringPrototypeRepeat,
            NativeCallTarget::StringPrototypeSlice => Self::StringPrototypeSlice,
            NativeCallTarget::StringPrototypeStartsWith => Self::StringPrototypeStartsWith,
            NativeCallTarget::StringPrototypeSubstring => Self::StringPrototypeSubstring,
            NativeCallTarget::StringPrototypeToLocaleLowerCase => {
                Self::StringPrototypeToLocaleLowerCase
            }
            NativeCallTarget::StringPrototypeToLocaleUpperCase => {
                Self::StringPrototypeToLocaleUpperCase
            }
            NativeCallTarget::StringPrototypeToLowerCase => Self::StringPrototypeToLowerCase,
            NativeCallTarget::StringPrototypeToString => Self::StringPrototypeToString,
            NativeCallTarget::StringPrototypeToUpperCase => Self::StringPrototypeToUpperCase,
            NativeCallTarget::StringPrototypeTrim => Self::StringPrototypeTrim,
            NativeCallTarget::StringPrototypeTrimEnd => Self::StringPrototypeTrimEnd,
            NativeCallTarget::StringPrototypeTrimStart => Self::StringPrototypeTrimStart,
            NativeCallTarget::StringPrototypeValueOf => Self::StringPrototypeValueOf,
            _ => Self::String,
        }
    }

    const fn to_string_call_target(self) -> Option<NativeCallTarget> {
        match self {
            Self::String => Some(NativeCallTarget::String),
            Self::StringFromCharCode => Some(NativeCallTarget::StringFromCharCode),
            Self::StringFromCodePoint => Some(NativeCallTarget::StringFromCodePoint),
            Self::StringRaw => Some(NativeCallTarget::StringRaw),
            Self::StringPrototypeAt => Some(NativeCallTarget::StringPrototypeAt),
            Self::StringPrototypeCharAt => Some(NativeCallTarget::StringPrototypeCharAt),
            Self::StringPrototypeCharCodeAt => Some(NativeCallTarget::StringPrototypeCharCodeAt),
            Self::StringPrototypeCodePointAt => Some(NativeCallTarget::StringPrototypeCodePointAt),
            Self::StringPrototypeConcat => Some(NativeCallTarget::StringPrototypeConcat),
            Self::StringPrototypeEndsWith => Some(NativeCallTarget::StringPrototypeEndsWith),
            Self::StringPrototypeIncludes => Some(NativeCallTarget::StringPrototypeIncludes),
            Self::StringPrototypeIndexOf => Some(NativeCallTarget::StringPrototypeIndexOf),
            Self::StringPrototypeLastIndexOf => Some(NativeCallTarget::StringPrototypeLastIndexOf),
            Self::StringPrototypePadEnd => Some(NativeCallTarget::StringPrototypePadEnd),
            Self::StringPrototypePadStart => Some(NativeCallTarget::StringPrototypePadStart),
            Self::StringPrototypeRepeat => Some(NativeCallTarget::StringPrototypeRepeat),
            Self::StringPrototypeSlice => Some(NativeCallTarget::StringPrototypeSlice),
            Self::StringPrototypeStartsWith => Some(NativeCallTarget::StringPrototypeStartsWith),
            Self::StringPrototypeSubstring => Some(NativeCallTarget::StringPrototypeSubstring),
            Self::StringPrototypeToLocaleLowerCase => {
                Some(NativeCallTarget::StringPrototypeToLocaleLowerCase)
            }
            Self::StringPrototypeToLocaleUpperCase => {
                Some(NativeCallTarget::StringPrototypeToLocaleUpperCase)
            }
            Self::StringPrototypeToLowerCase => Some(NativeCallTarget::StringPrototypeToLowerCase),
            Self::StringPrototypeToString => Some(NativeCallTarget::StringPrototypeToString),
            Self::StringPrototypeToUpperCase => Some(NativeCallTarget::StringPrototypeToUpperCase),
            Self::StringPrototypeTrim => Some(NativeCallTarget::StringPrototypeTrim),
            Self::StringPrototypeTrimEnd => Some(NativeCallTarget::StringPrototypeTrimEnd),
            Self::StringPrototypeTrimStart => Some(NativeCallTarget::StringPrototypeTrimStart),
            Self::StringPrototypeValueOf => Some(NativeCallTarget::StringPrototypeValueOf),
            _ => None,
        }
    }
}
