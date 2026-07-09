use super::{EngineCase, Expectation};

#[path = "cases_test262_string.rs"]
mod cases_test262_string;

use cases_test262_string::test262_string_builtin_cases;

const PATH_TEST262_ARITHMETIC: &str =
    "tests/corpora/test262/active/language/expressions/arithmetic.js";
const PATH_TEST262_NUMERIC_LITERALS: &str =
    "tests/corpora/test262/active/language/expressions/numeric_literals.js";
const PATH_TEST262_CONDITIONAL_BITAND: &str =
    "tests/corpora/test262/active/language/expressions/conditional_bitand.js";
const PATH_TEST262_STRING_ESCAPE_SEQUENCES: &str =
    "tests/corpora/test262/active/language/expressions/string_escape_sequences.js";
const PATH_TEST262_TEMPLATE_LITERALS: &str =
    "tests/corpora/test262/active/language/expressions/template_literals.js";
const PATH_TEST262_ASYNC_AWAIT: &str =
    "tests/corpora/test262/active/language/expressions/async_await.js";
const PATH_TEST262_ASYNC_ARROW: &str =
    "tests/corpora/test262/active/language/expressions/async_arrow.js";
const PATH_TEST262_FUNCTION_EXPRESSION: &str =
    "tests/corpora/test262/active/language/expressions/function_expression.js";
const PATH_TEST262_FUNCTION_PROPERTIES: &str =
    "tests/corpora/test262/active/language/expressions/function_properties.js";
const PATH_TEST262_FUNCTION_CUSTOM_PROPERTIES: &str =
    "tests/corpora/test262/active/language/expressions/function_custom_properties.js";
const PATH_TEST262_DEFAULT_PARAMETERS: &str =
    "tests/corpora/test262/active/language/expressions/default_parameters.js";
const PATH_TEST262_FUNCTION_DECLARATION: &str =
    "tests/corpora/test262/active/language/statements/function_declaration.js";
const PATH_TEST262_FUNCTION_DESCRIPTORS: &str =
    "tests/corpora/test262/active/built-ins/Function/descriptors.js";
const PATH_TEST262_FUNCTION_APPLY_HAS_INSTANCE: &str =
    "tests/corpora/test262/active/built-ins/Function/apply_has_instance.js";
const PATH_TEST262_FUNCTION_INTRINSIC_DESCRIPTORS: &str =
    "tests/corpora/test262/active/built-ins/Function/intrinsic_descriptors.js";
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
const PATH_TEST262_OBJECT_LITERAL_SHORTHAND_METHODS: &str =
    "tests/corpora/test262/active/language/expressions/object_literal_shorthand_methods.js";
const PATH_TEST262_OBJECT_PROTOTYPES: &str =
    "tests/corpora/test262/active/language/expressions/object_prototypes.js";
const PATH_TEST262_OBJECT_PROTOTYPE_ROOT: &str =
    "tests/corpora/test262/active/language/expressions/object_prototype_root.js";
const PATH_TEST262_OBJECT_BUILTIN: &str =
    "tests/corpora/test262/active/language/expressions/object_builtin.js";
const PATH_TEST262_OBJECT_DESCRIPTORS: &str =
    "tests/corpora/test262/active/built-ins/Object/descriptors.js";
const PATH_TEST262_OBJECT_STATIC_METHODS: &str =
    "tests/corpora/test262/active/built-ins/Object/static_methods.js";
const PATH_TEST262_OBJECT_INTEGRITY_METHODS: &str =
    "tests/corpora/test262/active/built-ins/Object/integrity_methods.js";
const PATH_TEST262_OBJECT_PROTOTYPE_METHODS: &str =
    "tests/corpora/test262/active/built-ins/Object/prototype_methods.js";
const PATH_TEST262_NUMBER_BUILTIN: &str =
    "tests/corpora/test262/active/built-ins/Number/constructor.js";
const PATH_TEST262_NUMBER_FORMATTING: &str =
    "tests/corpora/test262/active/built-ins/Number/number_formatting.js";
const PATH_TEST262_NUMBER_STATIC_METHODS: &str =
    "tests/corpora/test262/active/built-ins/Number/static_methods.js";
const PATH_TEST262_BOOLEAN_BUILTIN: &str =
    "tests/corpora/test262/active/built-ins/Boolean/constructor.js";
const PATH_TEST262_BOOLEAN_PROTOTYPE_METHODS: &str =
    "tests/corpora/test262/active/built-ins/Boolean/prototype_methods.js";
const PATH_TEST262_NUMBER_PROTOTYPE_METHODS: &str =
    "tests/corpora/test262/active/built-ins/Number/prototype_methods.js";
