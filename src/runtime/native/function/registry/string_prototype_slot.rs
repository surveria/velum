use super::{
    NativeFunctionKind, NativeFunctionSlot, STRING_FROM_CHAR_CODE_SLOT,
    STRING_FROM_CODE_POINT_SLOT, STRING_PROTOTYPE_AT_SLOT, STRING_PROTOTYPE_CHAR_AT_SLOT,
    STRING_PROTOTYPE_CHAR_CODE_AT_SLOT, STRING_PROTOTYPE_CODE_POINT_AT_SLOT,
    STRING_PROTOTYPE_CONCAT_SLOT, STRING_PROTOTYPE_ENDS_WITH_SLOT, STRING_PROTOTYPE_INCLUDES_SLOT,
    STRING_PROTOTYPE_INDEX_OF_SLOT, STRING_PROTOTYPE_LAST_INDEX_OF_SLOT,
    STRING_PROTOTYPE_LOCALE_COMPARE_SLOT, STRING_PROTOTYPE_NORMALIZE_SLOT,
    STRING_PROTOTYPE_PAD_END_SLOT, STRING_PROTOTYPE_PAD_START_SLOT, STRING_PROTOTYPE_REPEAT_SLOT,
    STRING_PROTOTYPE_SLICE_SLOT, STRING_PROTOTYPE_STARTS_WITH_SLOT,
    STRING_PROTOTYPE_SUBSTRING_SLOT, STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_SLOT,
    STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_SLOT, STRING_PROTOTYPE_TO_LOWER_CASE_SLOT,
    STRING_PROTOTYPE_TO_STRING_SLOT, STRING_PROTOTYPE_TO_UPPER_CASE_SLOT,
    STRING_PROTOTYPE_TRIM_END_SLOT, STRING_PROTOTYPE_TRIM_SLOT, STRING_PROTOTYPE_TRIM_START_SLOT,
    STRING_PROTOTYPE_VALUE_OF_SLOT, STRING_RAW_SLOT,
};

pub(super) const fn string_prototype_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::StringPrototypeAt => Some(STRING_PROTOTYPE_AT_SLOT),
        NativeFunctionKind::StringPrototypeCharAt => Some(STRING_PROTOTYPE_CHAR_AT_SLOT),
        NativeFunctionKind::StringPrototypeCharCodeAt => Some(STRING_PROTOTYPE_CHAR_CODE_AT_SLOT),
        NativeFunctionKind::StringPrototypeCodePointAt => Some(STRING_PROTOTYPE_CODE_POINT_AT_SLOT),
        NativeFunctionKind::StringPrototypeConcat => Some(STRING_PROTOTYPE_CONCAT_SLOT),
        NativeFunctionKind::StringPrototypeEndsWith => Some(STRING_PROTOTYPE_ENDS_WITH_SLOT),
        NativeFunctionKind::StringPrototypeIncludes => Some(STRING_PROTOTYPE_INCLUDES_SLOT),
        NativeFunctionKind::StringPrototypeIndexOf => Some(STRING_PROTOTYPE_INDEX_OF_SLOT),
        NativeFunctionKind::StringPrototypeLastIndexOf => Some(STRING_PROTOTYPE_LAST_INDEX_OF_SLOT),
        NativeFunctionKind::StringPrototypeLocaleCompare => {
            Some(STRING_PROTOTYPE_LOCALE_COMPARE_SLOT)
        }
        NativeFunctionKind::StringPrototypeNormalize => Some(STRING_PROTOTYPE_NORMALIZE_SLOT),
        NativeFunctionKind::StringPrototypePadEnd => Some(STRING_PROTOTYPE_PAD_END_SLOT),
        NativeFunctionKind::StringPrototypePadStart => Some(STRING_PROTOTYPE_PAD_START_SLOT),
        NativeFunctionKind::StringPrototypeRepeat => Some(STRING_PROTOTYPE_REPEAT_SLOT),
        NativeFunctionKind::StringPrototypeSlice => Some(STRING_PROTOTYPE_SLICE_SLOT),
        NativeFunctionKind::StringPrototypeStartsWith => Some(STRING_PROTOTYPE_STARTS_WITH_SLOT),
        NativeFunctionKind::StringPrototypeSubstring => Some(STRING_PROTOTYPE_SUBSTRING_SLOT),
        NativeFunctionKind::StringPrototypeToLocaleLowerCase => {
            Some(STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_SLOT)
        }
        NativeFunctionKind::StringPrototypeToLocaleUpperCase => {
            Some(STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_SLOT)
        }
        NativeFunctionKind::StringPrototypeToLowerCase => Some(STRING_PROTOTYPE_TO_LOWER_CASE_SLOT),
        NativeFunctionKind::StringPrototypeToString => Some(STRING_PROTOTYPE_TO_STRING_SLOT),
        NativeFunctionKind::StringPrototypeToUpperCase => Some(STRING_PROTOTYPE_TO_UPPER_CASE_SLOT),
        NativeFunctionKind::StringPrototypeTrim => Some(STRING_PROTOTYPE_TRIM_SLOT),
        NativeFunctionKind::StringPrototypeTrimEnd => Some(STRING_PROTOTYPE_TRIM_END_SLOT),
        NativeFunctionKind::StringPrototypeTrimStart => Some(STRING_PROTOTYPE_TRIM_START_SLOT),
        NativeFunctionKind::StringPrototypeValueOf => Some(STRING_PROTOTYPE_VALUE_OF_SLOT),
        _ => None,
    }
}

pub(super) const fn string_static_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::StringFromCharCode => Some(STRING_FROM_CHAR_CODE_SLOT),
        NativeFunctionKind::StringFromCodePoint => Some(STRING_FROM_CODE_POINT_SLOT),
        NativeFunctionKind::StringRaw => Some(STRING_RAW_SLOT),
        _ => None,
    }
}
