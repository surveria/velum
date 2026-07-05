#[derive(Debug)]
pub struct EngineCase {
    pub id: &'static str,
    pub path: &'static str,
    pub expectation: Expectation,
}

#[derive(Debug)]
pub enum Expectation {
    Value(&'static str),
    OutputAndValue {
        output: &'static [&'static str],
        value: &'static str,
    },
    ErrorContains(&'static str),
}

#[derive(Debug)]
pub struct DifferentialCase {
    pub id: &'static str,
    pub path: &'static str,
}

#[derive(Debug)]
pub struct BenchmarkCase {
    pub id: &'static str,
    pub path: &'static str,
}

const PATH_ARITHMETIC: &str = "tests/engine_cases/arithmetic_precedence.js";
const PATH_HOST_PRINT: &str = "tests/engine_cases/host_print.js";
const PATH_CONST_ASSIGNMENT: &str = "tests/engine_cases/const_assignment_error.js";
const PATH_SHORT_CIRCUIT: &str = "tests/engine_cases/short_circuit.js";
const PATH_VAR_HOISTING: &str = "tests/engine_cases/var_hoisting.js";
const PATH_TRY_CATCH: &str = "tests/engine_cases/try_catch.js";
const PATH_CONDITIONAL_BITAND: &str = "tests/engine_cases/conditional_bitand.js";
const PATH_FUNCTION_EXPRESSION: &str = "tests/engine_cases/function_expression.js";
const PATH_FUNCTION_RETURN: &str = "tests/engine_cases/function_return.js";
const PATH_FUNCTION_PARAMETERS_SCOPE: &str = "tests/engine_cases/function_parameters_scope.js";
const PATH_ASSERT_THROWS_REFERENCE_ERROR: &str =
    "tests/engine_cases/assert_throws_reference_error.js";
const PATH_ERROR_OBJECT_PROPERTIES: &str = "tests/engine_cases/error_object_properties.js";
const PATH_TEST262_ARITHMETIC: &str =
    "tests/corpora/test262/active/language/expressions/arithmetic.js";
const PATH_TEST262_CONDITIONAL_BITAND: &str =
    "tests/corpora/test262/active/language/expressions/conditional_bitand.js";
const PATH_TEST262_FUNCTION_EXPRESSION: &str =
    "tests/corpora/test262/active/language/expressions/function_expression.js";
const PATH_TEST262_FUNCTION_RETURN: &str =
    "tests/corpora/test262/active/language/statements/function_return.js";
const PATH_TEST262_FUNCTION_PARAMETERS_SCOPE: &str =
    "tests/corpora/test262/active/language/statements/function_parameters_scope.js";
const PATH_TEST262_LET_CONST: &str = "tests/corpora/test262/active/language/bindings/let_const.js";
const PATH_TEST262_VAR_HOISTING: &str =
    "tests/corpora/test262/active/language/bindings/var_hoisting.js";
const PATH_TEST262_TRY_CATCH: &str =
    "tests/corpora/test262/active/language/statements/try_catch.js";
const PATH_TEST262_ASSERT_THROWS_REFERENCE_ERROR: &str =
    "tests/corpora/test262/active/language/statements/assert_throws_reference_error.js";
const PATH_TEST262_ERROR_OBJECT_PROPERTIES: &str =
    "tests/corpora/test262/active/language/statements/error_object_properties.js";
const PATH_QUICKJS_PRINT_ARITHMETIC: &str =
    "tests/corpora/quickjs_differential/active/print_arithmetic.js";
const PATH_QUICKJS_PRINT_BINDING: &str =
    "tests/corpora/quickjs_differential/active/print_binding.js";
const PATH_QUICKJS_BOOLEAN_CONVERSION: &str =
    "tests/corpora/quickjs_differential/active/boolean_conversion.js";
const PATH_QUICKJS_VAR_HOISTING: &str = "tests/corpora/quickjs_differential/active/var_hoisting.js";
const PATH_QUICKJS_TRY_CATCH: &str = "tests/corpora/quickjs_differential/active/try_catch.js";
const PATH_QUICKJS_CONDITIONAL_BITAND: &str =
    "tests/corpora/quickjs_differential/active/conditional_bitand.js";
const PATH_QUICKJS_FUNCTION_EXPRESSION: &str =
    "tests/corpora/quickjs_differential/active/function_expression.js";
const PATH_QUICKJS_FUNCTION_RETURN: &str =
    "tests/corpora/quickjs_differential/active/function_return.js";
const PATH_QUICKJS_FUNCTION_PARAMETERS_SCOPE: &str =
    "tests/corpora/quickjs_differential/active/function_parameters_scope.js";
const PATH_QUICKJS_REFERENCE_ERROR_CATCH: &str =
    "tests/corpora/quickjs_differential/active/reference_error_catch.js";
const PATH_QUICKJS_ERROR_OBJECT_PROPERTIES: &str =
    "tests/corpora/quickjs_differential/active/error_object_properties.js";
const PATH_BENCH_ARITHMETIC: &str = "tests/corpora/benchmarks/active/arithmetic_chain.js";
const PATH_BENCH_CONDITIONAL_BITAND: &str = "tests/corpora/benchmarks/active/conditional_bitand.js";
const PATH_BENCH_FUNCTION_EXPRESSION: &str =
    "tests/corpora/benchmarks/active/function_expression.js";
const PATH_BENCH_FUNCTION_RETURN: &str = "tests/corpora/benchmarks/active/function_return.js";
const PATH_BENCH_FUNCTION_PARAMETERS_SCOPE: &str =
    "tests/corpora/benchmarks/active/function_parameters_scope.js";
const PATH_BENCH_STRING: &str = "tests/corpora/benchmarks/active/string_concat.js";
const PATH_BENCH_BOOLEAN: &str = "tests/corpora/benchmarks/active/boolean_conversion.js";
const PATH_BENCH_VAR_HOISTING: &str = "tests/corpora/benchmarks/active/var_hoisting.js";
const PATH_BENCH_TRY_CATCH: &str = "tests/corpora/benchmarks/active/try_catch.js";
const PATH_BENCH_REFERENCE_ERROR_CATCH: &str =
    "tests/corpora/benchmarks/active/reference_error_catch.js";
const PATH_BENCH_ERROR_OBJECT_PROPERTIES: &str =
    "tests/corpora/benchmarks/active/error_object_properties.js";

pub fn engine_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "arithmetic_precedence",
            path: PATH_ARITHMETIC,
            expectation: Expectation::Value("5"),
        },
        EngineCase {
            id: "host_print",
            path: PATH_HOST_PRINT,
            expectation: Expectation::OutputAndValue {
                output: &["hello camera"],
                value: "id-7",
            },
        },
        EngineCase {
            id: "const_assignment_error",
            path: PATH_CONST_ASSIGNMENT,
            expectation: Expectation::ErrorContains("assignment to constant"),
        },
        EngineCase {
            id: "short_circuit",
            path: PATH_SHORT_CIRCUIT,
            expectation: Expectation::Value("ok"),
        },
        EngineCase {
            id: "var_hoisting",
            path: PATH_VAR_HOISTING,
            expectation: Expectation::OutputAndValue {
                output: &["undefined"],
                value: "42",
            },
        },
        EngineCase {
            id: "try_catch",
            path: PATH_TRY_CATCH,
            expectation: Expectation::OutputAndValue {
                output: &["boom"],
                value: "42",
            },
        },
        EngineCase {
            id: "conditional_bitand",
            path: PATH_CONDITIONAL_BITAND,
            expectation: Expectation::OutputAndValue {
                output: &["1"],
                value: "42",
            },
        },
        EngineCase {
            id: "function_expression",
            path: PATH_FUNCTION_EXPRESSION,
            expectation: Expectation::OutputAndValue {
                output: &["called"],
                value: "42",
            },
        },
        EngineCase {
            id: "function_return",
            path: PATH_FUNCTION_RETURN,
            expectation: Expectation::OutputAndValue {
                output: &["42", "undefined"],
                value: "42",
            },
        },
        EngineCase {
            id: "function_parameters_scope",
            path: PATH_FUNCTION_PARAMETERS_SCOPE,
            expectation: Expectation::OutputAndValue {
                output: &["42", "undefined", "7", "99", "2", "42"],
                value: "42",
            },
        },
        EngineCase {
            id: "assert_throws_reference_error",
            path: PATH_ASSERT_THROWS_REFERENCE_ERROR,
            expectation: Expectation::OutputAndValue {
                output: &["ReferenceError: 'missing' is not defined"],
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
    ]
}

