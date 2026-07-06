use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn tracks_heap_strings_without_reallocating_repeated_runtime_strings() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    ensure_usize(vm.resource_usage().string_count, 0)?;
    ensure_usize(vm.resource_usage().string_bytes, 0)?;

    let typeof_value = vm.context().eval("typeof neverDeclared")?;
    ensure_value(&typeof_value, &Value::String("undefined".to_owned()))?;
    let after_typeof = vm.resource_usage();
    ensure_usize(after_typeof.string_count, 1)?;
    ensure_usize(after_typeof.string_bytes, "undefined".len())?;

    let repeated_typeof = vm.context().eval("typeof anotherMissing")?;
    ensure_value(&repeated_typeof, &Value::String("undefined".to_owned()))?;
    let after_repeated_typeof = vm.resource_usage();
    ensure_usize(
        after_repeated_typeof.string_count,
        after_typeof.string_count,
    )?;
    ensure_usize(
        after_repeated_typeof.string_bytes,
        after_typeof.string_bytes,
    )?;

    let concat_value = vm.context().eval(r#""front" + "-door""#)?;
    ensure_value(&concat_value, &Value::String("front-door".to_owned()))?;
    let after_concat = vm.resource_usage();
    ensure_usize(after_concat.string_count, 2)?;
    ensure_usize(
        after_concat.string_bytes,
        "undefined".len() + "front-door".len(),
    )?;

    let repeated_concat = vm.context().eval(r#""front" + "-door""#)?;
    ensure_value(&repeated_concat, &Value::String("front-door".to_owned()))?;
    let after_repeated_concat = vm.resource_usage();
    ensure_usize(
        after_repeated_concat.string_count,
        after_concat.string_count,
    )?;
    ensure_usize(
        after_repeated_concat.string_bytes,
        after_concat.string_bytes,
    )
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
