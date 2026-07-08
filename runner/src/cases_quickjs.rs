use super::super::DifferentialCase;

const PATH_QUICKJS_PRINT_ARITHMETIC: &str =
    "tests/corpora/quickjs_differential/active/print_arithmetic.js";
const PATH_QUICKJS_NUMERIC_LITERALS: &str =
    "tests/corpora/quickjs_differential/active/numeric_literals.js";
const PATH_QUICKJS_PRINT_BINDING: &str =
    "tests/corpora/quickjs_differential/active/print_binding.js";
const PATH_QUICKJS_STRING_ESCAPE_SEQUENCES: &str =
    "tests/corpora/quickjs_differential/active/string_escape_sequences.js";
const PATH_QUICKJS_TEMPLATE_LITERALS: &str =
    "tests/corpora/quickjs_differential/active/template_literals.js";
const PATH_QUICKJS_BOOLEAN_CONVERSION: &str =
    "tests/corpora/quickjs_differential/active/boolean_conversion.js";
const PATH_QUICKJS_VAR_HOISTING: &str = "tests/corpora/quickjs_differential/active/var_hoisting.js";
const PATH_QUICKJS_TRY_CATCH: &str = "tests/corpora/quickjs_differential/active/try_catch.js";
const PATH_QUICKJS_OMITTED_CATCH_BINDING: &str =
    "tests/corpora/quickjs_differential/active/omitted_catch_binding.js";
const PATH_QUICKJS_TRY_FINALLY: &str = "tests/corpora/quickjs_differential/active/try_finally.js";
const PATH_QUICKJS_CONDITIONAL_BITAND: &str =
    "tests/corpora/quickjs_differential/active/conditional_bitand.js";
const PATH_QUICKJS_WHILE_STATEMENTS: &str =
    "tests/corpora/quickjs_differential/active/while_statements.js";
const PATH_QUICKJS_BREAK_CONTINUE: &str =
    "tests/corpora/quickjs_differential/active/break_continue.js";
const PATH_QUICKJS_FOR_STATEMENTS: &str =
    "tests/corpora/quickjs_differential/active/for_statements.js";
const PATH_QUICKJS_FOR_IN_STATEMENTS: &str =
    "tests/corpora/quickjs_differential/active/for_in_statements.js";
const PATH_QUICKJS_FOR_OF: &str = "tests/corpora/quickjs_differential/active/for_of.js";
const PATH_QUICKJS_DESTRUCTURING: &str =
    "tests/corpora/quickjs_differential/active/destructuring.js";
const PATH_QUICKJS_SPREAD_REST: &str = "tests/corpora/quickjs_differential/active/spread_rest.js";
const PATH_QUICKJS_CLASS_BASELINE: &str =
    "tests/corpora/quickjs_differential/active/class_baseline.js";
const PATH_QUICKJS_CLASS_INHERITANCE: &str =
    "tests/corpora/quickjs_differential/active/class_inheritance.js";
const PATH_QUICKJS_ARGUMENTS_OBJECT: &str =
    "tests/corpora/quickjs_differential/active/arguments_object.js";
const PATH_QUICKJS_CLASS_FIELDS: &str = "tests/corpora/quickjs_differential/active/class_fields.js";
const PATH_QUICKJS_SWITCH_STATEMENTS: &str =
    "tests/corpora/quickjs_differential/active/switch_statements.js";
const PATH_QUICKJS_BLOCK_LEXICAL_SCOPE: &str =
    "tests/corpora/quickjs_differential/active/block_lexical_scope.js";
const PATH_QUICKJS_FUNCTION_EXPRESSION: &str =
    "tests/corpora/quickjs_differential/active/function_expression.js";
const PATH_QUICKJS_FUNCTION_PROPERTIES: &str =
    "tests/corpora/quickjs_differential/active/function_properties.js";
const PATH_QUICKJS_FUNCTION_CUSTOM_PROPERTIES: &str =
    "tests/corpora/quickjs_differential/active/function_custom_properties.js";
const PATH_QUICKJS_FUNCTION_DESCRIPTORS: &str =
    "tests/corpora/quickjs_differential/active/function_descriptors.js";
const PATH_QUICKJS_FUNCTION_INTRINSIC_DESCRIPTORS: &str =
    "tests/corpora/quickjs_differential/active/function_intrinsic_descriptors.js";
