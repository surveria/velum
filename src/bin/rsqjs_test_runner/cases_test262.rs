use super::{EngineCase, Expectation};

const PATH_TEST262_ARITHMETIC: &str =
    "tests/corpora/test262/active/language/expressions/arithmetic.js";
const PATH_TEST262_CONDITIONAL_BITAND: &str =
    "tests/corpora/test262/active/language/expressions/conditional_bitand.js";
const PATH_TEST262_FUNCTION_EXPRESSION: &str =
    "tests/corpora/test262/active/language/expressions/function_expression.js";
const PATH_TEST262_FUNCTION_PROPERTIES: &str =
    "tests/corpora/test262/active/language/expressions/function_properties.js";
const PATH_TEST262_FUNCTION_CUSTOM_PROPERTIES: &str =
    "tests/corpora/test262/active/language/expressions/function_custom_properties.js";
const PATH_TEST262_METHOD_THIS: &str =
    "tests/corpora/test262/active/language/expressions/method_this.js";
const PATH_TEST262_CONSTRUCTOR_PROTOTYPES: &str =
    "tests/corpora/test262/active/language/expressions/constructor_prototypes.js";
const PATH_TEST262_PROTOTYPE_CONSTRUCTOR_PROPERTY: &str =
    "tests/corpora/test262/active/language/expressions/prototype_constructor_property.js";
const PATH_TEST262_FUNCTION_RETURN: &str =
    "tests/corpora/test262/active/language/statements/function_return.js";
const PATH_TEST262_FUNCTION_PARAMETERS_SCOPE: &str =
    "tests/corpora/test262/active/language/statements/function_parameters_scope.js";
const PATH_TEST262_CLOSURE_ENVIRONMENTS: &str =
    "tests/corpora/test262/active/language/expressions/closure_environments.js";
const PATH_TEST262_OBJECT_LITERALS: &str =
    "tests/corpora/test262/active/language/expressions/object_literals.js";
const PATH_TEST262_OBJECT_PROTOTYPES: &str =
    "tests/corpora/test262/active/language/expressions/object_prototypes.js";
const PATH_TEST262_OBJECT_PROTOTYPE_ROOT: &str =
    "tests/corpora/test262/active/language/expressions/object_prototype_root.js";
const PATH_TEST262_OBJECT_BUILTIN: &str =
    "tests/corpora/test262/active/language/expressions/object_builtin.js";
const PATH_TEST262_COMPUTED_PROPERTIES: &str =
    "tests/corpora/test262/active/language/expressions/computed_properties.js";
const PATH_TEST262_ARRAY_LITERALS: &str =
    "tests/corpora/test262/active/language/expressions/array_literals.js";
const PATH_TEST262_ARRAY_BUILTIN: &str =
    "tests/corpora/test262/active/language/expressions/array_builtin.js";
const PATH_TEST262_ARRAY_PROTOTYPE_METHODS: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_methods.js";
const PATH_TEST262_ARRAY_PROTOTYPE_JOIN: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_join.js";
const PATH_TEST262_ARRAY_PROTOTYPE_SHIFT_UNSHIFT: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_shift_unshift.js";
const PATH_TEST262_ARRAY_PROTOTYPE_SLICE: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_slice.js";
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
const PATH_TEST262_IN_OPERATOR: &str =
    "tests/corpora/test262/active/language/expressions/in_operator.js";
const PATH_TEST262_IN_OPERATOR_RHS_ERROR: &str =
    "tests/corpora/test262/active/language/expressions/in_operator_rhs_error.js";
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
const PATH_TEST262_FOR_IN: &str = "tests/corpora/test262/active/language/statements/for_in.js";
const PATH_TEST262_FOR_IN_NULLISH_ERROR: &str =
    "tests/corpora/test262/active/language/statements/for_in_nullish_error.js";
const PATH_TEST262_SWITCH: &str = "tests/corpora/test262/active/language/statements/switch.js";
const PATH_TEST262_BLOCK_LEXICAL_SCOPE: &str =
    "tests/corpora/test262/active/language/statements/block_lexical_scope.js";
const PATH_TEST262_ASSERT_THROWS_REFERENCE_ERROR: &str =
    "tests/corpora/test262/active/language/statements/assert_throws_reference_error.js";
const PATH_TEST262_ERROR_OBJECT_PROPERTIES: &str =
    "tests/corpora/test262/active/language/statements/error_object_properties.js";

pub fn test262_cases() -> Vec<EngineCase> {
    let mut cases = test262_expression_cases();
    cases.extend(test262_binding_cases());
    cases.extend(test262_statement_cases());
    cases
}

fn test262_expression_cases() -> Vec<EngineCase> {
    let mut cases = test262_basic_expression_cases();
    cases.extend(test262_function_expression_cases());
    cases.extend(test262_object_expression_cases());
    cases.extend(test262_operator_expression_cases());
    cases
}

fn test262_basic_expression_cases() -> Vec<EngineCase> {
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
    ]
}

fn test262_function_expression_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "language/expressions/function_expression",
            path: PATH_TEST262_FUNCTION_EXPRESSION,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/function_properties",
            path: PATH_TEST262_FUNCTION_PROPERTIES,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/function_custom_properties",
            path: PATH_TEST262_FUNCTION_CUSTOM_PROPERTIES,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/method_this",
            path: PATH_TEST262_METHOD_THIS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/constructor_prototypes",
            path: PATH_TEST262_CONSTRUCTOR_PROTOTYPES,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/prototype_constructor_property",
            path: PATH_TEST262_PROTOTYPE_CONSTRUCTOR_PROPERTY,
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
    ]
}

fn test262_object_expression_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "language/expressions/object_literals",
            path: PATH_TEST262_OBJECT_LITERALS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/object_prototypes",
            path: PATH_TEST262_OBJECT_PROTOTYPES,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/object_prototype_root",
            path: PATH_TEST262_OBJECT_PROTOTYPE_ROOT,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/object_builtin",
            path: PATH_TEST262_OBJECT_BUILTIN,
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
            id: "language/expressions/array_builtin",
            path: PATH_TEST262_ARRAY_BUILTIN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_methods",
            path: PATH_TEST262_ARRAY_PROTOTYPE_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_join",
            path: PATH_TEST262_ARRAY_PROTOTYPE_JOIN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_shift_unshift",
            path: PATH_TEST262_ARRAY_PROTOTYPE_SHIFT_UNSHIFT,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_slice",
            path: PATH_TEST262_ARRAY_PROTOTYPE_SLICE,
            expectation: Expectation::Value("42"),
        },
    ]
}

fn test262_operator_expression_cases() -> Vec<EngineCase> {
    vec![
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
        EngineCase {
            id: "language/expressions/in_operator",
            path: PATH_TEST262_IN_OPERATOR,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/in_operator_rhs_error",
            path: PATH_TEST262_IN_OPERATOR_RHS_ERROR,
            expectation: Expectation::ErrorContains("operator 'in'"),
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
            id: "language/statements/for_in",
            path: PATH_TEST262_FOR_IN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/for_in_nullish_error",
            path: PATH_TEST262_FOR_IN_NULLISH_ERROR,
            expectation: Expectation::ErrorContains("Cannot convert"),
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
