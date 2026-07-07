use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn supports_object_literal_shorthand_and_methods() -> TestResult {
    let value = eval(
        r#"
        let name = "front-door";
        let count = 40;
        let camera = {
            name,
            count,
            default: 1,
            7: 2,
            add(extra) {
                return this.count + extra;
            },
            nested() {
                return this.add(this[7]);
            },
        };
        ("prototype" in camera.add) ? 0 : camera.nested()
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_computed_object_literal_property_names() -> TestResult {
    let value = eval(
        r#"
        let order = "";
        function mark(name, value) {
            order = order + name;
            return value;
        }
        let object = {
            [mark("k", "front")]: mark("v", 40),
            [mark("n", "door")]: mark("w", 2),
        };
        order === "kvnw" && object.front + object.door === 42 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn computed_proto_object_literal_property_is_data_property() -> TestResult {
    let value = eval(
        r#"
        let object = { ["__proto__"]: 42, marker: 1 };
        object.__proto__ === 42 && !("inherited" in object) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_computed_symbol_object_literal_property_names() -> TestResult {
    let value = eval(
        r#"
        let key = Symbol("camera");
        let object = { [key]: 42 };
        object[key]
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_computed_object_literal_methods() -> TestResult {
    let value = eval(
        r#"
        let order = "";
        function mark(name, value) {
            order = order + name;
            return value;
        }
        let object = {
            value: 40,
            [mark("k", "read")](extra) {
                order = order + "m";
                return this.value + extra;
            },
            after: mark("a", 1),
        };
        order === "ka" &&
            object.read(2) === 42 &&
            order === "kam" &&
            object.read.name === "read" &&
            !("prototype" in object.read) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_computed_symbol_object_literal_methods() -> TestResult {
    let value = eval(
        r#"
        let key = Symbol("camera");
        let object = {
            value: 40,
            [key](extra) {
                return this.value + extra;
            },
        };
        object[key](2)
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_missing_shorthand_bindings() -> TestResult {
    let Err(error) = eval("let camera = { missing }; camera.missing") else {
        return Err("expected missing shorthand binding to fail".into());
    };
    let message = error.to_string();
    if message.contains("ReferenceError: 'missing' is not defined") {
        return Ok(());
    }
    Err(format!("expected ReferenceError, got '{message}'").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