pub fn test262_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "language/expressions/arithmetic",
            path: PATH_TEST262_ARITHMETIC,
            expectation: Expectation::Value("5"),
        },
        EngineCase {
            id: "language/expressions/conditional_bitand",
            path: PATH_TEST262_CONDITIONAL_BITAND,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/function_expression",
            path: PATH_TEST262_FUNCTION_EXPRESSION,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/function_return",
            path: PATH_TEST262_FUNCTION_RETURN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/function_parameters_scope",
            path: PATH_TEST262_FUNCTION_PARAMETERS_SCOPE,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/bindings/let_const",
            path: PATH_TEST262_LET_CONST,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/bindings/var_hoisting",
            path: PATH_TEST262_VAR_HOISTING,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/try_catch",
            path: PATH_TEST262_TRY_CATCH,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/assert_throws_reference_error",
            path: PATH_TEST262_ASSERT_THROWS_REFERENCE_ERROR,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/error_object_properties",
            path: PATH_TEST262_ERROR_OBJECT_PROPERTIES,
            expectation: Expectation::Value("42"),
        },
    ]
}

pub fn quickjs_differential_cases() -> Vec<DifferentialCase> {
    vec![
        DifferentialCase {
            id: "print_arithmetic",
            path: PATH_QUICKJS_PRINT_ARITHMETIC,
        },
        DifferentialCase {
            id: "print_binding",
            path: PATH_QUICKJS_PRINT_BINDING,
        },
        DifferentialCase {
            id: "boolean_conversion",
            path: PATH_QUICKJS_BOOLEAN_CONVERSION,
        },
        DifferentialCase {
            id: "var_hoisting",
            path: PATH_QUICKJS_VAR_HOISTING,
        },
        DifferentialCase {
            id: "try_catch",
            path: PATH_QUICKJS_TRY_CATCH,
        },
        DifferentialCase {
            id: "conditional_bitand",
            path: PATH_QUICKJS_CONDITIONAL_BITAND,
        },
        DifferentialCase {
            id: "function_expression",
            path: PATH_QUICKJS_FUNCTION_EXPRESSION,
        },
        DifferentialCase {
            id: "function_return",
            path: PATH_QUICKJS_FUNCTION_RETURN,
        },
        DifferentialCase {
            id: "function_parameters_scope",
            path: PATH_QUICKJS_FUNCTION_PARAMETERS_SCOPE,
        },
        DifferentialCase {
            id: "reference_error_catch",
            path: PATH_QUICKJS_REFERENCE_ERROR_CATCH,
        },
        DifferentialCase {
            id: "error_object_properties",
            path: PATH_QUICKJS_ERROR_OBJECT_PROPERTIES,
        },
    ]
}

