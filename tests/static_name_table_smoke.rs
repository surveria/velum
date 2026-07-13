use std::fmt::Write as _;

use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const STATIC_NAME_TABLE_SOURCE: &str = r"
let value = 1;
let total = value + value;
let record = {
    value,
    total,
    method() {
        return value + this.value;
    },
};
record.value + record.total + record.method();
";

const OUT_OF_ORDER_STATIC_NAME_SOURCE: &str = r"
let zeta = 1;
let alpha = zeta + 1;
let middle = alpha + zeta;
middle + alpha + zeta;
";

const FOR_STATEMENT_SOURCE: &str = r"
for (let index = 0; index < 2; index = index + 1) {
    index;
}
";

const REPEATED_STATIC_STRING_SOURCE: &str = r#"
let first = "shared \u03c0";
let second = "shared \u03c0";
first + second;
"#;

const DISTINCT_STATIC_VALUES: usize = 1_000;

#[test]
fn compiled_script_deduplicates_static_names() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(STATIC_NAME_TABLE_SOURCE)?;

    ensure_usize(script.usage().static_name_count(), 4)?;
    ensure_usize(script.usage().static_binding_count(), 11)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(5.0))
}

#[test]
fn compiled_script_reuses_out_of_order_static_names() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(OUT_OF_ORDER_STATIC_NAME_SOURCE)?;

    ensure_usize(script.usage().static_name_count(), 3)?;
    ensure_usize(script.usage().static_binding_count(), 9)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(6.0))
}

#[test]
fn for_statement_checkpoint_does_not_keep_speculative_bindings() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();
    let script = vm.compile(FOR_STATEMENT_SOURCE)?;

    ensure_usize(script.usage().static_name_count(), 1)?;
    ensure_usize(script.usage().static_binding_count(), 5)
}

#[test]
fn compiled_script_deduplicates_utf16_static_strings() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();
    let script = vm.compile(REPEATED_STATIC_STRING_SOURCE)?;

    ensure_usize(script.usage().static_string_count(), 1)
}

#[test]
fn compiles_many_distinct_names_and_strings_without_duplicate_owners() -> TestResult {
    let mut source = String::new();
    for index in 0..DISTINCT_STATIC_VALUES {
        writeln!(
            source,
            r#"let distinct_name_{index} = "distinct_string_{index}_\u03c0";"#
        )?;
    }

    let engine = Engine::new();
    let vm = engine.create_vm();
    let script = vm.compile(&source)?;

    ensure_usize(script.usage().static_name_count(), DISTINCT_STATIC_VALUES)?;
    ensure_usize(script.usage().static_string_count(), DISTINCT_STATIC_VALUES)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
