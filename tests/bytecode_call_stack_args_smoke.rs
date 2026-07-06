use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const BYTECODE_CALL_STACK_ARGS_SOURCE: &str = r#"
var order = "";
var mark = function (label, value) {
    order = order + label;
    return value;
};

var call = String;
var directBindingOk = String(mark("a", "front"), mark("b", "unused")) === "front";
var cachedBindingOk = call(mark("c", "side"), mark("d", "unused")) === "side";
var staticMemberOk = Math.max(mark("e", 3), mark("f", 8), mark("g", 1)) === 8;
var object = new String(mark("h", "go"), mark("i", "unused"));
print(mark("j", "line"), mark("k", "tail"));

directBindingOk &&
    cachedBindingOk &&
    staticMemberOk &&
    object.length === 2 &&
    order === "abcdefghijk" ? 42 : 0;
"#;

#[test]
fn bytecode_calls_reuse_stack_tail_for_arguments() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_CALL_STACK_ARGS_SOURCE)?;

    ensure_positive(
        script.usage().bytecode_direct_native_call_count(),
        "direct native calls",
    )?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(vm.take_output().as_slice(), &["line tail".to_owned()])?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(vm.take_output().as_slice(), &["line tail".to_owned()])
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_positive(value: usize, label: &str) -> TestResult {
    if value > 0 {
        return Ok(());
    }
    Err(format!("expected positive {label}, got {value}").into())
}
