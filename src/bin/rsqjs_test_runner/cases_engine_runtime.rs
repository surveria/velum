use super::{EngineCase, Expectation};

const PATH_ASSERT_THROWS_REFERENCE_ERROR: &str =
    "tests/engine_cases/assert_throws_reference_error.js";
const PATH_BOOLEAN_BUILTIN: &str = "tests/engine_cases/boolean_builtin.js";
const PATH_ERROR_OBJECT_PROPERTIES: &str = "tests/engine_cases/error_object_properties.js";
const PATH_GLOBAL_INFINITY_COMPOUND_ASSIGNMENT_ERROR: &str =
    "tests/engine_cases/global_infinity_compound_assignment_error.js";
const PATH_GLOBAL_NAN_ASSIGNMENT_ERROR: &str = "tests/engine_cases/global_nan_assignment_error.js";
const PATH_GLOBAL_NUMERIC_CONSTANTS: &str = "tests/engine_cases/global_numeric_constants.js";
const PATH_MATH_BUILTIN: &str = "tests/engine_cases/math_builtin.js";
const PATH_MATH_METHODS: &str = "tests/engine_cases/math_methods.js";
const PATH_STANDARD_ERROR_CONSTRUCTORS: &str = "tests/engine_cases/standard_error_constructors.js";

pub(super) fn engine_runtime_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "assert_throws_reference_error",
            path: PATH_ASSERT_THROWS_REFERENCE_ERROR,
            expectation: Expectation::OutputAndValue {
                output: &["ReferenceError: 'missing' is not defined"],
                value: "42",
            },
        },
        EngineCase {
            id: "boolean_builtin",
            path: PATH_BOOLEAN_BUILTIN,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "function Boolean 1 true",
                    "false false false false false false true true true true",
                    "object true true true",
                    "keys:|",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "error_object_properties",
            path: PATH_ERROR_OBJECT_PROPERTIES,
            expectation: Expectation::OutputAndValue {
                output: &["ReferenceError", "'missing' is not defined"],
                value: "42",
            },
        },
        EngineCase {
            id: "global_numeric_constants",
            path: PATH_GLOBAL_NUMERIC_CONSTANTS,
            expectation: Expectation::OutputAndValue {
                output: &["number true true true", "false false false true"],
                value: "42",
            },
        },
        EngineCase {
            id: "global_nan_assignment_error",
            path: PATH_GLOBAL_NAN_ASSIGNMENT_ERROR,
            expectation: Expectation::ErrorContains("assignment to constant 'NaN'"),
        },
        EngineCase {
            id: "global_infinity_compound_assignment_error",
            path: PATH_GLOBAL_INFINITY_COMPOUND_ASSIGNMENT_ERROR,
            expectation: Expectation::ErrorContains("assignment to constant 'Infinity'"),
        },
        EngineCase {
            id: "math_builtin",
            path: PATH_MATH_BUILTIN,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "object true true true abs 2 2",
                    "7 2 1 -1 2 -1 9 32 7 -2",
                    "-Infinity Infinity true true true",
                    "true true keys: 42",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "math_methods",
            path: PATH_MATH_METHODS,
            expectation: Expectation::OutputAndValue {
                output: &["true true true", "true true true true true"],
                value: "42",
            },
        },
        EngineCase {
            id: "standard_error_constructors",
            path: PATH_STANDARD_ERROR_CONSTRUCTORS,
            expectation: Expectation::OutputAndValue {
                output: &["Error plain TypeError typed SyntaxError syntax"],
                value: "42",
            },
        },
    ]
}
