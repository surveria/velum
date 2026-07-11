use super::{EngineCase, Expectation};

const PATH_TEST262_NUMERIC_TYPED_ARRAYS: &str =
    "tests/corpora/test262/active/built-ins/TypedArray/numeric_typed_arrays.js";
const PATH_TEST262_DATA_VIEW: &str =
    "tests/corpora/test262/active/built-ins/DataView/data_view_numeric.js";

pub(super) fn test262_typed_array_cases() -> Vec<EngineCase> {
    vec![EngineCase {
        id: "built-ins/TypedArray/numeric-typed-arrays",
        path: PATH_TEST262_NUMERIC_TYPED_ARRAYS,
        expectation: Expectation::Value("42"),
    }]
}

pub(super) fn test262_data_view_cases() -> Vec<EngineCase> {
    vec![EngineCase {
        id: "built-ins/DataView/numeric-accessors",
        path: PATH_TEST262_DATA_VIEW,
        expectation: Expectation::Value("42"),
    }]
}
