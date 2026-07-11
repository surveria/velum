use super::{EngineCase, Expectation};

const PATH_TEST262_NUMERIC_TYPED_ARRAYS: &str =
    "tests/corpora/test262/active/built-ins/TypedArray/numeric_typed_arrays.js";

pub(super) fn test262_typed_array_cases() -> Vec<EngineCase> {
    vec![EngineCase {
        id: "built-ins/TypedArray/numeric-typed-arrays",
        path: PATH_TEST262_NUMERIC_TYPED_ARRAYS,
        expectation: Expectation::Value("42"),
    }]
}
