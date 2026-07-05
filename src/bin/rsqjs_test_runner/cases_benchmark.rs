use super::super::BenchmarkCase;

const PATH_BENCH_ARITHMETIC: &str = "tests/corpora/benchmarks/active/arithmetic_chain.js";
const PATH_BENCH_CONDITIONAL_BITAND: &str = "tests/corpora/benchmarks/active/conditional_bitand.js";
const PATH_BENCH_WHILE_STATEMENTS: &str = "tests/corpora/benchmarks/active/while_statements.js";
const PATH_BENCH_BREAK_CONTINUE: &str = "tests/corpora/benchmarks/active/break_continue.js";
const PATH_BENCH_FOR_STATEMENTS: &str = "tests/corpora/benchmarks/active/for_statements.js";
const PATH_BENCH_FOR_IN_STATEMENTS: &str = "tests/corpora/benchmarks/active/for_in_statements.js";
const PATH_BENCH_SWITCH_STATEMENTS: &str = "tests/corpora/benchmarks/active/switch_statements.js";
const PATH_BENCH_BLOCK_LEXICAL_SCOPE: &str =
    "tests/corpora/benchmarks/active/block_lexical_scope.js";
const PATH_BENCH_FUNCTION_EXPRESSION: &str =
    "tests/corpora/benchmarks/active/function_expression.js";
const PATH_BENCH_FUNCTION_PROPERTIES: &str =
    "tests/corpora/benchmarks/active/function_properties.js";
const PATH_BENCH_FUNCTION_CUSTOM_PROPERTIES: &str =
    "tests/corpora/benchmarks/active/function_custom_properties.js";
const PATH_BENCH_METHOD_THIS: &str = "tests/corpora/benchmarks/active/method_this.js";
const PATH_BENCH_CONSTRUCTOR_PROTOTYPES: &str =
    "tests/corpora/benchmarks/active/constructor_prototypes.js";
const PATH_BENCH_PROTOTYPE_CONSTRUCTOR_PROPERTY: &str =
    "tests/corpora/benchmarks/active/prototype_constructor_property.js";
const PATH_BENCH_FUNCTION_RETURN: &str = "tests/corpora/benchmarks/active/function_return.js";
const PATH_BENCH_FUNCTION_PARAMETERS_SCOPE: &str =
    "tests/corpora/benchmarks/active/function_parameters_scope.js";
const PATH_BENCH_CLOSURE_ENVIRONMENTS: &str =
    "tests/corpora/benchmarks/active/closure_environments.js";
const PATH_BENCH_OBJECT_LITERALS: &str = "tests/corpora/benchmarks/active/object_literals.js";
const PATH_BENCH_OBJECT_PROTOTYPES: &str = "tests/corpora/benchmarks/active/object_prototypes.js";
const PATH_BENCH_OBJECT_PROTOTYPE_ROOT: &str =
    "tests/corpora/benchmarks/active/object_prototype_root.js";
const PATH_BENCH_OBJECT_BUILTIN: &str = "tests/corpora/benchmarks/active/object_builtin.js";
const PATH_BENCH_COMPUTED_PROPERTIES: &str =
    "tests/corpora/benchmarks/active/computed_properties.js";
const PATH_BENCH_ARRAY_LITERALS: &str = "tests/corpora/benchmarks/active/array_literals.js";
const PATH_BENCH_ARRAY_BUILTIN: &str = "tests/corpora/benchmarks/active/array_builtin.js";
const PATH_BENCH_ARRAY_PROTOTYPE_METHODS: &str =
    "tests/corpora/benchmarks/active/array_prototype_methods.js";
const PATH_BENCH_ARRAY_PROTOTYPE_JOIN: &str =
    "tests/corpora/benchmarks/active/array_prototype_join.js";
const PATH_BENCH_ARRAY_PROTOTYPE_SHIFT_UNSHIFT: &str =
    "tests/corpora/benchmarks/active/array_prototype_shift_unshift.js";
const PATH_BENCH_ARRAY_PROTOTYPE_SLICE: &str =
    "tests/corpora/benchmarks/active/array_prototype_slice.js";
const PATH_BENCH_UNARY_OPERATORS: &str = "tests/corpora/benchmarks/active/unary_operators.js";
const PATH_BENCH_UPDATE_EXPRESSIONS: &str = "tests/corpora/benchmarks/active/update_expressions.js";
const PATH_BENCH_COMPOUND_ASSIGNMENT: &str =
    "tests/corpora/benchmarks/active/compound_assignment.js";
const PATH_BENCH_COMPOUND_ASSIGNMENT_EXTENDED: &str =
    "tests/corpora/benchmarks/active/compound_assignment_extended.js";
const PATH_BENCH_EXPONENTIATION_PARENTHESES: &str =
    "tests/corpora/benchmarks/active/exponentiation_parentheses.js";
const PATH_BENCH_IN_OPERATOR: &str = "tests/corpora/benchmarks/active/in_operator.js";
const PATH_BENCH_STRING: &str = "tests/corpora/benchmarks/active/string_concat.js";
const PATH_BENCH_BOOLEAN: &str = "tests/corpora/benchmarks/active/boolean_conversion.js";
const PATH_BENCH_VAR_HOISTING: &str = "tests/corpora/benchmarks/active/var_hoisting.js";
const PATH_BENCH_TRY_CATCH: &str = "tests/corpora/benchmarks/active/try_catch.js";
const PATH_BENCH_TRY_FINALLY: &str = "tests/corpora/benchmarks/active/try_finally.js";
const PATH_BENCH_REFERENCE_ERROR_CATCH: &str =
    "tests/corpora/benchmarks/active/reference_error_catch.js";
