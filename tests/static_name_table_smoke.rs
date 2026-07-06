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

#[test]
fn compiled_script_deduplicates_static_names() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(STATIC_NAME_TABLE_SOURCE)?;

    ensure_usize(script.usage().static_name_count(), 4)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(5.0))
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