const PATH_TEST262_SYMBOL_PROTOTYPE_METHODS: &str =
    "tests/corpora/test262/active/built-ins/Symbol/prototype_methods.js";
const PATH_TEST262_COMPUTED_PROPERTIES: &str =
    "tests/corpora/test262/active/language/expressions/computed_properties.js";
const PATH_TEST262_ARRAY_LITERALS: &str =
    "tests/corpora/test262/active/language/expressions/array_literals.js";
const PATH_TEST262_ARRAY_BUILTIN: &str =
    "tests/corpora/test262/active/language/expressions/array_builtin.js";
const PATH_TEST262_ARRAY_PROTOTYPE_METHODS: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_methods.js";
const PATH_TEST262_ARRAY_PROTOTYPE_GENERIC_METHODS: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_generic_methods.js";
const PATH_TEST262_ARRAY_PROTOTYPE_CALLBACK_METHODS: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_callback_methods.js";
const PATH_TEST262_ARRAY_FLAT_FLATMAP: &str =
    "tests/corpora/test262/active/built-ins/Array/flat_flatmap.js";
const PATH_TEST262_ARRAY_PROTOTYPE_CONCAT: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_concat.js";
const PATH_TEST262_ARRAY_PROTOTYPE_INCLUDES: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_includes.js";
const PATH_TEST262_ARRAY_PROTOTYPE_JOIN: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_join.js";
const PATH_TEST262_ARRAY_PROTOTYPE_INDEX_OF: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_index_of.js";
const PATH_TEST262_ARRAY_PROTOTYPE_LAST_INDEX_OF: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_last_index_of.js";
const PATH_TEST262_ARRAY_PROTOTYPE_REVERSE: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_reverse.js";
const PATH_TEST262_ARRAY_PROTOTYPE_SORT_COPY_METHODS: &str =
    "tests/corpora/test262/active/language/expressions/array_prototype_sort_copy_methods.js";
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
const PATH_TEST262_NULLISH_LOGICAL_ASSIGNMENT: &str =
    "tests/corpora/test262/active/language/expressions/nullish_logical_assignment.js";
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
const PATH_TEST262_OMITTED_CATCH_BINDING: &str =
    "tests/corpora/test262/active/language/statements/omitted_catch_binding.js";
const PATH_TEST262_TRY_FINALLY: &str =
    "tests/corpora/test262/active/language/statements/try_finally.js";
const PATH_TEST262_WHILE: &str = "tests/corpora/test262/active/language/statements/while.js";
const PATH_TEST262_BREAK_CONTINUE: &str =
    "tests/corpora/test262/active/language/statements/break_continue.js";
const PATH_TEST262_FOR: &str = "tests/corpora/test262/active/language/statements/for.js";
const PATH_TEST262_FOR_IN: &str = "tests/corpora/test262/active/language/statements/for_in.js";
const PATH_TEST262_FOR_OF: &str = "tests/corpora/test262/active/language/statements/for_of.js";
const PATH_TEST262_DESTRUCTURING: &str =
    "tests/corpora/test262/active/language/statements/destructuring.js";
const PATH_TEST262_SPREAD_REST: &str =
    "tests/corpora/test262/active/language/expressions/spread_rest.js";
const PATH_TEST262_CLASS_BASELINE: &str =
    "tests/corpora/test262/active/language/statements/class_baseline.js";
const PATH_TEST262_CLASS_INHERITANCE: &str =
    "tests/corpora/test262/active/language/statements/class_inheritance.js";
const PATH_TEST262_ARGUMENTS_OBJECT: &str =
    "tests/corpora/test262/active/language/arguments_object.js";
const PATH_TEST262_CLASS_FIELDS: &str =
    "tests/corpora/test262/active/language/statements/class_fields.js";
const PATH_TEST262_DATE: &str = "tests/corpora/test262/active/built-ins/Date/date_baseline.js";
const PATH_TEST262_FOR_IN_NULLISH_ERROR: &str =
    "tests/corpora/test262/active/language/statements/for_in_nullish_error.js";
const PATH_TEST262_SWITCH: &str = "tests/corpora/test262/active/language/statements/switch.js";
const PATH_TEST262_BLOCK_LEXICAL_SCOPE: &str =
    "tests/corpora/test262/active/language/statements/block_lexical_scope.js";
const PATH_TEST262_ASSERT_THROWS_REFERENCE_ERROR: &str =
    "tests/corpora/test262/active/language/statements/assert_throws_reference_error.js";
const PATH_TEST262_ERROR_OBJECT_PROPERTIES: &str =
    "tests/corpora/test262/active/language/statements/error_object_properties.js";
