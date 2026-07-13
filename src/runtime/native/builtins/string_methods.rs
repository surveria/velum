use super::{
    NativeFunctionKind, STRING_PROTOTYPE_CHAR_AT_NAME, STRING_PROTOTYPE_CHAR_CODE_AT_NAME,
    STRING_PROTOTYPE_CONCAT_NAME, STRING_PROTOTYPE_ENDS_WITH_NAME, STRING_PROTOTYPE_INCLUDES_NAME,
    STRING_PROTOTYPE_INDEX_OF_NAME, STRING_PROTOTYPE_LAST_INDEX_OF_NAME,
    STRING_PROTOTYPE_LOCALE_COMPARE_NAME, STRING_PROTOTYPE_MATCH_NAME,
    STRING_PROTOTYPE_NORMALIZE_NAME, STRING_PROTOTYPE_REPEAT_NAME, STRING_PROTOTYPE_REPLACE_NAME,
    STRING_PROTOTYPE_SEARCH_NAME, STRING_PROTOTYPE_SLICE_NAME, STRING_PROTOTYPE_SPLIT_NAME,
    STRING_PROTOTYPE_STARTS_WITH_NAME, STRING_PROTOTYPE_SUBSTRING_NAME,
    STRING_PROTOTYPE_TO_LOWER_CASE_NAME, STRING_PROTOTYPE_TO_UPPER_CASE_NAME,
    STRING_PROTOTYPE_TRIM_END_NAME, STRING_PROTOTYPE_TRIM_NAME, STRING_PROTOTYPE_TRIM_START_NAME,
};

pub(in crate::runtime::native) const STRING_PROTOTYPE_METHODS: &[(&str, NativeFunctionKind)] = &[
    (
        STRING_PROTOTYPE_CHAR_AT_NAME,
        NativeFunctionKind::StringPrototypeCharAt,
    ),
    (
        STRING_PROTOTYPE_CHAR_CODE_AT_NAME,
        NativeFunctionKind::StringPrototypeCharCodeAt,
    ),
    (
        STRING_PROTOTYPE_CONCAT_NAME,
        NativeFunctionKind::StringPrototypeConcat,
    ),
    (
        STRING_PROTOTYPE_ENDS_WITH_NAME,
        NativeFunctionKind::StringPrototypeEndsWith,
    ),
    (
        STRING_PROTOTYPE_INCLUDES_NAME,
        NativeFunctionKind::StringPrototypeIncludes,
    ),
    (
        STRING_PROTOTYPE_INDEX_OF_NAME,
        NativeFunctionKind::StringPrototypeIndexOf,
    ),
    (
        STRING_PROTOTYPE_LAST_INDEX_OF_NAME,
        NativeFunctionKind::StringPrototypeLastIndexOf,
    ),
    (
        STRING_PROTOTYPE_LOCALE_COMPARE_NAME,
        NativeFunctionKind::StringPrototypeLocaleCompare,
    ),
    (
        STRING_PROTOTYPE_MATCH_NAME,
        NativeFunctionKind::StringPrototypeMatch,
    ),
    (
        STRING_PROTOTYPE_NORMALIZE_NAME,
        NativeFunctionKind::StringPrototypeNormalize,
    ),
    (
        STRING_PROTOTYPE_REPEAT_NAME,
        NativeFunctionKind::StringPrototypeRepeat,
    ),
    (
        STRING_PROTOTYPE_REPLACE_NAME,
        NativeFunctionKind::StringPrototypeReplace,
    ),
    (
        STRING_PROTOTYPE_SEARCH_NAME,
        NativeFunctionKind::StringPrototypeSearch,
    ),
    (
        STRING_PROTOTYPE_SLICE_NAME,
        NativeFunctionKind::StringPrototypeSlice,
    ),
    (
        STRING_PROTOTYPE_SPLIT_NAME,
        NativeFunctionKind::StringPrototypeSplit,
    ),
    (
        STRING_PROTOTYPE_STARTS_WITH_NAME,
        NativeFunctionKind::StringPrototypeStartsWith,
    ),
    (
        STRING_PROTOTYPE_SUBSTRING_NAME,
        NativeFunctionKind::StringPrototypeSubstring,
    ),
    (
        STRING_PROTOTYPE_TO_LOWER_CASE_NAME,
        NativeFunctionKind::StringPrototypeToLowerCase,
    ),
    (
        STRING_PROTOTYPE_TO_UPPER_CASE_NAME,
        NativeFunctionKind::StringPrototypeToUpperCase,
    ),
    (
        STRING_PROTOTYPE_TRIM_NAME,
        NativeFunctionKind::StringPrototypeTrim,
    ),
    (
        STRING_PROTOTYPE_TRIM_END_NAME,
        NativeFunctionKind::StringPrototypeTrimEnd,
    ),
    (
        STRING_PROTOTYPE_TRIM_START_NAME,
        NativeFunctionKind::StringPrototypeTrimStart,
    ),
];