const PATH_BENCH_ERROR_OBJECT_PROPERTIES: &str =
    "tests/corpora/benchmarks/active/error_object_properties.js";

pub fn benchmark_cases() -> Vec<BenchmarkCase> {
    let mut cases = benchmark_control_flow_cases();
    cases.extend(benchmark_function_cases());
    cases.extend(benchmark_object_cases());
    cases.extend(benchmark_runtime_cases());
    cases
}

fn benchmark_control_flow_cases() -> Vec<BenchmarkCase> {
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
            id: "while_statements",
            path: PATH_BENCH_WHILE_STATEMENTS,
        },
        BenchmarkCase {
            id: "break_continue",
            path: PATH_BENCH_BREAK_CONTINUE,
        },
        BenchmarkCase {
            id: "for_statements",
            path: PATH_BENCH_FOR_STATEMENTS,
        },
        BenchmarkCase {
            id: "for_in_statements",
            path: PATH_BENCH_FOR_IN_STATEMENTS,
        },
        BenchmarkCase {
            id: "switch_statements",
            path: PATH_BENCH_SWITCH_STATEMENTS,
        },
        BenchmarkCase {
            id: "block_lexical_scope",
            path: PATH_BENCH_BLOCK_LEXICAL_SCOPE,
        },
    ]
}

fn benchmark_function_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase {
            id: "function_expression",
            path: PATH_BENCH_FUNCTION_EXPRESSION,
        },
        BenchmarkCase {
            id: "function_properties",
            path: PATH_BENCH_FUNCTION_PROPERTIES,
        },
        BenchmarkCase {
            id: "function_custom_properties",
            path: PATH_BENCH_FUNCTION_CUSTOM_PROPERTIES,
        },
        BenchmarkCase {
            id: "method_this",
            path: PATH_BENCH_METHOD_THIS,
        },
        BenchmarkCase {
            id: "constructor_prototypes",
            path: PATH_BENCH_CONSTRUCTOR_PROTOTYPES,
        },
        BenchmarkCase {
            id: "prototype_constructor_property",
            path: PATH_BENCH_PROTOTYPE_CONSTRUCTOR_PROPERTY,
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
            id: "closure_environments",
            path: PATH_BENCH_CLOSURE_ENVIRONMENTS,
        },
    ]
}

fn benchmark_object_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase {
            id: "object_literals",
            path: PATH_BENCH_OBJECT_LITERALS,
        },
        BenchmarkCase {
            id: "object_prototypes",
            path: PATH_BENCH_OBJECT_PROTOTYPES,
        },
        BenchmarkCase {
            id: "object_prototype_root",
            path: PATH_BENCH_OBJECT_PROTOTYPE_ROOT,
        },
        BenchmarkCase {
            id: "object_builtin",
            path: PATH_BENCH_OBJECT_BUILTIN,
        },
        BenchmarkCase {
            id: "computed_properties",
            path: PATH_BENCH_COMPUTED_PROPERTIES,
        },
        BenchmarkCase {
            id: "array_literals",
            path: PATH_BENCH_ARRAY_LITERALS,
        },
        BenchmarkCase {
            id: "array_builtin",
            path: PATH_BENCH_ARRAY_BUILTIN,
        },
        BenchmarkCase {
            id: "array_prototype_methods",
            path: PATH_BENCH_ARRAY_PROTOTYPE_METHODS,
        },
        BenchmarkCase {
            id: "array_prototype_join",
            path: PATH_BENCH_ARRAY_PROTOTYPE_JOIN,
        },
        BenchmarkCase {
            id: "array_prototype_shift_unshift",
            path: PATH_BENCH_ARRAY_PROTOTYPE_SHIFT_UNSHIFT,
        },
        BenchmarkCase {
            id: "array_prototype_slice",
            path: PATH_BENCH_ARRAY_PROTOTYPE_SLICE,
        },
        BenchmarkCase {
            id: "unary_operators",
            path: PATH_BENCH_UNARY_OPERATORS,
        },
        BenchmarkCase {
            id: "update_expressions",
            path: PATH_BENCH_UPDATE_EXPRESSIONS,
        },
        BenchmarkCase {
            id: "compound_assignment",
            path: PATH_BENCH_COMPOUND_ASSIGNMENT,
        },
        BenchmarkCase {
            id: "compound_assignment_extended",
            path: PATH_BENCH_COMPOUND_ASSIGNMENT_EXTENDED,
        },
        BenchmarkCase {
            id: "exponentiation_parentheses",
            path: PATH_BENCH_EXPONENTIATION_PARENTHESES,
        },
        BenchmarkCase {
            id: "in_operator",
            path: PATH_BENCH_IN_OPERATOR,
        },
    ]
}

fn benchmark_runtime_cases() -> Vec<BenchmarkCase> {
    vec![
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
            id: "try_finally",
            path: PATH_BENCH_TRY_FINALLY,
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
