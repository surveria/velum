use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const FUNCTION_CONSTRUCTOR_DIRECT_ARGS_SCRIPT: &str = r#"
var order = "";

var mark = function(label, value) {
    order = order + label;
    return value;
};

var add = Function(
    mark("a", "left"),
    mark("b", "right"),
    mark("c", "order = order + 'd'; return left + right;")
);
var sum = add(20, 22);

var made = new Function(
    mark("e", "value"),
    mark("f", "order = order + 'g'; return value + 1;")
);
var madeValue = made(41);

sum === 42 &&
    madeValue === 42 &&
    order === "abcdefg" ? 42 : 0
"#;

#[test]
fn function_constructor_calls_compile_to_direct_native_operands() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(FUNCTION_CONSTRUCTOR_DIRECT_ARGS_SCRIPT)?;

    ensure_at_least(
        script.usage().bytecode_direct_native_call_count(),
        2,
        "Function direct native call operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn function_constructor_preserves_argument_side_effects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(FUNCTION_CONSTRUCTOR_DIRECT_ARGS_SCRIPT)?;

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