const PATH_QUICKJS_METHOD_THIS: &str = "tests/corpora/quickjs_differential/active/method_this.js";
const PATH_QUICKJS_CONSTRUCTOR_PROTOTYPES: &str =
    "tests/corpora/quickjs_differential/active/constructor_prototypes.js";
const PATH_QUICKJS_PROTOTYPE_CONSTRUCTOR_PROPERTY: &str =
    "tests/corpora/quickjs_differential/active/prototype_constructor_property.js";
const PATH_QUICKJS_FUNCTION_RETURN: &str =
    "tests/corpora/quickjs_differential/active/function_return.js";
const PATH_QUICKJS_FUNCTION_PARAMETERS_SCOPE: &str =
    "tests/corpora/quickjs_differential/active/function_parameters_scope.js";
const PATH_QUICKJS_CLOSURE_ENVIRONMENTS: &str =
    "tests/corpora/quickjs_differential/active/closure_environments.js";
const PATH_QUICKJS_OBJECT_LITERALS: &str =
    "tests/corpora/quickjs_differential/active/object_literals.js";
const PATH_QUICKJS_OBJECT_LITERAL_SHORTHAND_METHODS: &str =
    "tests/corpora/quickjs_differential/active/object_literal_shorthand_methods.js";
const PATH_QUICKJS_OBJECT_PROTOTYPES: &str =
    "tests/corpora/quickjs_differential/active/object_prototypes.js";
const PATH_QUICKJS_OBJECT_PROTOTYPE_ROOT: &str =
    "tests/corpora/quickjs_differential/active/object_prototype_root.js";
const PATH_QUICKJS_OBJECT_BUILTIN: &str =
    "tests/corpora/quickjs_differential/active/object_builtin.js";
const PATH_QUICKJS_NUMBER_BUILTIN: &str =
    "tests/corpora/quickjs_differential/active/number_builtin.js";
const PATH_QUICKJS_STRING_BUILTIN: &str =
    "tests/corpora/quickjs_differential/active/string_builtin.js";
const PATH_QUICKJS_STRING_PROTOTYPE_METHODS: &str =
    "tests/corpora/quickjs_differential/active/string_prototype_methods.js";
const PATH_QUICKJS_COMPUTED_PROPERTIES: &str =
    "tests/corpora/quickjs_differential/active/computed_properties.js";
const PATH_QUICKJS_ARRAY_LITERALS: &str =
    "tests/corpora/quickjs_differential/active/array_literals.js";
const PATH_QUICKJS_ARRAY_BUILTIN: &str =
    "tests/corpora/quickjs_differential/active/array_builtin.js";
const PATH_QUICKJS_ARRAY_PROTOTYPE_METHODS: &str =
    "tests/corpora/quickjs_differential/active/array_prototype_methods.js";
const PATH_QUICKJS_ARRAY_PROTOTYPE_CONCAT: &str =
    "tests/corpora/quickjs_differential/active/array_prototype_concat.js";
const PATH_QUICKJS_ARRAY_PROTOTYPE_INCLUDES: &str =
    "tests/corpora/quickjs_differential/active/array_prototype_includes.js";
const PATH_QUICKJS_ARRAY_PROTOTYPE_JOIN: &str =
    "tests/corpora/quickjs_differential/active/array_prototype_join.js";
const PATH_QUICKJS_ARRAY_PROTOTYPE_INDEX_OF: &str =
    "tests/corpora/quickjs_differential/active/array_prototype_index_of.js";
const PATH_QUICKJS_ARRAY_PROTOTYPE_LAST_INDEX_OF: &str =
    "tests/corpora/quickjs_differential/active/array_prototype_last_index_of.js";
const PATH_QUICKJS_ARRAY_PROTOTYPE_REVERSE: &str =
    "tests/corpora/quickjs_differential/active/array_prototype_reverse.js";
const PATH_QUICKJS_ARRAY_PROTOTYPE_SHIFT_UNSHIFT: &str =
    "tests/corpora/quickjs_differential/active/array_prototype_shift_unshift.js";
const PATH_QUICKJS_ARRAY_PROTOTYPE_SLICE: &str =
    "tests/corpora/quickjs_differential/active/array_prototype_slice.js";
const PATH_QUICKJS_UNARY_OPERATORS: &str =
    "tests/corpora/quickjs_differential/active/unary_operators.js";
const PATH_QUICKJS_UPDATE_EXPRESSIONS: &str =
    "tests/corpora/quickjs_differential/active/update_expressions.js";
const PATH_QUICKJS_COMPOUND_ASSIGNMENT: &str =
    "tests/corpora/quickjs_differential/active/compound_assignment.js";
