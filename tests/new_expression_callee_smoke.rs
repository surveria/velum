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
fn rejects_import_call_constructor_forms() -> TestResult {
    let Err(error) = eval("new import.defer('./empty_FIXTURE.js');") else {
        return Err("expected import constructor form to fail during parsing".into());
    };

    let message = error.to_string();
    if message.contains("import call cannot be used as a constructor") {
        return Ok(());
    }

    Err(format!("expected import constructor parse error, got {message}").into())
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
