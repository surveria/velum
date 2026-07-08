use super::{
    NativeFunctionKind, STRING_PROTOTYPE_FUNCTION_LENGTH_ONE, STRING_PROTOTYPE_FUNCTION_LENGTH_TWO,
    STRING_PROTOTYPE_FUNCTION_LENGTH_ZERO,
};

impl NativeFunctionKind {
    pub(super) const fn string_static_length(self) -> Option<f64> {
        match self {
            Self::StringFromCharCode | Self::StringFromCodePoint | Self::StringRaw => {
                Some(STRING_PROTOTYPE_FUNCTION_LENGTH_ONE)
            }
            _ => None,
        }
    }

    pub(super) const fn string_prototype_length(self) -> Option<f64> {
        match self {
            Self::StringPrototypeToLocaleLowerCase
            | Self::StringPrototypeToLocaleUpperCase
            | Self::StringPrototypeToLowerCase
            | Self::StringPrototypeToUpperCase
            | Self::StringPrototypeToString
            | Self::StringPrototypeTrim
            | Self::StringPrototypeTrimEnd
            | Self::StringPrototypeTrimStart
            | Self::StringPrototypeValueOf => Some(STRING_PROTOTYPE_FUNCTION_LENGTH_ZERO),
            Self::StringPrototypeReplace
            | Self::StringPrototypeSlice
            | Self::StringPrototypeSplit
            | Self::StringPrototypeSubstring => Some(STRING_PROTOTYPE_FUNCTION_LENGTH_TWO),
            Self::StringPrototypeAt
            | Self::StringPrototypeCharAt
            | Self::StringPrototypeCharCodeAt
            | Self::StringPrototypeCodePointAt
            | Self::StringPrototypeConcat
            | Self::StringPrototypeEndsWith
            | Self::StringPrototypeIncludes
            | Self::StringPrototypeIndexOf
            | Self::StringPrototypeLastIndexOf
            | Self::StringPrototypeMatch
            | Self::StringPrototypePadEnd
            | Self::StringPrototypePadStart
            | Self::StringPrototypeRepeat
            | Self::StringPrototypeSearch
            | Self::StringPrototypeStartsWith => Some(STRING_PROTOTYPE_FUNCTION_LENGTH_ONE),
            _ => None,
        }
    }
}