const PATH_QUICKJS_COMPOUND_ASSIGNMENT_EXTENDED: &str =
    "tests/corpora/quickjs_differential/active/compound_assignment_extended.js";
const PATH_QUICKJS_NULLISH_LOGICAL_ASSIGNMENT: &str =
    "tests/corpora/quickjs_differential/active/nullish_logical_assignment.js";
const PATH_QUICKJS_EXPONENTIATION_PARENTHESES: &str =
    "tests/corpora/quickjs_differential/active/exponentiation_parentheses.js";
const PATH_QUICKJS_IN_OPERATOR: &str = "tests/corpora/quickjs_differential/active/in_operator.js";
const PATH_QUICKJS_REFERENCE_ERROR_CATCH: &str =
    "tests/corpora/quickjs_differential/active/reference_error_catch.js";
const PATH_QUICKJS_ERROR_OBJECT_PROPERTIES: &str =
    "tests/corpora/quickjs_differential/active/error_object_properties.js";
const PATH_QUICKJS_GLOBAL_NUMERIC_CONSTANTS: &str =
    "tests/corpora/quickjs_differential/active/global_numeric_constants.js";
const PATH_QUICKJS_GLOBAL_UTILITY_BUILTINS: &str =
    "tests/corpora/quickjs_differential/active/global_utility_builtins.js";
const PATH_QUICKJS_JSON_BUILTIN: &str = "tests/corpora/quickjs_differential/active/json_builtin.js";
const PATH_QUICKJS_MATH_BUILTIN: &str = "tests/corpora/quickjs_differential/active/math_builtin.js";
const PATH_QUICKJS_MATH_INTEGER_METHODS: &str =
    "tests/corpora/quickjs_differential/active/math_integer_methods.js";
const PATH_QUICKJS_MATH_METHODS: &str = "tests/corpora/quickjs_differential/active/math_methods.js";
const PATH_QUICKJS_MATH_RANDOM: &str = "tests/corpora/quickjs_differential/active/math_random.js";
const PATH_QUICKJS_STANDARD_ERROR_CONSTRUCTORS: &str =
    "tests/corpora/quickjs_differential/active/standard_error_constructors.js";
const PATH_QUICKJS_BOOLEAN_BUILTIN: &str =
    "tests/corpora/quickjs_differential/active/boolean_builtin.js";
const PATH_QUICKJS_OBJECT_DESCRIPTORS: &str =
    "tests/corpora/quickjs_differential/active/object_descriptors.js";
const PATH_QUICKJS_OBJECT_STATIC_METHODS: &str =
    "tests/corpora/quickjs_differential/active/object_static_methods.js";

pub fn quickjs_differential_cases() -> Vec<DifferentialCase> {
    let mut cases = quickjs_language_cases();
    cases.extend(quickjs_control_flow_cases());
    cases.extend(quickjs_object_cases());
    cases.extend(quickjs_operator_expression_cases());
    cases.extend(quickjs_runtime_cases());
    cases
}

fn quickjs_language_cases() -> Vec<DifferentialCase> {
    vec![
        DifferentialCase {
            id: "print_arithmetic",
            path: PATH_QUICKJS_PRINT_ARITHMETIC,
        },
        DifferentialCase {
            id: "numeric_literals",
            path: PATH_QUICKJS_NUMERIC_LITERALS,
        },
        DifferentialCase {
            id: "print_binding",
            path: PATH_QUICKJS_PRINT_BINDING,
        },
        DifferentialCase {
            id: "string_escape_sequences",
            path: PATH_QUICKJS_STRING_ESCAPE_SEQUENCES,
        },
        DifferentialCase {
            id: "template_literals",
            path: PATH_QUICKJS_TEMPLATE_LITERALS,
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
            id: "omitted_catch_binding",
            path: PATH_QUICKJS_OMITTED_CATCH_BINDING,
        },
        DifferentialCase {
            id: "try_finally",
            path: PATH_QUICKJS_TRY_FINALLY,
        },
        DifferentialCase {
            id: "conditional_bitand",
            path: PATH_QUICKJS_CONDITIONAL_BITAND,
        },
    ]
}

