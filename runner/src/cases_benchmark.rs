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
const PATH_BENCH_FUNCTION_DESCRIPTORS: &str =
    "tests/corpora/benchmarks/active/function_descriptors.js";
const PATH_BENCH_FUNCTION_APPLY_HAS_INSTANCE: &str =
    "tests/corpora/benchmarks/active/function_apply_has_instance.js";
const PATH_BENCH_FUNCTION_INTRINSIC_DESCRIPTORS: &str =
    "tests/corpora/benchmarks/active/function_intrinsic_descriptors.js";
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
const PATH_BENCH_OBJECT_LITERAL_SHORTHAND_METHODS: &str =
    "tests/corpora/benchmarks/active/object_literal_shorthand_methods.js";
const PATH_BENCH_OBJECT_PROTOTYPES: &str = "tests/corpora/benchmarks/active/object_prototypes.js";
const PATH_BENCH_OBJECT_PROTOTYPE_ROOT: &str =
    "tests/corpora/benchmarks/active/object_prototype_root.js";
const PATH_BENCH_OBJECT_BUILTIN: &str = "tests/corpora/benchmarks/active/object_builtin.js";
const PATH_BENCH_OBJECT_PROTOTYPE_METHODS: &str =
    "tests/corpora/benchmarks/active/object_prototype_methods.js";
const PATH_BENCH_SET_OPERATIONS: &str = "tests/corpora/benchmarks/active/set_operations.js";
const PATH_BENCH_NUMBER_BUILTIN: &str = "tests/corpora/benchmarks/active/number_builtin.js";
const PATH_BENCH_NUMBER_FORMATTING: &str = "tests/corpora/benchmarks/active/number_formatting.js";
const PATH_BENCH_STRING_BUILTIN: &str = "tests/corpora/benchmarks/active/string_builtin.js";
const PATH_BENCH_STRING_REGEXP_INTEROP: &str =
    "tests/corpora/benchmarks/active/string_regexp_interop.js";
const PATH_BENCH_COMPUTED_PROPERTIES: &str =
    "tests/corpora/benchmarks/active/computed_properties.js";
const PATH_BENCH_ARRAY_LITERALS: &str = "tests/corpora/benchmarks/active/array_literals.js";
const PATH_BENCH_ARRAY_BUILTIN: &str = "tests/corpora/benchmarks/active/array_builtin.js";
const PATH_BENCH_ARRAY_PROTOTYPE_METHODS: &str =
    "tests/corpora/benchmarks/active/array_prototype_methods.js";
const PATH_BENCH_ARRAY_PROTOTYPE_CALLBACKS: &str =
    "tests/corpora/benchmarks/active/array_prototype_callbacks.js";
const PATH_BENCH_ARRAY_FLAT_FLATMAP: &str = "tests/corpora/benchmarks/active/array_flat_flatmap.js";
const PATH_BENCH_ARRAY_PROTOTYPE_CONCAT: &str =
    "tests/corpora/benchmarks/active/array_prototype_concat.js";
const PATH_BENCH_ARRAY_PROTOTYPE_INCLUDES: &str =
    "tests/corpora/benchmarks/active/array_prototype_includes.js";
const PATH_BENCH_ARRAY_PROTOTYPE_JOIN: &str =
    "tests/corpora/benchmarks/active/array_prototype_join.js";
const PATH_BENCH_ARRAY_PROTOTYPE_INDEX_OF: &str =
    "tests/corpora/benchmarks/active/array_prototype_index_of.js";
const PATH_BENCH_ARRAY_PROTOTYPE_LAST_INDEX_OF: &str =
    "tests/corpora/benchmarks/active/array_prototype_last_index_of.js";
const PATH_BENCH_ARRAY_PROTOTYPE_SORT: &str =
    "tests/corpora/benchmarks/active/array_prototype_sort.js";
const PATH_BENCH_ARRAY_PROTOTYPE_REVERSE: &str =
    "tests/corpora/benchmarks/active/array_prototype_reverse.js";
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
const PATH_BENCH_INSTANCEOF_HAS_INSTANCE: &str =
    "tests/corpora/benchmarks/active/instanceof_has_instance.js";
