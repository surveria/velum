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

#[path = "cases_engine_runtime.rs"]
mod cases_engine_runtime;
#[path = "cases_reporting.rs"]
mod cases_reporting;
#[path = "cases_test262.rs"]
mod cases_test262;
#[path = "cases_test262_additions.rs"]
mod cases_test262_additions;
#[path = "cases_test262_collections.rs"]
mod cases_test262_collections;
#[path = "cases_test262_proxy.rs"]
mod cases_test262_proxy;
#[path = "cases_test262_reflect.rs"]
mod cases_test262_reflect;

pub use cases_reporting::{benchmark_cases, quickjs_differential_cases};
pub use cases_test262::test262_cases;

const PATH_ARITHMETIC: &str = "tests/engine_cases/arithmetic_precedence.js";
const PATH_HOST_PRINT: &str = "tests/engine_cases/host_print.js";
const PATH_STRING_ESCAPE_SEQUENCES: &str = "tests/engine_cases/string_escape_sequences.js";
const PATH_TEMPLATE_LITERALS: &str = "tests/engine_cases/template_literals.js";
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
const PATH_IN_OPERATOR: &str = "tests/engine_cases/in_operator.js";
const PATH_IN_OPERATOR_RHS_ERROR: &str = "tests/engine_cases/in_operator_rhs_error.js";
const PATH_WHILE_STATEMENTS: &str = "tests/engine_cases/while_statements.js";
const PATH_BREAK_CONTINUE: &str = "tests/engine_cases/break_continue.js";
const PATH_FOR_STATEMENTS: &str = "tests/engine_cases/for_statements.js";
const PATH_FOR_IN_STATEMENTS: &str = "tests/engine_cases/for_in_statements.js";
const PATH_FOR_IN_NULLISH_ERROR: &str = "tests/engine_cases/for_in_nullish_error.js";
const PATH_SWITCH_STATEMENTS: &str = "tests/engine_cases/switch_statements.js";
const PATH_BLOCK_LEXICAL_SCOPE: &str = "tests/engine_cases/block_lexical_scope.js";
const PATH_FUNCTION_EXPRESSION: &str = "tests/engine_cases/function_expression.js";
const PATH_FUNCTION_PROPERTIES: &str = "tests/engine_cases/function_properties.js";
const PATH_FUNCTION_CUSTOM_PROPERTIES: &str = "tests/engine_cases/function_custom_properties.js";
const PATH_FUNCTION_DESCRIPTORS: &str = "tests/engine_cases/function_descriptors.js";
const PATH_FUNCTION_INTRINSIC_DESCRIPTORS: &str =
    "tests/engine_cases/function_intrinsic_descriptors.js";
const PATH_METHOD_THIS: &str = "tests/engine_cases/method_this.js";
const PATH_CONSTRUCTOR_PROTOTYPES: &str = "tests/engine_cases/constructor_prototypes.js";
const PATH_PROTOTYPE_CONSTRUCTOR_PROPERTY: &str =
    "tests/engine_cases/prototype_constructor_property.js";
const PATH_FUNCTION_RETURN: &str = "tests/engine_cases/function_return.js";
const PATH_FUNCTION_PARAMETERS_SCOPE: &str = "tests/engine_cases/function_parameters_scope.js";
const PATH_CLOSURE_ENVIRONMENTS: &str = "tests/engine_cases/closure_environments.js";
const PATH_OBJECT_LITERALS: &str = "tests/engine_cases/object_literals.js";
const PATH_OBJECT_LITERAL_SHORTHAND_METHODS: &str =
    "tests/engine_cases/object_literal_shorthand_methods.js";