fn quickjs_control_flow_cases() -> Vec<DifferentialCase> {
    vec![
        DifferentialCase {
            id: "while_statements",
            path: PATH_QUICKJS_WHILE_STATEMENTS,
        },
        DifferentialCase {
            id: "break_continue",
            path: PATH_QUICKJS_BREAK_CONTINUE,
        },
        DifferentialCase {
            id: "for_statements",
            path: PATH_QUICKJS_FOR_STATEMENTS,
        },
        DifferentialCase {
            id: "for_in_statements",
            path: PATH_QUICKJS_FOR_IN_STATEMENTS,
        },
        DifferentialCase {
            id: "for_of",
            path: PATH_QUICKJS_FOR_OF,
        },
        DifferentialCase {
            id: "destructuring",
            path: PATH_QUICKJS_DESTRUCTURING,
        },
        DifferentialCase {
            id: "spread_rest",
            path: PATH_QUICKJS_SPREAD_REST,
        },
        DifferentialCase {
            id: "class_baseline",
            path: PATH_QUICKJS_CLASS_BASELINE,
        },
        DifferentialCase {
            id: "class_inheritance",
            path: PATH_QUICKJS_CLASS_INHERITANCE,
        },
        DifferentialCase {
            id: "arguments_object",
            path: PATH_QUICKJS_ARGUMENTS_OBJECT,
        },
        DifferentialCase {
            id: "class_fields",
            path: PATH_QUICKJS_CLASS_FIELDS,
        },
        DifferentialCase {
            id: "switch_statements",
            path: PATH_QUICKJS_SWITCH_STATEMENTS,
        },
        DifferentialCase {
            id: "block_lexical_scope",
            path: PATH_QUICKJS_BLOCK_LEXICAL_SCOPE,
        },
        DifferentialCase {
            id: "function_expression",
            path: PATH_QUICKJS_FUNCTION_EXPRESSION,
        },
        DifferentialCase {
            id: "function_properties",
            path: PATH_QUICKJS_FUNCTION_PROPERTIES,
        },
        DifferentialCase {
            id: "function_custom_properties",
            path: PATH_QUICKJS_FUNCTION_CUSTOM_PROPERTIES,
        },
        DifferentialCase {
            id: "function_descriptors",
            path: PATH_QUICKJS_FUNCTION_DESCRIPTORS,
        },
        DifferentialCase {
            id: "function_intrinsic_descriptors",
            path: PATH_QUICKJS_FUNCTION_INTRINSIC_DESCRIPTORS,
        },
        DifferentialCase {
            id: "method_this",
            path: PATH_QUICKJS_METHOD_THIS,
        },
        DifferentialCase {
            id: "constructor_prototypes",
            path: PATH_QUICKJS_CONSTRUCTOR_PROTOTYPES,
        },
        DifferentialCase {
            id: "prototype_constructor_property",
            path: PATH_QUICKJS_PROTOTYPE_CONSTRUCTOR_PROPERTY,
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
            id: "closure_environments",
            path: PATH_QUICKJS_CLOSURE_ENVIRONMENTS,
        },
    ]
}

fn quickjs_object_cases() -> Vec<DifferentialCase> {
    vec![
        DifferentialCase {
            id: "object_literals",
            path: PATH_QUICKJS_OBJECT_LITERALS,
        },
        DifferentialCase {
            id: "object_literal_shorthand_methods",
            path: PATH_QUICKJS_OBJECT_LITERAL_SHORTHAND_METHODS,
        },
        DifferentialCase {
            id: "object_prototypes",
            path: PATH_QUICKJS_OBJECT_PROTOTYPES,
        },
        DifferentialCase {
            id: "object_prototype_root",
            path: PATH_QUICKJS_OBJECT_PROTOTYPE_ROOT,
        },
        DifferentialCase {
            id: "object_builtin",
            path: PATH_QUICKJS_OBJECT_BUILTIN,
        },
        DifferentialCase {
            id: "number_builtin",
            path: PATH_QUICKJS_NUMBER_BUILTIN,
        },
        DifferentialCase {
            id: "computed_properties",
            path: PATH_QUICKJS_COMPUTED_PROPERTIES,
        },
        DifferentialCase {
            id: "array_literals",
            path: PATH_QUICKJS_ARRAY_LITERALS,
        },
        DifferentialCase {
            id: "array_builtin",
            path: PATH_QUICKJS_ARRAY_BUILTIN,
        },
        DifferentialCase {
            id: "array_prototype_methods",
            path: PATH_QUICKJS_ARRAY_PROTOTYPE_METHODS,
        },
        DifferentialCase {
            id: "array_prototype_join",
            path: PATH_QUICKJS_ARRAY_PROTOTYPE_JOIN,
        },
        DifferentialCase {
            id: "array_prototype_concat",
            path: PATH_QUICKJS_ARRAY_PROTOTYPE_CONCAT,
        },
        DifferentialCase {
            id: "array_prototype_includes",
            path: PATH_QUICKJS_ARRAY_PROTOTYPE_INCLUDES,
        },
        DifferentialCase {
            id: "array_prototype_index_of",
            path: PATH_QUICKJS_ARRAY_PROTOTYPE_INDEX_OF,
        },
        DifferentialCase {
            id: "array_prototype_last_index_of",
            path: PATH_QUICKJS_ARRAY_PROTOTYPE_LAST_INDEX_OF,
        },
        DifferentialCase {
            id: "array_prototype_reverse",
            path: PATH_QUICKJS_ARRAY_PROTOTYPE_REVERSE,
        },
        DifferentialCase {
            id: "array_prototype_shift_unshift",
            path: PATH_QUICKJS_ARRAY_PROTOTYPE_SHIFT_UNSHIFT,
        },
        DifferentialCase {
            id: "array_prototype_slice",
            path: PATH_QUICKJS_ARRAY_PROTOTYPE_SLICE,
        },
    ]
}

