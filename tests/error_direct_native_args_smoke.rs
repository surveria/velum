use velum::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const DIRECT_ERROR_TARGET_ARGS_SCRIPT: &str = r#"
let order = "";
let mark = function(label, value) {
    order = order + label;
    return value;
};

let run = function() {
    let plain = Error(mark("a", "plain"), mark("b", "unused"));
    let typed = TypeError(mark("c", "typed"), mark("d", "unused"));
    return plain.name === "Error" &&
        plain.message === "plain" &&
        typed.name === "TypeError" &&
        typed.message === "typed";
};

let first = run();
let second = run();

first && second && order === "abcdabcd" ? 42 : 0
"#;

#[test]
fn direct_error_targets_preserve_argument_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(DIRECT_ERROR_TARGET_ARGS_SCRIPT)?;

    ensure_at_least(
        script.usage().bytecode_direct_native_call_count(),
        2,
        "direct Error native call operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual >= expected {
        return Ok(());
    }

    Err(format!("expected {label} >= {expected}, got {actual}").into())
}
