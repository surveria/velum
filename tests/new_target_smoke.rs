use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn direct_function_calls_observe_undefined_new_target() -> TestResult {
    let value = eval(
        r"
function Camera() {
    return new.target === undefined;
}
Camera();
",
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn constructor_calls_observe_constructor_new_target() -> TestResult {
    let value = eval(
        r"
function Camera() {
    this.ok = new.target === Camera;
}
var camera = new Camera();
camera.ok;
",
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn new_target_supports_member_access() -> TestResult {
    let value = eval(
        r"
function Camera() {
    this.name = new.target.name;
}
var camera = new Camera();
camera.name;
",
    )?;

    ensure_value(&value, &Value::from("Camera"))
}

#[test]
fn arrow_functions_capture_lexical_new_target() -> TestResult {
    let value = eval(
        r"
function Camera() {
    this.sameDuringConstruction = (() => new.target)() === Camera;
    this.sameAfterConstruction = () => new.target === Camera;
}
var camera = new Camera();
camera.sameDuringConstruction && camera.sameAfterConstruction();
",
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn top_level_new_target_is_rejected_during_parse() -> TestResult {
    let Err(error) = eval("new.target") else {
        return Err("expected top-level new.target to be rejected".into());
    };

    ensure_parse_error_contains(&error, "new.target")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual != expected {
        return Err(format!("expected {expected:?}, got {actual:?}").into());
    }
    Ok(())
}

fn ensure_parse_error_contains(error: &Error, expected: &str) -> TestResult {
    if error.to_string().contains("parser error") && error.to_string().contains(expected) {
        return Ok(());
    }
    Err(format!("expected parse error containing '{expected}', got {error}").into())
}
