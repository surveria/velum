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

#[path = "cases_reporting.rs"]
mod cases_reporting;

pub use cases_reporting::{benchmark_cases, quickjs_differential_cases};

const PATH_ARITHMETIC: &str = "tests/engine_cases/arithmetic_precedence.js";
const PATH_HOST_PRINT: &str = "tests/engine_cases/host_print.js";
const PATH_CONST_ASSIGNMENT: &str = "tests/engine_cases/const_assignment_error.js";
const PATH_SHORT_CIRCUIT: &str = "tests/engine_cases/short_circuit.js";
const PATH_VAR_HOISTING: &str = "tests/engine_cases/var_hoisting.js";
const PATH_TRY_CATCH: &str = "tests/engine_cases/try_catch.js";
const PATH_TRY_FINALLY: &str = "tests/engine_cases/try_finally.js";
const PATH_CONDITIONAL_BITAND: &str = "tests/engine_cases/conditional_bitand.js";
const PATH_UPDATE_EXPRESSIONS: &str = "tests/engine_cases/update_expressions.js";
const PATH_COMPOUND_ASSIGNMENT: &str = "tests/engine_cases/compound_assignment.js";
const PATH_COMPOUND_ASSIGNMENT_EXTENDED: &str =
    "tests/engine_cases/compound_assignment_extended.js";
const PATH_EXPONENTIATION_PARENTHESES: &str = "tests/engine_cases/exponentiation_parentheses.js";
const PATH_EXPONENTIATION_UNARY_LEFT_ERROR: &str =
    "tests/engine_cases/exponentiation_unary_left_error.js";
const PATH_WHILE_STATEMENTS: &str = "tests/engine_cases/while_statements.js";
const PATH_BREAK_CONTINUE: &str = "tests/engine_cases/break_continue.js";
const PATH_FOR_STATEMENTS: &str = "tests/engine_cases/for_statements.js";
const PATH_SWITCH_STATEMENTS: &str = "tests/engine_cases/switch_statements.js";
const PATH_BLOCK_LEXICAL_SCOPE: &str = "tests/engine_cases/block_lexical_scope.js";
const PATH_FUNCTION_EXPRESSION: &str = "tests/engine_cases/function_expression.js";
const PATH_FUNCTION_RETURN: &str = "tests/engine_cases/function_return.js";
const PATH_FUNCTION_PARAMETERS_SCOPE: &str = "tests/engine_cases/function_parameters_scope.js";
const PATH_CLOSURE_ENVIRONMENTS: &str = "tests/engine_cases/closure_environments.js";
const PATH_OBJECT_LITERALS: &str = "tests/engine_cases/object_literals.js";
const PATH_COMPUTED_PROPERTIES: &str = "tests/engine_cases/computed_properties.js";
const PATH_ARRAY_LITERALS: &str = "tests/engine_cases/array_literals.js";
const PATH_UNARY_OPERATORS: &str = "tests/engine_cases/unary_operators.js";
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
const PATH_TEST262_CLOSURE_ENVIRONMENTS: &str =
    "tests/corpora/test262/active/language/expressions/closure_environments.js";
const PATH_TEST262_OBJECT_LITERALS: &str =
    "tests/corpora/test262/active/language/expressions/object_literals.js";
const PATH_TEST262_COMPUTED_PROPERTIES: &str =
    "tests/corpora/test262/active/language/expressions/computed_properties.js";
const PATH_TEST262_ARRAY_LITERALS: &str =
    "tests/corpora/test262/active/language/expressions/array_literals.js";
const PATH_TEST262_UNARY_OPERATORS: &str =
    "tests/corpora/test262/active/language/expressions/unary_operators.js";
const PATH_TEST262_UPDATE_EXPRESSIONS: &str =
    "tests/corpora/test262/active/language/expressions/update_expressions.js";
const PATH_TEST262_COMPOUND_ASSIGNMENT: &str =
    "tests/corpora/test262/active/language/expressions/compound_assignment.js";
const PATH_TEST262_COMPOUND_ASSIGNMENT_EXTENDED: &str =
    "tests/corpora/test262/active/language/expressions/compound_assignment_extended.js";
