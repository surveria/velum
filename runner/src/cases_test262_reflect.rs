use super::{EngineCase, Expectation};

const PATH_TEST262_REFLECT_METADATA: &str =
    "tests/corpora/test262/active/built-ins/Reflect/metadata.js";
const PATH_TEST262_REFLECT_PROPERTY_OPS: &str =
    "tests/corpora/test262/active/built-ins/Reflect/property_ops.js";
const PATH_TEST262_REFLECT_PROTOTYPE_EXTENSIBILITY: &str =
    "tests/corpora/test262/active/built-ins/Reflect/prototype_extensibility.js";
const PATH_TEST262_REFLECT_APPLY_CONSTRUCT: &str =
    "tests/corpora/test262/active/built-ins/Reflect/apply_construct.js";

pub(super) fn test262_reflect_builtin_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "built-ins/Reflect/metadata",
            path: PATH_TEST262_REFLECT_METADATA,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Reflect/property-ops",
            path: PATH_TEST262_REFLECT_PROPERTY_OPS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Reflect/prototype-extensibility",
            path: PATH_TEST262_REFLECT_PROTOTYPE_EXTENSIBILITY,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Reflect/apply-construct",
            path: PATH_TEST262_REFLECT_APPLY_CONSTRUCT,
            expectation: Expectation::Value("42"),
        },
    ]
}