const PATH_BENCH_STRING: &str = "tests/corpora/benchmarks/active/string_concat.js";
const PATH_BENCH_STRING_ESCAPE_SEQUENCES: &str =
    "tests/corpora/benchmarks/active/string_escape_sequences.js";
const PATH_BENCH_BOOLEAN: &str = "tests/corpora/benchmarks/active/boolean_conversion.js";
const PATH_BENCH_BOOLEAN_BUILTIN: &str = "tests/corpora/benchmarks/active/boolean_builtin.js";
const PATH_BENCH_VAR_HOISTING: &str = "tests/corpora/benchmarks/active/var_hoisting.js";
const PATH_BENCH_TRY_CATCH: &str = "tests/corpora/benchmarks/active/try_catch.js";
const PATH_BENCH_TRY_FINALLY: &str = "tests/corpora/benchmarks/active/try_finally.js";
const PATH_BENCH_REFERENCE_ERROR_CATCH: &str =
    "tests/corpora/benchmarks/active/reference_error_catch.js";
const PATH_BENCH_ERROR_OBJECT_PROPERTIES: &str =
    "tests/corpora/benchmarks/active/error_object_properties.js";
const PATH_BENCH_GLOBAL_NUMERIC_CONSTANTS: &str =
    "tests/corpora/benchmarks/active/global_numeric_constants.js";
const PATH_BENCH_JSON_BUILTIN: &str = "tests/corpora/benchmarks/active/json_builtin.js";
const PATH_BENCH_REGEXP_BASELINE: &str = "tests/corpora/benchmarks/active/regexp_baseline.js";
const PATH_BENCH_MATH_BUILTIN: &str = "tests/corpora/benchmarks/active/math_builtin.js";
const PATH_BENCH_MATH_INTEGER_METHODS: &str =
    "tests/corpora/benchmarks/active/math_integer_methods.js";
const PATH_BENCH_MATH_METHODS: &str = "tests/corpora/benchmarks/active/math_methods.js";
const PATH_BENCH_MATH_RANDOM: &str = "tests/corpora/benchmarks/active/math_random.js";
const PATH_BENCH_STANDARD_ERROR_CONSTRUCTORS: &str =
    "tests/corpora/benchmarks/active/standard_error_constructors.js";
const PATH_BENCH_OBJECT_DESCRIPTORS: &str = "tests/corpora/benchmarks/active/object_descriptors.js";
const PATH_BENCH_COMPILED_SCRIPT_REUSE: &str =
    "tests/corpora/benchmarks/active/compiled_script_reuse.js";
const PATH_BENCH_ATOMIZED_BINDINGS: &str = "tests/corpora/benchmarks/active/atomized_bindings.js";
const PATH_BENCH_TYPED_ARRAY_RGBA_FILL_72P: &str =
    "tests/corpora/benchmarks/active/typed_array_rgba_fill_72p.js";
const PATH_BENCH_TYPED_ARRAY_RGBA_GRADIENT_72P: &str =
    "tests/corpora/benchmarks/active/typed_array_rgba_gradient_72p.js";
const PATH_BENCH_TYPED_ARRAY_RGBA_QUANTIZE_72P: &str =
    "tests/corpora/benchmarks/active/typed_array_rgba_quantize_72p.js";
const PATH_BENCH_TYPED_ARRAY_RGBA_BLUR_72P: &str =
    "tests/corpora/benchmarks/active/typed_array_rgba_blur_72p.js";
const PATH_BENCH_TYPED_ARRAY_RGBA_SHARPEN_72P: &str =
    "tests/corpora/benchmarks/active/typed_array_rgba_sharpen_72p.js";
const PATH_BENCH_TYPED_ARRAY_BULK_METHODS_72P: &str =
    "tests/corpora/benchmarks/active/typed_array_bulk_methods_72p.js";
const PATH_BENCH_SENTINEL_ARITHMETIC: &str =
    "tests/corpora/benchmarks/prepared/sentinel_arithmetic.js";
const PATH_BENCH_SENTINEL_ARRAY_INDEX: &str =
    "tests/corpora/benchmarks/prepared/sentinel_array_index.js";
const PATH_BENCH_SENTINEL_PROPERTY_READ: &str =
    "tests/corpora/benchmarks/prepared/sentinel_property_read.js";