const PATH_OBJECT_PROTOTYPES: &str = "tests/engine_cases/object_prototypes.js";
const PATH_OBJECT_PROTOTYPE_ROOT: &str = "tests/engine_cases/object_prototype_root.js";
const PATH_OBJECT_BUILTIN: &str = "tests/engine_cases/object_builtin.js";
const PATH_NUMBER_BUILTIN: &str = "tests/engine_cases/number_builtin.js";
const PATH_STRING_BUILTIN: &str = "tests/engine_cases/string_builtin.js";
const PATH_COMPUTED_PROPERTIES: &str = "tests/engine_cases/computed_properties.js";
const PATH_ARRAY_LITERALS: &str = "tests/engine_cases/array_literals.js";
const PATH_ARRAY_BUILTIN: &str = "tests/engine_cases/array_builtin.js";
const PATH_ARRAY_PROTOTYPE_METHODS: &str = "tests/engine_cases/array_prototype_methods.js";
const PATH_ARRAY_PROTOTYPE_CONCAT: &str = "tests/engine_cases/array_prototype_concat.js";
const PATH_ARRAY_PROTOTYPE_INCLUDES: &str = "tests/engine_cases/array_prototype_includes.js";
const PATH_ARRAY_PROTOTYPE_JOIN: &str = "tests/engine_cases/array_prototype_join.js";
const PATH_ARRAY_PROTOTYPE_INDEX_OF: &str = "tests/engine_cases/array_prototype_index_of.js";
const PATH_ARRAY_PROTOTYPE_LAST_INDEX_OF: &str =
    "tests/engine_cases/array_prototype_last_index_of.js";
const PATH_ARRAY_PROTOTYPE_REVERSE: &str = "tests/engine_cases/array_prototype_reverse.js";
const PATH_ARRAY_PROTOTYPE_SHIFT_UNSHIFT: &str =
    "tests/engine_cases/array_prototype_shift_unshift.js";
const PATH_ARRAY_PROTOTYPE_SLICE: &str = "tests/engine_cases/array_prototype_slice.js";
const PATH_UNARY_OPERATORS: &str = "tests/engine_cases/unary_operators.js";

pub fn engine_cases() -> Vec<EngineCase> {
    let mut cases = engine_language_cases();
    cases.extend(engine_expression_cases());
    cases.extend(engine_control_flow_cases());
    cases.extend(engine_function_cases());
    cases.extend(engine_object_cases());
    cases.extend(cases_engine_runtime::engine_runtime_cases());
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
            id: "string_escape_sequences",
            path: PATH_STRING_ESCAPE_SEQUENCES,
            expectation: Expectation::OutputAndValue {
                output: &["ABC \"'\\ frontdoor"],
                value: "42",
            },
        },
        EngineCase {
            id: "template_literals",
            path: PATH_TEMPLATE_LITERALS,
            expectation: Expectation::Value("42"),
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
            id: "for_in_statements",
            path: PATH_FOR_IN_STATEMENTS,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "first:1;third:3;second:20; undefined",
                    "0=10;1=20;3=40; undefined",
                    "beta string beta",
                    "ac c",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "for_in_nullish_error",
            path: PATH_FOR_IN_NULLISH_ERROR,
            expectation: Expectation::ErrorContains("Cannot convert"),
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
        EngineCase {
            id: "in_operator",
            path: PATH_IN_OPERATOR,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "true true false",
                    "false",
                    "true true false true true",
                    "true true true",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "in_operator_rhs_error",
            path: PATH_IN_OPERATOR_RHS_ERROR,
            expectation: Expectation::ErrorContains("operator 'in'"),
        },
    ]
}

fn engine_function_cases() -> Vec<EngineCase> {
    let mut cases = engine_function_property_cases();
    cases.extend(engine_function_call_cases());
    cases
}

