use super::{EngineCase, Expectation};

pub(super) const PATH_TEST262_MAP_SET: &str =
    "tests/corpora/test262/active/built-ins/Map/map_set_baseline.js";
const PATH_TEST262_SET_OPERATIONS: &str =
    "tests/corpora/test262/active/built-ins/Set/set_operations.js";
const PATH_TEST262_WEAK_MAP: &str =
    "tests/corpora/test262/active/built-ins/WeakMap/weak_collections_baseline.js";
const PATH_TEST262_WEAK_SET: &str =
    "tests/corpora/test262/active/built-ins/WeakSet/weak_set_baseline.js";

pub(super) fn test262_collection_builtin_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "built-ins/Map/map-set-baseline",
            path: PATH_TEST262_MAP_SET,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Set/set-operations",
            path: PATH_TEST262_SET_OPERATIONS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/WeakMap/weak-map-baseline",
            path: PATH_TEST262_WEAK_MAP,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/WeakSet/weak-set-baseline",
            path: PATH_TEST262_WEAK_SET,
            expectation: Expectation::Value("42"),
        },
    ]
}