const PATH_TEST262_EXPONENTIATION_PARENTHESES: &str =
    "tests/corpora/test262/active/language/expressions/exponentiation_parentheses.js";
const PATH_TEST262_EXPONENTIATION_UNARY_LEFT_ERROR: &str =
    "tests/corpora/test262/active/language/expressions/exponentiation_unary_left_error.js";
const PATH_TEST262_LET_CONST: &str = "tests/corpora/test262/active/language/bindings/let_const.js";
const PATH_TEST262_VAR_HOISTING: &str =
    "tests/corpora/test262/active/language/bindings/var_hoisting.js";
const PATH_TEST262_TRY_CATCH: &str =
    "tests/corpora/test262/active/language/statements/try_catch.js";
const PATH_TEST262_TRY_FINALLY: &str =
    "tests/corpora/test262/active/language/statements/try_finally.js";
const PATH_TEST262_WHILE: &str = "tests/corpora/test262/active/language/statements/while.js";
const PATH_TEST262_BREAK_CONTINUE: &str =
    "tests/corpora/test262/active/language/statements/break_continue.js";
const PATH_TEST262_FOR: &str = "tests/corpora/test262/active/language/statements/for.js";
const PATH_TEST262_SWITCH: &str = "tests/corpora/test262/active/language/statements/switch.js";
const PATH_TEST262_BLOCK_LEXICAL_SCOPE: &str =
    "tests/corpora/test262/active/language/statements/block_lexical_scope.js";
const PATH_TEST262_ASSERT_THROWS_REFERENCE_ERROR: &str =
    "tests/corpora/test262/active/language/statements/assert_throws_reference_error.js";
const PATH_TEST262_ERROR_OBJECT_PROPERTIES: &str =
    "tests/corpora/test262/active/language/statements/error_object_properties.js";

pub fn engine_cases() -> Vec<EngineCase> {
    let mut cases = engine_language_cases();
    cases.extend(engine_expression_cases());
    cases.extend(engine_control_flow_cases());
    cases.extend(engine_function_cases());
    cases.extend(engine_object_cases());
    cases.extend(engine_runtime_cases());
    cases
}

fn engine_language_cases() -> Vec<EngineCase> {
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
            id: "try_finally",
            path: PATH_TRY_FINALLY,
            expectation: Expectation::OutputAndValue {
                output: &["42 finally try 42"],
                value: "42",
            },
        },
    ]
}

fn engine_control_flow_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "while_statements",
            path: PATH_WHILE_STATEMENTS,
            expectation: Expectation::OutputAndValue {
                output: &["4 42"],
                value: "42",
            },
        },
        EngineCase {
            id: "break_continue",
            path: PATH_BREAK_CONTINUE,
            expectation: Expectation::OutputAndValue {
                output: &["3 42"],
                value: "42",
            },
        },
        EngineCase {
            id: "for_statements",
            path: PATH_FOR_STATEMENTS,
            expectation: Expectation::OutputAndValue {
                output: &["5 1 42 5"],
                value: "42",
            },
        },
        EngineCase {
            id: "switch_statements",
            path: PATH_SWITCH_STATEMENTS,
            expectation: Expectation::OutputAndValue {
                output: &["42 two 46"],
                value: "42",
            },
        },
        EngineCase {
            id: "block_lexical_scope",
            path: PATH_BLOCK_LEXICAL_SCOPE,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "42 number",
                    "1 undefined",
                    "10 undefined undefined",
                    "42 undefined undefined",
                    "42",
                    "boom 40 2 undefined undefined undefined undefined",
                ],
                value: "42",
            },
        },
    ]
}

