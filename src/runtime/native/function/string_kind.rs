use super::kind::{
    NativeFunctionKind, STRING_FROM_CHAR_CODE_NAME, STRING_FROM_CODE_POINT_NAME,
    STRING_PROTOTYPE_AT_NAME, STRING_PROTOTYPE_CHAR_AT_NAME, STRING_PROTOTYPE_CHAR_CODE_AT_NAME,
    STRING_PROTOTYPE_CODE_POINT_AT_NAME, STRING_PROTOTYPE_CONCAT_NAME,
    STRING_PROTOTYPE_ENDS_WITH_NAME, STRING_PROTOTYPE_INCLUDES_NAME,
    STRING_PROTOTYPE_INDEX_OF_NAME, STRING_PROTOTYPE_LAST_INDEX_OF_NAME,
    STRING_PROTOTYPE_MATCH_NAME, STRING_PROTOTYPE_PAD_END_NAME, STRING_PROTOTYPE_PAD_START_NAME,
    STRING_PROTOTYPE_REPEAT_NAME, STRING_PROTOTYPE_REPLACE_NAME, STRING_PROTOTYPE_SEARCH_NAME,
    STRING_PROTOTYPE_SLICE_NAME, STRING_PROTOTYPE_SPLIT_NAME, STRING_PROTOTYPE_STARTS_WITH_NAME,
    STRING_PROTOTYPE_SUBSTRING_NAME, STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_NAME,
    STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_NAME, STRING_PROTOTYPE_TO_LOWER_CASE_NAME,
    STRING_PROTOTYPE_TO_STRING_NAME, STRING_PROTOTYPE_TO_UPPER_CASE_NAME,
    STRING_PROTOTYPE_TRIM_END_NAME, STRING_PROTOTYPE_TRIM_NAME, STRING_PROTOTYPE_TRIM_START_NAME,
    STRING_PROTOTYPE_VALUE_OF_NAME, STRING_RAW_NAME,
};

impl NativeFunctionKind {
    pub(in crate::runtime::native::function) const fn string_static_name(
        self,
    ) -> Option<&'static str> {
        match self {
            Self::StringFromCharCode => Some(STRING_FROM_CHAR_CODE_NAME),
            Self::StringFromCodePoint => Some(STRING_FROM_CODE_POINT_NAME),
            Self::StringRaw => Some(STRING_RAW_NAME),
            _ => None,
        }
    }

    pub(in crate::runtime::native::function) const fn string_prototype_name(
        self,
    ) -> Option<&'static str> {
        match self {
            Self::StringPrototypeAt => Some(STRING_PROTOTYPE_AT_NAME),
            Self::StringPrototypeCharAt => Some(STRING_PROTOTYPE_CHAR_AT_NAME),
            Self::StringPrototypeCharCodeAt => Some(STRING_PROTOTYPE_CHAR_CODE_AT_NAME),
            Self::StringPrototypeCodePointAt => Some(STRING_PROTOTYPE_CODE_POINT_AT_NAME),
            Self::StringPrototypeConcat => Some(STRING_PROTOTYPE_CONCAT_NAME),
            Self::StringPrototypeEndsWith => Some(STRING_PROTOTYPE_ENDS_WITH_NAME),
            Self::StringPrototypeIncludes => Some(STRING_PROTOTYPE_INCLUDES_NAME),
            Self::StringPrototypeIndexOf => Some(STRING_PROTOTYPE_INDEX_OF_NAME),
            Self::StringPrototypeLastIndexOf => Some(STRING_PROTOTYPE_LAST_INDEX_OF_NAME),
            Self::StringPrototypeMatch => Some(STRING_PROTOTYPE_MATCH_NAME),
            Self::StringPrototypePadEnd => Some(STRING_PROTOTYPE_PAD_END_NAME),
            Self::StringPrototypePadStart => Some(STRING_PROTOTYPE_PAD_START_NAME),
            Self::StringPrototypeRepeat => Some(STRING_PROTOTYPE_REPEAT_NAME),
            Self::StringPrototypeReplace => Some(STRING_PROTOTYPE_REPLACE_NAME),
            Self::StringPrototypeSearch => Some(STRING_PROTOTYPE_SEARCH_NAME),
            Self::StringPrototypeSlice => Some(STRING_PROTOTYPE_SLICE_NAME),
            Self::StringPrototypeSplit => Some(STRING_PROTOTYPE_SPLIT_NAME),
            Self::StringPrototypeStartsWith => Some(STRING_PROTOTYPE_STARTS_WITH_NAME),
            Self::StringPrototypeSubstring => Some(STRING_PROTOTYPE_SUBSTRING_NAME),
            Self::StringPrototypeToLocaleLowerCase => {
                Some(STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_NAME)
            }
            Self::StringPrototypeToLocaleUpperCase => {
                Some(STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_NAME)
            }
            Self::StringPrototypeToLowerCase => Some(STRING_PROTOTYPE_TO_LOWER_CASE_NAME),
            Self::StringPrototypeToString => Some(STRING_PROTOTYPE_TO_STRING_NAME),
            Self::StringPrototypeToUpperCase => Some(STRING_PROTOTYPE_TO_UPPER_CASE_NAME),
            Self::StringPrototypeTrim => Some(STRING_PROTOTYPE_TRIM_NAME),
            Self::StringPrototypeTrimEnd => Some(STRING_PROTOTYPE_TRIM_END_NAME),
            Self::StringPrototypeTrimStart => Some(STRING_PROTOTYPE_TRIM_START_NAME),
            Self::StringPrototypeValueOf => Some(STRING_PROTOTYPE_VALUE_OF_NAME),
            _ => None,
        }
    }
}
