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
fn captures_nested_new_constructor_before_arguments() -> TestResult {
    let value = eval(
        r"
        let Constructor = function() { this.value = 42; };
        let instance = new Constructor(Constructor = 1);
        let nestedThrows = false;
        try {
            new new Boolean(true);
        } catch (error) {
            nestedThrows = error instanceof TypeError;
        }
        Constructor === 1 && instance.value === 42 && nestedThrows
        ",
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn rejects_import_call_constructor_forms() -> TestResult {
    for source in [
        "new import('./empty_FIXTURE.js');",
        "new import.defer('./empty_FIXTURE.js');",
    ] {
        let Err(error) = eval(source) else {
            return Err(format!("expected import constructor form to fail: {source}").into());
        };
        let message = error.to_string();
        if !message.contains("import call cannot be used as a constructor") {
            return Err(format!("expected import constructor parse error, got {message}").into());
        }
    }
    let Err(error) = eval("new import.meta;") else {
        return Err("expected import.meta outside a module to fail".into());
    };
    if !error
        .to_string()
        .contains("import.meta is only valid in modules")
    {
        return Err(format!("unexpected import.meta parse error: {error}").into());
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