const PATH_TEST262_GLOBAL_NUMERIC_CONSTANTS: &str =
    "tests/corpora/test262/active/built-ins/global/numeric_constants.js";
const PATH_TEST262_GLOBAL_UTILITY_FUNCTIONS: &str =
    "tests/corpora/test262/active/built-ins/global/utility_functions.js";
const PATH_TEST262_GLOBAL_THIS: &str =
    "tests/corpora/test262/active/built-ins/global/global_this.js";
const PATH_TEST262_JSON_BUILTIN: &str = "tests/corpora/test262/active/built-ins/JSON/basic.js";
const PATH_TEST262_PROMISE_BUILTIN: &str =
    "tests/corpora/test262/active/built-ins/Promise/basic.js";
const PATH_TEST262_MATH_BUILTIN: &str = "tests/corpora/test262/active/built-ins/Math/basic.js";
const PATH_TEST262_MATH_INTEGER_METHODS: &str =
    "tests/corpora/test262/active/built-ins/Math/integer_methods.js";
const PATH_TEST262_MATH_METHODS: &str = "tests/corpora/test262/active/built-ins/Math/methods.js";
const PATH_TEST262_MATH_RANDOM: &str = "tests/corpora/test262/active/built-ins/Math/random.js";
const PATH_TEST262_MATH_RESIDUAL: &str = "tests/corpora/test262/active/built-ins/Math/residual.js";
const PATH_TEST262_STANDARD_ERROR_CONSTRUCTORS: &str =
    "tests/corpora/test262/active/language/statements/standard_error_constructors.js";
const PATH_TEST262_ERROR_PROTOTYPE_TO_STRING: &str =
    "tests/corpora/test262/active/built-ins/Error/prototype_to_string.js";
const PATH_TEST262_REGEXP_BASELINE: &str =
    "tests/corpora/test262/active/built-ins/RegExp/baseline.js";

pub fn test262_cases() -> Vec<EngineCase> {
    let mut cases = test262_expression_cases();
    cases.extend(test262_builtin_cases());
    cases.extend(test262_binding_cases());
    cases.extend(test262_statement_cases());
    cases.extend(test262_class_cases());
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
            id: "language/expressions/numeric_literals",
            path: PATH_TEST262_NUMERIC_LITERALS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/conditional_bitand",
            path: PATH_TEST262_CONDITIONAL_BITAND,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/string_escape_sequences",
            path: PATH_TEST262_STRING_ESCAPE_SEQUENCES,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/template_literals",
            path: PATH_TEST262_TEMPLATE_LITERALS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/async_await",
            path: PATH_TEST262_ASYNC_AWAIT,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/async_arrow",
            path: PATH_TEST262_ASYNC_ARROW,
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
            id: "language/expressions/default_parameters",
            path: PATH_TEST262_DEFAULT_PARAMETERS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/function_declaration",
            path: PATH_TEST262_FUNCTION_DECLARATION,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Function/descriptors",
            path: PATH_TEST262_FUNCTION_DESCRIPTORS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Function/intrinsic_descriptors",
            path: PATH_TEST262_FUNCTION_INTRINSIC_DESCRIPTORS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Function/apply-has-instance",
            path: PATH_TEST262_FUNCTION_APPLY_HAS_INSTANCE,
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
    let mut cases = test262_plain_object_expression_cases();
    cases.extend(test262_array_expression_cases());
    cases
}

fn test262_plain_object_expression_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "language/expressions/object_literals",
            path: PATH_TEST262_OBJECT_LITERALS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/object_literal_shorthand_methods",
            path: PATH_TEST262_OBJECT_LITERAL_SHORTHAND_METHODS,
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
    ]
}

