use super::{EngineCase, Expectation};

const PATH_TEST262_PROXY_CONSTRUCTOR: &str =
    "tests/corpora/test262/active/built-ins/Proxy/constructor.js";
const PATH_TEST262_PROXY_PROPERTY_TRAPS: &str =
    "tests/corpora/test262/active/built-ins/Proxy/property_traps.js";
const PATH_TEST262_PROXY_REFLECTION_TRAPS: &str =
    "tests/corpora/test262/active/built-ins/Proxy/reflection_traps.js";
const PATH_TEST262_PROXY_PROTOTYPE_EXTENSIBILITY_TRAPS: &str =
    "tests/corpora/test262/active/built-ins/Proxy/prototype_extensibility_traps.js";
const PATH_TEST262_PROXY_CALLABLE: &str =
    "tests/corpora/test262/active/built-ins/Proxy/callable.js";

pub(super) fn test262_proxy_builtin_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "built-ins/Proxy/constructor",
            path: PATH_TEST262_PROXY_CONSTRUCTOR,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Proxy/property-traps",
            path: PATH_TEST262_PROXY_PROPERTY_TRAPS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Proxy/reflection-traps",
            path: PATH_TEST262_PROXY_REFLECTION_TRAPS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Proxy/prototype-extensibility-traps",
            path: PATH_TEST262_PROXY_PROTOTYPE_EXTENSIBILITY_TRAPS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Proxy/callable",
            path: PATH_TEST262_PROXY_CALLABLE,
            expectation: Expectation::Value("42"),
        },
    ]
}
