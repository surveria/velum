use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn constructs_member_expression_callees() -> TestResult {
    let value = eval(
        r#"
let ns = {};
ns.Camera = function Camera(name) {
    this.name = name;
};
let camera = new ns.Camera("front");
camera.name === "front" && camera.__proto__ === ns.Camera.prototype;
"#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn constructs_computed_member_expression_callees() -> TestResult {
    let value = eval(
        r#"
let ns = {};
ns.Camera = function Camera(name) {
    this.name = name;
};
let key = "Camera";
let camera = new ns[key]("side");
camera.name === "side" && camera.__proto__ === ns.Camera.prototype;
"#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn constructs_parenthesized_function_value_callees() -> TestResult {
    let value = eval(
        r#"
let Used;
let make = function() {
    Used = function Camera(name) {
        this.name = name;
    };
    return Used;
};
let camera = new (make())("roof");
camera.name === "roof" && camera.__proto__ === Used.prototype;
"#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn supports_call_suffix_after_new_expression() -> TestResult {
    let value = eval(r#"new Function("return 42")()"#)?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_import_call_constructor_forms() -> TestResult {
    for source in [
        "new import('./empty_FIXTURE.js');",
        "new import.defer('./empty_FIXTURE.js');",
        "new import.meta;",
    ] {
        let Err(error) = eval(source) else {
            return Err(format!("expected import constructor form to fail: {source}").into());
        };
        let message = error.to_string();
        if !message.contains("import call cannot be used as a constructor") {
            return Err(format!("expected import constructor parse error, got {message}").into());
        }
    }
    Ok(())
}

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