const PATH_BENCH_SENTINEL_FUNCTION_CALL: &str =
    "tests/corpora/benchmarks/prepared/sentinel_function_call.js";
const PATH_BENCH_SENTINEL_STRING_SCAN: &str =
    "tests/corpora/benchmarks/prepared/sentinel_string_scan.js";

const IMAGE_72P_RGBA_BYTES: usize = 128 * 72 * 4;

pub fn benchmark_cases() -> Vec<BenchmarkCase> {
    let mut cases = benchmark_control_flow_cases();
    cases.extend(benchmark_function_cases());
    cases.extend(benchmark_object_cases());
    cases.extend(benchmark_array_cases());
    cases.extend(benchmark_typed_array_image_cases());
    cases.extend(benchmark_operator_cases());
    cases.extend(benchmark_runtime_cases());
    cases.extend(benchmark_prepared_sentinel_cases());
    cases
}

fn benchmark_typed_array_image_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase::cold(
            "typed_array_owned_rgba_fill_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_FILL_72P,
        ),
        BenchmarkCase::cold(
            "typed_array_owned_rgba_gradient_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_GRADIENT_72P,
        ),
        BenchmarkCase::cold(
            "typed_array_owned_rgba_quantize_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_QUANTIZE_72P,
        ),
        BenchmarkCase::cold(
            "typed_array_owned_rgba_blur_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_BLUR_72P,
        ),
        BenchmarkCase::cold(
            "typed_array_owned_rgba_sharpen_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_SHARPEN_72P,
        ),
        BenchmarkCase::cold(
            "typed_array_bulk_methods_72p",
            PATH_BENCH_TYPED_ARRAY_BULK_METHODS_72P,
        ),
        BenchmarkCase::cold_host_image(
            "typed_array_host_rgba_fill_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_FILL_72P,
            IMAGE_72P_RGBA_BYTES,
        ),
        BenchmarkCase::cold_host_image(
            "typed_array_host_rgba_gradient_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_GRADIENT_72P,
            IMAGE_72P_RGBA_BYTES,
        ),
        BenchmarkCase::cold_host_image(
            "typed_array_host_rgba_quantize_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_QUANTIZE_72P,
            IMAGE_72P_RGBA_BYTES,
        ),
        BenchmarkCase::cold_host_image(
            "typed_array_host_rgba_blur_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_BLUR_72P,
            IMAGE_72P_RGBA_BYTES,
        ),
        BenchmarkCase::cold_host_image(
            "typed_array_host_rgba_sharpen_72p",
            PATH_BENCH_TYPED_ARRAY_RGBA_SHARPEN_72P,
            IMAGE_72P_RGBA_BYTES,
        ),
    ]
}

fn benchmark_control_flow_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase::cold("arithmetic_chain", PATH_BENCH_ARITHMETIC),
        BenchmarkCase::cold("conditional_bitand", PATH_BENCH_CONDITIONAL_BITAND),
        BenchmarkCase::cold("while_statements", PATH_BENCH_WHILE_STATEMENTS),
        BenchmarkCase::cold("break_continue", PATH_BENCH_BREAK_CONTINUE),
        BenchmarkCase::cold("for_statements", PATH_BENCH_FOR_STATEMENTS),
        BenchmarkCase::cold("for_in_statements", PATH_BENCH_FOR_IN_STATEMENTS),
        BenchmarkCase::cold("switch_statements", PATH_BENCH_SWITCH_STATEMENTS),
        BenchmarkCase::cold("block_lexical_scope", PATH_BENCH_BLOCK_LEXICAL_SCOPE),
    ]
}