pub fn benchmark_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase {
            id: "arithmetic_chain",
            path: PATH_BENCH_ARITHMETIC,
        },
        BenchmarkCase {
            id: "conditional_bitand",
            path: PATH_BENCH_CONDITIONAL_BITAND,
        },
        BenchmarkCase {
            id: "function_expression",
            path: PATH_BENCH_FUNCTION_EXPRESSION,
        },
        BenchmarkCase {
            id: "function_return",
            path: PATH_BENCH_FUNCTION_RETURN,
        },
        BenchmarkCase {
            id: "function_parameters_scope",
            path: PATH_BENCH_FUNCTION_PARAMETERS_SCOPE,
        },
        BenchmarkCase {
            id: "string_concat",
            path: PATH_BENCH_STRING,
        },
        BenchmarkCase {
            id: "boolean_conversion",
            path: PATH_BENCH_BOOLEAN,
        },
        BenchmarkCase {
            id: "var_hoisting",
            path: PATH_BENCH_VAR_HOISTING,
        },
        BenchmarkCase {
            id: "try_catch",
            path: PATH_BENCH_TRY_CATCH,
        },
        BenchmarkCase {
            id: "reference_error_catch",
            path: PATH_BENCH_REFERENCE_ERROR_CATCH,
        },
        BenchmarkCase {
            id: "error_object_properties",
            path: PATH_BENCH_ERROR_OBJECT_PROPERTIES,
        },
    ]
}
