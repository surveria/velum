use super::super::{EngineCase, Expectation};

const PATH_TEST262_STRING_BUILTIN: &str =
    "tests/corpora/test262/active/built-ins/String/constructor.js";
const PATH_TEST262_STRING_PROTOTYPE_METHODS: &str =
    "tests/corpora/test262/active/built-ins/String/prototype_methods.js";
const PATH_TEST262_STRING_REGEXP_INTEROP: &str =
    "tests/corpora/test262/active/built-ins/String/regexp_interop.js";
const PATH_TEST262_STRING_STATIC_UNICODE_METHODS: &str =
    "tests/corpora/test262/active/built-ins/String/static_unicode_methods.js";

pub(super) fn test262_string_builtin_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "built-ins/String/constructor",
            path: PATH_TEST262_STRING_BUILTIN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/String/prototype-methods",
            path: PATH_TEST262_STRING_PROTOTYPE_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/String/regexp-interop",
            path: PATH_TEST262_STRING_REGEXP_INTEROP,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/String/static-unicode-methods",
            path: PATH_TEST262_STRING_STATIC_UNICODE_METHODS,
            expectation: Expectation::Value("42"),
        },
    ]
}