fn benchmark_function_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase::cold("function_expression", PATH_BENCH_FUNCTION_EXPRESSION),
        BenchmarkCase::cold("function_properties", PATH_BENCH_FUNCTION_PROPERTIES),
        BenchmarkCase::cold(
            "function_custom_properties",
            PATH_BENCH_FUNCTION_CUSTOM_PROPERTIES,
        ),
        BenchmarkCase::cold("function_descriptors", PATH_BENCH_FUNCTION_DESCRIPTORS),
        BenchmarkCase::cold(
            "function_apply_has_instance",
            PATH_BENCH_FUNCTION_APPLY_HAS_INSTANCE,
        ),
        BenchmarkCase::cold(
            "function_intrinsic_descriptors",
            PATH_BENCH_FUNCTION_INTRINSIC_DESCRIPTORS,
        ),
        BenchmarkCase::cold("method_this", PATH_BENCH_METHOD_THIS),
        BenchmarkCase::cold("constructor_prototypes", PATH_BENCH_CONSTRUCTOR_PROTOTYPES),
        BenchmarkCase::cold(
            "prototype_constructor_property",
            PATH_BENCH_PROTOTYPE_CONSTRUCTOR_PROPERTY,
        ),
        BenchmarkCase::cold("function_return", PATH_BENCH_FUNCTION_RETURN),
        BenchmarkCase::cold(
            "function_parameters_scope",
            PATH_BENCH_FUNCTION_PARAMETERS_SCOPE,
        ),
        BenchmarkCase::cold("closure_environments", PATH_BENCH_CLOSURE_ENVIRONMENTS),
    ]
}

fn benchmark_object_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase::cold("object_literals", PATH_BENCH_OBJECT_LITERALS),
        BenchmarkCase::cold(
            "object_literal_shorthand_methods",
            PATH_BENCH_OBJECT_LITERAL_SHORTHAND_METHODS,
        ),
        BenchmarkCase::cold("object_prototypes", PATH_BENCH_OBJECT_PROTOTYPES),
        BenchmarkCase::cold("object_prototype_root", PATH_BENCH_OBJECT_PROTOTYPE_ROOT),
        BenchmarkCase::cold("object_builtin", PATH_BENCH_OBJECT_BUILTIN),
        BenchmarkCase::cold(
            "object_prototype_methods",
            PATH_BENCH_OBJECT_PROTOTYPE_METHODS,
        ),
        BenchmarkCase::cold("set_operations", PATH_BENCH_SET_OPERATIONS),
        BenchmarkCase::cold("number_builtin", PATH_BENCH_NUMBER_BUILTIN),
        BenchmarkCase::cold("number_formatting", PATH_BENCH_NUMBER_FORMATTING),
        BenchmarkCase::cold("computed_properties", PATH_BENCH_COMPUTED_PROPERTIES),
    ]
}

fn benchmark_array_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase::cold("array_literals", PATH_BENCH_ARRAY_LITERALS),
        BenchmarkCase::cold("array_builtin", PATH_BENCH_ARRAY_BUILTIN),
        BenchmarkCase::cold(
            "array_prototype_methods",
            PATH_BENCH_ARRAY_PROTOTYPE_METHODS,
        ),
        BenchmarkCase::cold(
            "array_prototype_callbacks",
            PATH_BENCH_ARRAY_PROTOTYPE_CALLBACKS,
        ),
        BenchmarkCase::cold("array_flat_flatmap", PATH_BENCH_ARRAY_FLAT_FLATMAP),
        BenchmarkCase::cold("array_prototype_join", PATH_BENCH_ARRAY_PROTOTYPE_JOIN),
        BenchmarkCase::cold("array_prototype_concat", PATH_BENCH_ARRAY_PROTOTYPE_CONCAT),
        BenchmarkCase::cold(
            "array_prototype_includes",
            PATH_BENCH_ARRAY_PROTOTYPE_INCLUDES,
        ),
        BenchmarkCase::cold(
            "array_prototype_index_of",
            PATH_BENCH_ARRAY_PROTOTYPE_INDEX_OF,
        ),
        BenchmarkCase::cold(
            "array_prototype_last_index_of",
            PATH_BENCH_ARRAY_PROTOTYPE_LAST_INDEX_OF,
        ),
        BenchmarkCase::cold(
            "array_prototype_reverse",
            PATH_BENCH_ARRAY_PROTOTYPE_REVERSE,
        ),
        BenchmarkCase::cold("array_prototype_sort", PATH_BENCH_ARRAY_PROTOTYPE_SORT),
        BenchmarkCase::cold(
            "array_prototype_shift_unshift",
            PATH_BENCH_ARRAY_PROTOTYPE_SHIFT_UNSHIFT,
        ),
        BenchmarkCase::cold("array_prototype_slice", PATH_BENCH_ARRAY_PROTOTYPE_SLICE),
    ]
}