fn test262_array_expression_cases() -> Vec<EngineCase> {
    vec![
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
            id: "language/expressions/array_prototype_generic_methods",
            path: PATH_TEST262_ARRAY_PROTOTYPE_GENERIC_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_callback_methods",
            path: PATH_TEST262_ARRAY_PROTOTYPE_CALLBACK_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Array/flat_flatmap",
            path: PATH_TEST262_ARRAY_FLAT_FLATMAP,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_join",
            path: PATH_TEST262_ARRAY_PROTOTYPE_JOIN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_concat",
            path: PATH_TEST262_ARRAY_PROTOTYPE_CONCAT,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_includes",
            path: PATH_TEST262_ARRAY_PROTOTYPE_INCLUDES,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_index_of",
            path: PATH_TEST262_ARRAY_PROTOTYPE_INDEX_OF,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_last_index_of",
            path: PATH_TEST262_ARRAY_PROTOTYPE_LAST_INDEX_OF,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_reverse",
            path: PATH_TEST262_ARRAY_PROTOTYPE_REVERSE,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/array_prototype_sort_copy_methods",
            path: PATH_TEST262_ARRAY_PROTOTYPE_SORT_COPY_METHODS,
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
            id: "language/expressions/nullish_logical_assignment",
            path: PATH_TEST262_NULLISH_LOGICAL_ASSIGNMENT,
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

fn test262_builtin_cases() -> Vec<EngineCase> {
    let mut cases = test262_primitive_builtin_cases();
    cases.extend(test262_global_builtin_cases());
    cases.extend(test262_math_builtin_cases());
    cases.extend(test262_object_builtin_cases());
    cases.extend(test262_string_builtin_cases());
    cases.extend(super::cases_test262_collections::test262_collection_builtin_cases());
    cases
}

fn test262_primitive_builtin_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "built-ins/Boolean/constructor",
            path: PATH_TEST262_BOOLEAN_BUILTIN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Boolean/prototype-methods",
            path: PATH_TEST262_BOOLEAN_PROTOTYPE_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Number/constructor",
            path: PATH_TEST262_NUMBER_BUILTIN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Number/number-formatting",
            path: PATH_TEST262_NUMBER_FORMATTING,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Number/static-methods",
            path: PATH_TEST262_NUMBER_STATIC_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Number/prototype-methods",
            path: PATH_TEST262_NUMBER_PROTOTYPE_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Symbol/prototype-methods",
            path: PATH_TEST262_SYMBOL_PROTOTYPE_METHODS,
            expectation: Expectation::Value("42"),
        },
    ]
}

fn test262_global_builtin_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "built-ins/global/numeric-constants",
            path: PATH_TEST262_GLOBAL_NUMERIC_CONSTANTS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/global/utility-functions",
            path: PATH_TEST262_GLOBAL_UTILITY_FUNCTIONS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/global/globalThis",
            path: PATH_TEST262_GLOBAL_THIS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/JSON/basic",
            path: PATH_TEST262_JSON_BUILTIN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Promise/basic",
            path: PATH_TEST262_PROMISE_BUILTIN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/RegExp/baseline",
            path: PATH_TEST262_REGEXP_BASELINE,
            expectation: Expectation::Value("42"),
        },
    ]
}

fn test262_math_builtin_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "built-ins/Math/basic",
            path: PATH_TEST262_MATH_BUILTIN,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Math/methods",
            path: PATH_TEST262_MATH_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Math/integer-methods",
            path: PATH_TEST262_MATH_INTEGER_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Math/random",
            path: PATH_TEST262_MATH_RANDOM,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Math/residual",
            path: PATH_TEST262_MATH_RESIDUAL,
            expectation: Expectation::Value("42"),
        },
    ]
}

fn test262_object_builtin_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "built-ins/Object/descriptors",
            path: PATH_TEST262_OBJECT_DESCRIPTORS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Object/static-methods",
            path: PATH_TEST262_OBJECT_STATIC_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Object/integrity-methods",
            path: PATH_TEST262_OBJECT_INTEGRITY_METHODS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Object/prototype-methods",
            path: PATH_TEST262_OBJECT_PROTOTYPE_METHODS,
            expectation: Expectation::Value("42"),
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

fn test262_class_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "language/statements/class_baseline",
            path: PATH_TEST262_CLASS_BASELINE,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/class_inheritance",
            path: PATH_TEST262_CLASS_INHERITANCE,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/arguments_object",
            path: PATH_TEST262_ARGUMENTS_OBJECT,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/class_fields",
            path: PATH_TEST262_CLASS_FIELDS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Map/map_set_baseline",
            path: super::cases_test262_collections::PATH_TEST262_MAP_SET,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Date/date_baseline",
            path: PATH_TEST262_DATE,
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
            id: "language/statements/omitted_catch_binding",
            path: PATH_TEST262_OMITTED_CATCH_BINDING,
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
            id: "language/statements/for_of",
            path: PATH_TEST262_FOR_OF,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/statements/destructuring",
            path: PATH_TEST262_DESTRUCTURING,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "language/expressions/spread_rest",
            path: PATH_TEST262_SPREAD_REST,
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
        EngineCase {
            id: "language/statements/standard_error_constructors",
            path: PATH_TEST262_STANDARD_ERROR_CONSTRUCTORS,
            expectation: Expectation::Value("42"),
        },
        EngineCase {
            id: "built-ins/Error/prototype_to_string",
            path: PATH_TEST262_ERROR_PROTOTYPE_TO_STRING,
            expectation: Expectation::Value("42"),
        },
    ]
}