fn engine_function_property_cases() -> Vec<EngineCase> {
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
            id: "function_properties",
            path: PATH_FUNCTION_PROPERTIES,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "2 namedCamera 42",
                    "3 true",
                    "true true false true",
                    " 2 namedCamera",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "function_custom_properties",
            path: PATH_FUNCTION_CUSTOM_PROPERTIES,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "41 2 42 2 namedCamera",
                    "true true true false",
                    "beta:2;count:42;gamma:3;alpha:10;",
                    "false true",
                    "2 namedCamera",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "function_descriptors",
            path: PATH_FUNCTION_DESCRIPTORS,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "camera 1 tag true false false false",
                    "9 false namedCamera true 2 true",
                    "native 1 tag true false false false",
                    "keys true 1 true",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "function_intrinsic_descriptors",
            path: PATH_FUNCTION_INTRINSIC_DESCRIPTORS,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "namedCamera true 2 true",
                    "true false assigned 1 name",
                    "11 true false 2 length",
                    "TypeError 1 TypedAssigned true false 6 true false",
                ],
                value: "42",
            },
        },
    ]
}

fn engine_function_call_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "method_this",
            path: PATH_METHOD_THIS,
            expectation: Expectation::OutputAndValue {
                output: &["42 42 42 42 keyword"],
                value: "42",
            },
        },
        EngineCase {
            id: "constructor_prototypes",
            path: PATH_CONSTRUCTOR_PROTOTYPES,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "front side camera 42 42",
                    "true true true",
                    "name;count;kind;read;",
                    "42 42",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "prototype_constructor_property",
            path: PATH_PROTOTYPE_CONSTRUCTOR_PROPERTY,
            expectation: Expectation::OutputAndValue {
                output: &["keys:||constructor;", "true true true true"],
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
    let mut cases = engine_plain_object_cases();
    cases.extend(engine_array_cases());
    cases.extend(engine_unary_cases());
    cases
}

fn engine_plain_object_cases() -> Vec<EngineCase> {
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
            id: "object_literal_shorthand_methods",
            path: PATH_OBJECT_LITERAL_SHORTHAND_METHODS,
            expectation: Expectation::OutputAndValue {
                output: &["front-door 1 5 2 add false"],
                value: "42",
            },
        },
        EngineCase {
            id: "object_prototypes",
            path: PATH_OBJECT_PROTOTYPES,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "40 42 41 40",
                    "true true false",
                    "own;duplicate;shared;read;",
                    "undefined",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "object_prototype_root",
            path: PATH_OBJECT_PROTOTYPE_ROOT,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "root false true true",
                    "constructor true true true false",
                    "keys:||",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "object_builtin",
            path: PATH_OBJECT_BUILTIN,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "function Object 1 true",
                    "true true true true true",
                    "keys:|",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "number_builtin",
            path: PATH_NUMBER_BUILTIN,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "function Number 1 true",
                    "0 0 1 0 42 100 16 5 8",
                    "Infinity -Infinity NaN",
                    "keys:|",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "string_builtin",
            path: PATH_STRING_BUILTIN,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "function String 1 true",
                    " null undefined true false 42 [object Object]",
                    "6 c a 0 5 r",
                    "keys:||0;1;2;3;4;5;|0;1;",
                ],
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
    ]
}

fn engine_array_cases() -> Vec<EngineCase> {
    let mut cases = engine_array_core_cases();
    cases.extend(engine_array_prototype_cases());
    cases
}

fn engine_array_core_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "array_literals",
            path: PATH_ARRAY_LITERALS,
            expectation: Expectation::OutputAndValue {
                output: &["4 2 undefined", "7 4", "0 2"],
                value: "42",
            },
        },
        EngineCase {
            id: "array_builtin",
            path: PATH_ARRAY_BUILTIN,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "function Array 1 true",
                    "true true true true",
                    "0 0 2 front 42 3 undefined",
                    "keys:|",
                ],
                value: "42",
            },
        },
    ]
}

fn engine_array_prototype_cases() -> Vec<EngineCase> {
    let mut cases = engine_array_prototype_core_cases();
    cases.extend(engine_array_search_cases());
    cases.extend(engine_array_mutation_cases());
    cases.extend(engine_array_copy_cases());
    cases
}

