use velum::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> velum::Result<Value> {
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
fn nested_direct_eval_inherits_new_target() -> TestResult {
    let value = eval(
        r#"
        function direct(expected) {
            return eval("new.target") === expected;
        }
        function nested(expected) {
            return eval('eval("new.target")') === expected;
        }
        function arrow(expected) {
            return (() => eval("new.target"))() === expected;
        }
        function capture(label, action) {
            try { return label + ":" + action(); }
            catch (error) { return label + ":" + error.name; }
        }
        [
            capture("direct", () => direct(undefined)),
            capture("nested", () => nested(undefined)),
            capture("arrow", () => arrow(undefined)),
            capture("new-direct", () => { new direct(direct); return true; }),
            capture("new-nested", () => { new nested(nested); return true; }),
            capture("new-arrow", () => { new arrow(arrow); return true; })
        ].join(";");
        "#,
    )?;

    ensure_value(
        &value,
        &Value::from(
            "direct:true;nested:true;arrow:true;new-direct:true;new-nested:true;new-arrow:true",
        ),
    )
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
