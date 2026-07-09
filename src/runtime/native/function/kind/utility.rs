use super::{
    GLOBAL_DECODE_URI_COMPONENT_FUNCTION_LENGTH, GLOBAL_DECODE_URI_COMPONENT_NAME,
    GLOBAL_DECODE_URI_FUNCTION_LENGTH, GLOBAL_DECODE_URI_NAME,
    GLOBAL_ENCODE_URI_COMPONENT_FUNCTION_LENGTH, GLOBAL_ENCODE_URI_COMPONENT_NAME,
    GLOBAL_ENCODE_URI_FUNCTION_LENGTH, GLOBAL_ENCODE_URI_NAME, GLOBAL_IS_FINITE_FUNCTION_LENGTH,
    GLOBAL_IS_FINITE_NAME, GLOBAL_IS_NAN_FUNCTION_LENGTH, GLOBAL_IS_NAN_NAME,
    GLOBAL_PARSE_FLOAT_FUNCTION_LENGTH, GLOBAL_PARSE_FLOAT_NAME, GLOBAL_PARSE_INT_FUNCTION_LENGTH,
    GLOBAL_PARSE_INT_NAME, NUMBER_IS_FINITE_FUNCTION_LENGTH, NUMBER_IS_INTEGER_FUNCTION_LENGTH,
    NUMBER_IS_INTEGER_NAME, NUMBER_IS_NAN_FUNCTION_LENGTH, NUMBER_IS_SAFE_INTEGER_FUNCTION_LENGTH,
    NUMBER_IS_SAFE_INTEGER_NAME, NativeFunctionKind,
};

impl NativeFunctionKind {
    pub(super) const fn global_utility_length(self) -> Option<f64> {
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

    pub(super) const fn global_utility_name(self) -> Option<&'static str> {
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
