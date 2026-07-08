use super::kind::NativeFunctionKind;

const ARRAY_CONCAT_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_CONCAT_NAME: &str = "concat";
const ARRAY_EVERY_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_EVERY_NAME: &str = "every";
const ARRAY_FILTER_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_FILTER_NAME: &str = "filter";
const ARRAY_FIND_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_FIND_NAME: &str = "find";
const ARRAY_FIND_INDEX_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_FIND_INDEX_NAME: &str = "findIndex";
const ARRAY_FOR_EACH_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_FOR_EACH_NAME: &str = "forEach";
const ARRAY_INCLUDES_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_INCLUDES_NAME: &str = "includes";
const ARRAY_INDEX_OF_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_INDEX_OF_NAME: &str = "indexOf";
const ARRAY_IS_ARRAY_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_IS_ARRAY_NAME: &str = "isArray";
const ARRAY_JOIN_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_JOIN_NAME: &str = "join";
const ARRAY_LAST_INDEX_OF_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_LAST_INDEX_OF_NAME: &str = "lastIndexOf";
const ARRAY_MAP_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_MAP_NAME: &str = "map";
const ARRAY_POP_FUNCTION_LENGTH: f64 = 0.0;
const ARRAY_POP_NAME: &str = "pop";
const ARRAY_PUSH_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_PUSH_NAME: &str = "push";
const ARRAY_REDUCE_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_REDUCE_NAME: &str = "reduce";
const ARRAY_REDUCE_RIGHT_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_REDUCE_RIGHT_NAME: &str = "reduceRight";
const ARRAY_REVERSE_FUNCTION_LENGTH: f64 = 0.0;
const ARRAY_REVERSE_NAME: &str = "reverse";
const ARRAY_SHIFT_FUNCTION_LENGTH: f64 = 0.0;
const ARRAY_SHIFT_NAME: &str = "shift";
const ARRAY_SLICE_FUNCTION_LENGTH: f64 = 2.0;
const ARRAY_SLICE_NAME: &str = "slice";
const ARRAY_SOME_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_SOME_NAME: &str = "some";
const ARRAY_UNSHIFT_FUNCTION_LENGTH: f64 = 1.0;
const ARRAY_UNSHIFT_NAME: &str = "unshift";
const ARRAY_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime::native) const ARRAY_NAME: &str = "Array";

impl NativeFunctionKind {
    pub(in crate::runtime::native::function) const fn array_length(self) -> Option<f64> {
        match self {
            Self::Array => Some(ARRAY_FUNCTION_LENGTH),
            Self::ArrayConcat => Some(ARRAY_CONCAT_FUNCTION_LENGTH),
            Self::ArrayEvery => Some(ARRAY_EVERY_FUNCTION_LENGTH),
            Self::ArrayFilter => Some(ARRAY_FILTER_FUNCTION_LENGTH),
            Self::ArrayFind => Some(ARRAY_FIND_FUNCTION_LENGTH),
            Self::ArrayFindIndex => Some(ARRAY_FIND_INDEX_FUNCTION_LENGTH),
            Self::ArrayForEach => Some(ARRAY_FOR_EACH_FUNCTION_LENGTH),
            Self::ArrayIncludes => Some(ARRAY_INCLUDES_FUNCTION_LENGTH),
            Self::ArrayIndexOf => Some(ARRAY_INDEX_OF_FUNCTION_LENGTH),
            Self::ArrayIsArray => Some(ARRAY_IS_ARRAY_FUNCTION_LENGTH),
            Self::ArrayJoin => Some(ARRAY_JOIN_FUNCTION_LENGTH),
            Self::ArrayLastIndexOf => Some(ARRAY_LAST_INDEX_OF_FUNCTION_LENGTH),
            Self::ArrayMap => Some(ARRAY_MAP_FUNCTION_LENGTH),
            Self::ArrayPop => Some(ARRAY_POP_FUNCTION_LENGTH),
            Self::ArrayPush => Some(ARRAY_PUSH_FUNCTION_LENGTH),
            Self::ArrayReduce => Some(ARRAY_REDUCE_FUNCTION_LENGTH),
            Self::ArrayReduceRight => Some(ARRAY_REDUCE_RIGHT_FUNCTION_LENGTH),
            Self::ArrayReverse => Some(ARRAY_REVERSE_FUNCTION_LENGTH),
            Self::ArrayShift => Some(ARRAY_SHIFT_FUNCTION_LENGTH),
            Self::ArraySlice => Some(ARRAY_SLICE_FUNCTION_LENGTH),
            Self::ArraySome => Some(ARRAY_SOME_FUNCTION_LENGTH),
            Self::ArrayUnshift => Some(ARRAY_UNSHIFT_FUNCTION_LENGTH),
            _ => None,
        }
    }

    pub(in crate::runtime::native::function) const fn array_name(self) -> Option<&'static str> {
        match self {
            Self::Array => Some(ARRAY_NAME),
            Self::ArrayConcat => Some(ARRAY_CONCAT_NAME),
            Self::ArrayEvery => Some(ARRAY_EVERY_NAME),
            Self::ArrayFilter => Some(ARRAY_FILTER_NAME),
            Self::ArrayFind => Some(ARRAY_FIND_NAME),
            Self::ArrayFindIndex => Some(ARRAY_FIND_INDEX_NAME),
            Self::ArrayForEach => Some(ARRAY_FOR_EACH_NAME),
            Self::ArrayIncludes => Some(ARRAY_INCLUDES_NAME),
            Self::ArrayIndexOf => Some(ARRAY_INDEX_OF_NAME),
            Self::ArrayIsArray => Some(ARRAY_IS_ARRAY_NAME),
            Self::ArrayJoin => Some(ARRAY_JOIN_NAME),
            Self::ArrayLastIndexOf => Some(ARRAY_LAST_INDEX_OF_NAME),
            Self::ArrayMap => Some(ARRAY_MAP_NAME),
            Self::ArrayPop => Some(ARRAY_POP_NAME),
            Self::ArrayPush => Some(ARRAY_PUSH_NAME),
            Self::ArrayReduce => Some(ARRAY_REDUCE_NAME),
            Self::ArrayReduceRight => Some(ARRAY_REDUCE_RIGHT_NAME),
            Self::ArrayReverse => Some(ARRAY_REVERSE_NAME),
            Self::ArrayShift => Some(ARRAY_SHIFT_NAME),
            Self::ArraySlice => Some(ARRAY_SLICE_NAME),
            Self::ArraySome => Some(ARRAY_SOME_NAME),
            Self::ArrayUnshift => Some(ARRAY_UNSHIFT_NAME),
            _ => None,
        }
    }
}