fn engine_array_prototype_core_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "array_prototype_methods",
            path: PATH_ARRAY_PROTOTYPE_METHODS,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "methods function push 1 function pop 0",
                    "values 3 3 3 2 undefined 1 undefined 0 42",
                    "keys:|0;1;",
                    "in true true",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "array_prototype_join",
            path: PATH_ARRAY_PROTOTYPE_JOIN,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "join 1,two,,,true 1-two---true 1null2",
                    "sparse true |middle| 7 42 proto|",
                    "meta function join 1",
                    "keys:",
                    "in true",
                ],
                value: "42",
            },
        },
    ]
}

fn engine_array_search_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "array_prototype_includes",
            path: PATH_ARRAY_PROTOTYPE_INCLUDES,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "includes true true false false true true",
                    "values true true true true true true true false",
                    "sparse true true true true false",
                    "inherited true true 42 true",
                    "coerced true true true",
                    "meta function includes 1",
                    "keys:",
                    "in true",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "array_prototype_index_of",
            path: PATH_ARRAY_PROTOTYPE_INDEX_OF,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "indexOf 1 3 3 -1 -1 6",
                    "values 4 5 3 3 0",
                    "sparse -1 2 2 1",
                    "inherited 1 -1 42 0",
                    "coerced 1 0 0",
                    "meta function indexOf 1",
                    "keys:",
                    "in true",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "array_prototype_last_index_of",
            path: PATH_ARRAY_PROTOTYPE_LAST_INDEX_OF,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "lastIndexOf 3 1 3 -1 0 6",
                    "values 4 5 1 1 -1 -1",
                    "sparse -1 2 -1 2 1",
                    "inherited 1 -1 42 0",
                    "coerced 1 0 0",
                    "meta function lastIndexOf 1",
                    "keys:",
                    "in true",
                ],
                value: "42",
            },
        },
    ]
}

fn engine_array_mutation_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "array_prototype_reverse",
            path: PATH_ARRAY_PROTOTYPE_REVERSE,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "reverse true 4|3|2|1 4 true 3|2|1",
                    "side 42 true 7",
                    "sparse 4 three false one false three||one| true",
                    "inherited-upper true proto-two true undefined false",
                    "inherited-lower true undefined false proto-zero true",
                    "meta function reverse 0",
                    "keys:",
                    "in true",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "array_prototype_shift_unshift",
            path: PATH_ARRAY_PROTOTYPE_SHIFT_UNSHIFT,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "shift 1 2 2 3 undefined 42",
                    "sparse undefined 2 false undefined tail",
                    "inherited undefined 1 proto-one",
                    "unshift 3 3 3 1 2 3",
                    "holes 3 false a||b",
                    "inherited-unshift 2 head|proto-zero undefined",
                    "meta function shift 0 function unshift 1",
                    "keys:",
                    "in true true",
                ],
                value: "42",
            },
        },
    ]
}

fn engine_array_copy_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: "array_prototype_concat",
            path: PATH_ARRAY_PROTOTYPE_CONCAT,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "concat 6 1 2 3 4 5 true",
                    "source 2 1|2 3|4",
                    "side 42 7|8|9",
                    "sparse 6 zero false undefined one false undefined three tail zero||one||three|tail",
                    "inherited 2 proto-zero true own-one",
                    "prefix 4 own-zero own-one proto-two true own-three own-zero|own-one|proto-two|own-three",
                    "plain 2 1 true",
                    "meta function concat 1",
                    "keys:",
                    "in true",
                ],
                value: "42",
            },
        },
        EngineCase {
            id: "array_prototype_slice",
            path: PATH_ARRAY_PROTOTYPE_SLICE,
            expectation: Expectation::OutputAndValue {
                output: &[
                    "slice 2|3 2|3 3|4 0 0",
                    "source 4 1 2 3 4",
                    "sparse 3 one false undefined three one||three |one||three",
                    "inherited 3 undefined proto-one tail true",
                    "coerced 1|2 1 42 7",
                    "meta function slice 2",
                    "keys:",
                    "in true",
                ],
                value: "42",
            },
        },
    ]
}

fn engine_unary_cases() -> Vec<EngineCase> {
    vec![EngineCase {
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
    }]
}