fn quickjs_operator_expression_cases() -> Vec<DifferentialCase> {
    vec![
        DifferentialCase {
            id: "unary_operators",
            path: PATH_QUICKJS_UNARY_OPERATORS,
        },
        DifferentialCase {
            id: "update_expressions",
            path: PATH_QUICKJS_UPDATE_EXPRESSIONS,
        },
        DifferentialCase {
            id: "compound_assignment",
            path: PATH_QUICKJS_COMPOUND_ASSIGNMENT,
        },
        DifferentialCase {
            id: "compound_assignment_extended",
            path: PATH_QUICKJS_COMPOUND_ASSIGNMENT_EXTENDED,
        },
        DifferentialCase {
            id: "nullish_logical_assignment",
            path: PATH_QUICKJS_NULLISH_LOGICAL_ASSIGNMENT,
        },
        DifferentialCase {
            id: "exponentiation_parentheses",
            path: PATH_QUICKJS_EXPONENTIATION_PARENTHESES,
        },
        DifferentialCase {
            id: "in_operator",
            path: PATH_QUICKJS_IN_OPERATOR,
        },
    ]
}

fn quickjs_runtime_cases() -> Vec<DifferentialCase> {
    vec![
        DifferentialCase {
            id: "reference_error_catch",
            path: PATH_QUICKJS_REFERENCE_ERROR_CATCH,
        },
        DifferentialCase {
            id: "error_object_properties",
            path: PATH_QUICKJS_ERROR_OBJECT_PROPERTIES,
        },
        DifferentialCase {
            id: "global_numeric_constants",
            path: PATH_QUICKJS_GLOBAL_NUMERIC_CONSTANTS,
        },
        DifferentialCase {
            id: "global_utility_builtins",
            path: PATH_QUICKJS_GLOBAL_UTILITY_BUILTINS,
        },
        DifferentialCase {
            id: "json_builtin",
            path: PATH_QUICKJS_JSON_BUILTIN,
        },
        DifferentialCase {
            id: "math_builtin",
            path: PATH_QUICKJS_MATH_BUILTIN,
        },
        DifferentialCase {
            id: "math_methods",
            path: PATH_QUICKJS_MATH_METHODS,
        },
        DifferentialCase {
            id: "math_integer_methods",
            path: PATH_QUICKJS_MATH_INTEGER_METHODS,
        },
        DifferentialCase {
            id: "math_random",
            path: PATH_QUICKJS_MATH_RANDOM,
        },
        DifferentialCase {
            id: "object_descriptors",
            path: PATH_QUICKJS_OBJECT_DESCRIPTORS,
        },
        DifferentialCase {
            id: "object_static_methods",
            path: PATH_QUICKJS_OBJECT_STATIC_METHODS,
        },
        DifferentialCase {
            id: "standard_error_constructors",
            path: PATH_QUICKJS_STANDARD_ERROR_CONSTRUCTORS,
        },
        DifferentialCase {
            id: "boolean_builtin",
            path: PATH_QUICKJS_BOOLEAN_BUILTIN,
        },
        DifferentialCase {
            id: "string_builtin",
            path: PATH_QUICKJS_STRING_BUILTIN,
        },
        DifferentialCase {
            id: "string_prototype_methods",
            path: PATH_QUICKJS_STRING_PROTOTYPE_METHODS,
        },
    ]
}