fn engine_expression_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "conditional_bitand",
            path: PATH_CONDITIONAL_BITAND,
            expectation: Expectation::OutputAndValue {
                output: &["1"],
                value: "42",
            },
        },
        EngineCase {
            id: "update_expressions",
            path: PATH_UPDATE_EXPRESSIONS,
            expectation: Expectation::OutputAndValue {
                output: &["40 42 42 40 40", "1 3 3", "1 3 2 3", "6 -1"],
                value: "42",
            },
        },
        EngineCase {
            id: "compound_assignment",
            path: PATH_COMPOUND_ASSIGNMENT,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "15 12 48 24 3 2 2",
                    "cam-01",
                    "15 12 12",
                    "10 2 2",
                    "kr 42 42",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "compound_assignment_extended",
            path: PATH_COMPOUND_ASSIGNMENT_EXTENDED,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "7 4 16 8 4 64 64",
                    "-4 2147483646 2147483646",
                    "5 2 16 16",
                    "32 2 32 32 32",
                    "kr 42 42",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "exponentiation_parentheses",
            path: PATH_EXPONENTIATION_PARENTHESES,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "512 4 -4",
                    "8 8 10 10",
                    "16 16",
                    "undefined true true undefined",
                    "42",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "exponentiation_unary_left_error",
            path: PATH_EXPONENTIATION_UNARY_LEFT_ERROR,
            expectation: Expectation::ErrorContains("unary expression cannot be the left operand"),
        },
    ]
}

fn engine_function_cases() -> Vec<EngineCase> {
    vec![
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
            id: "closure_environments",
            path: PATH_CLOSURE_ENVIRONMENTS,
            expectation: Expectation::OutputAndValue {
                output: &["41", "42", "42"],
                value: "42",
            },
        },
    ]
}

fn engine_object_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "object_literals",
            path: PATH_OBJECT_LITERALS,
            expectation: Expectation::OutputAndValue {
                output: &["front-door undefined", "42", "42"],
                value: "42",
            },
        },
        EngineCase {
            id: "computed_properties",
            path: PATH_COMPUTED_PROPERTIES,
            expectation: Expectation::OutputAndValue {
                output: &["front-door undefined", "42", "42"],
                value: "42",
            },
        },
        EngineCase {
            id: "array_literals",
            path: PATH_ARRAY_LITERALS,
            expectation: Expectation::OutputAndValue {
                output: &["4 2 undefined", "7 4", "0 2"],
                value: "42",
            },
        },
        EngineCase {
            id: "unary_operators",
            path: PATH_UNARY_OPERATORS,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "true true true false false true",
                    "object undefined undefined undefined function",
                    "2 42 undefined",
                ],
                value: "42",
            },
        },
    ]
}

fn engine_runtime_cases() -> Vec<EngineCase> {
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
    let mut cases = test262_expression_cases();
    cases.extend(test262_binding_cases());
    cases.extend(test262_statement_cases());
    cases
}

fn test262_expression_cases() -> Vec<EngineCase> {
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
            id: "language/expressions/closure_environments",
            path: PATH_TEST262_CLOSURE_ENVIRONMENTS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/object_literals",
            path: PATH_TEST262_OBJECT_LITERALS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/computed_properties",
            path: PATH_TEST262_COMPUTED_PROPERTIES,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_literals",
            path: PATH_TEST262_ARRAY_LITERALS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/unary_operators",
            path: PATH_TEST262_UNARY_OPERATORS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/update_expressions",
            path: PATH_TEST262_UPDATE_EXPRESSIONS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/compound_assignment",
            path: PATH_TEST262_COMPOUND_ASSIGNMENT,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/compound_assignment_extended",
            path: PATH_TEST262_COMPOUND_ASSIGNMENT_EXTENDED,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/exponentiation_parentheses",
            path: PATH_TEST262_EXPONENTIATION_PARENTHESES,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/exponentiation_unary_left_error",
            path: PATH_TEST262_EXPONENTIATION_UNARY_LEFT_ERROR,
            expectation: Expectation::ErrorContains("unary expression cannot be the left operand"),
        },
    ]
}

fn test262_binding_cases() -> Vec<EngineCase> {
    vec![
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
    ]
}

fn test262_statement_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "language/statements/try_catch",
            path: PATH_TEST262_TRY_CATCH,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/try_finally",
            path: PATH_TEST262_TRY_FINALLY,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/while",
            path: PATH_TEST262_WHILE,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/break_continue",
            path: PATH_TEST262_BREAK_CONTINUE,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/for",
            path: PATH_TEST262_FOR,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/switch",
            path: PATH_TEST262_SWITCH,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/block_lexical_scope",
            path: PATH_TEST262_BLOCK_LEXICAL_SCOPE,
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