fn benchmark_operator_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase::cold("unary_operators", PATH_BENCH_UNARY_OPERATORS),
        BenchmarkCase::cold("update_expressions", PATH_BENCH_UPDATE_EXPRESSIONS),
        BenchmarkCase::cold("compound_assignment", PATH_BENCH_COMPOUND_ASSIGNMENT),
        BenchmarkCase::cold(
            "compound_assignment_extended",
            PATH_BENCH_COMPOUND_ASSIGNMENT_EXTENDED,
        ),
        BenchmarkCase::cold(
            "exponentiation_parentheses",
            PATH_BENCH_EXPONENTIATION_PARENTHESES,
        ),
        BenchmarkCase::cold("in_operator", PATH_BENCH_IN_OPERATOR),
        BenchmarkCase::cold(
            "instanceof_has_instance",
            PATH_BENCH_INSTANCEOF_HAS_INSTANCE,
        ),
    ]
}

fn benchmark_runtime_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase::cold("string_concat", PATH_BENCH_STRING),
        BenchmarkCase::cold(
            "string_escape_sequences",
            PATH_BENCH_STRING_ESCAPE_SEQUENCES,
        ),
        BenchmarkCase::cold("string_builtin", PATH_BENCH_STRING_BUILTIN),
        BenchmarkCase::cold("string_regexp_interop", PATH_BENCH_STRING_REGEXP_INTEROP),
        BenchmarkCase::cold("boolean_conversion", PATH_BENCH_BOOLEAN),
        BenchmarkCase::cold("boolean_builtin", PATH_BENCH_BOOLEAN_BUILTIN),
        BenchmarkCase::cold("var_hoisting", PATH_BENCH_VAR_HOISTING),
        BenchmarkCase::cold("try_catch", PATH_BENCH_TRY_CATCH),
        BenchmarkCase::cold("try_finally", PATH_BENCH_TRY_FINALLY),
        BenchmarkCase::cold("reference_error_catch", PATH_BENCH_REFERENCE_ERROR_CATCH),
        BenchmarkCase::cold(
            "error_object_properties",
            PATH_BENCH_ERROR_OBJECT_PROPERTIES,
        ),
        BenchmarkCase::cold(
            "global_numeric_constants",
            PATH_BENCH_GLOBAL_NUMERIC_CONSTANTS,
        ),
        BenchmarkCase::cold("json_builtin", PATH_BENCH_JSON_BUILTIN),
        BenchmarkCase::cold("regexp_baseline", PATH_BENCH_REGEXP_BASELINE),
        BenchmarkCase::cold("math_builtin", PATH_BENCH_MATH_BUILTIN),
        BenchmarkCase::cold("math_methods", PATH_BENCH_MATH_METHODS),
        BenchmarkCase::cold("math_integer_methods", PATH_BENCH_MATH_INTEGER_METHODS),
        BenchmarkCase::cold("math_random", PATH_BENCH_MATH_RANDOM),
        BenchmarkCase::cold("object_descriptors", PATH_BENCH_OBJECT_DESCRIPTORS),
        BenchmarkCase::cold(
            "standard_error_constructors",
            PATH_BENCH_STANDARD_ERROR_CONSTRUCTORS,
        ),
        BenchmarkCase::cold("compiled_script_reuse", PATH_BENCH_COMPILED_SCRIPT_REUSE),
        BenchmarkCase::cold("atomized_bindings", PATH_BENCH_ATOMIZED_BINDINGS),
    ]
}

fn benchmark_prepared_sentinel_cases() -> Vec<BenchmarkCase> {
    vec![
        BenchmarkCase::prepared_sentinel("sentinel_arithmetic", PATH_BENCH_SENTINEL_ARITHMETIC),
        BenchmarkCase::prepared_sentinel("sentinel_array_index", PATH_BENCH_SENTINEL_ARRAY_INDEX),
        BenchmarkCase::prepared_sentinel(
            "sentinel_property_read",
            PATH_BENCH_SENTINEL_PROPERTY_READ,
        ),
        BenchmarkCase::prepared_sentinel(
            "sentinel_function_call",
            PATH_BENCH_SENTINEL_FUNCTION_CALL,
        ),
        BenchmarkCase::prepared_sentinel("sentinel_string_scan", PATH_BENCH_SENTINEL_STRING_SCAN),
    ]
}
