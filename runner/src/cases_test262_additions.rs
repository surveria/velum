use super::{EngineCase, Expectation};

const PATH_TEST262_FUNCTION_CONSTRUCTOR: &str =
    "tests/corpora/test262/active/built-ins/Function/constructor.js";
const PATH_TEST262_SYMBOL_BASIC: &str = "tests/corpora/test262/active/built-ins/Symbol/basic.js";

pub(super) fn test262_additional_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "built-ins/Function/constructor",
            path: PATH_TEST262_FUNCTION_CONSTRUCTOR,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Symbol/basic",
            path: PATH_TEST262_SYMBOL_BASIC,
            expectation: Expectation::Value("42"),
        },
    ]
}
