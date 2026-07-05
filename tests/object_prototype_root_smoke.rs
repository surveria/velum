use rs_quickjs::{Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn uses_object_prototype_as_default_object_root() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = {};
        let root = object.__proto__;
        let Camera = function Camera() {};
        let cameraRoot = Camera.prototype.__proto__;
        let nullProto = { __proto__: null };
        let primitiveProto = { __proto__: 7 };

        let rootKeys = "";
        for (let key in root) {
            rootKeys = rootKeys + key + ";";
        }

        let objectKeys = "";
        for (let key in object) {
            objectKeys = objectKeys + key + ";";
        }

        let deleted = delete Camera.prototype.constructor;
        let cameraKeys = "";
        for (let key in Camera.prototype) {
            cameraKeys = cameraKeys + key + ";";
        }

        print("root", object.__proto__ === null, root.__proto__ === null, cameraRoot === root);
        print(
            "constructor",
            "constructor" in object,
            "constructor" in Camera.prototype,
            "constructor" in primitiveProto,
            "constructor" in nullProto
        );
        print("keys:" + rootKeys + "|" + objectKeys + "|" + cameraKeys);

        object.__proto__ !== null &&
            root.__proto__ === null &&
            cameraRoot === root &&
            deleted &&
            ("constructor" in object) &&
            ("constructor" in Camera.prototype) &&
            ("constructor" in primitiveProto) &&
            !("constructor" in nullProto) &&
            nullProto.__proto__ === null &&
            rootKeys === "" &&
            objectKeys === "" &&
            cameraKeys === "" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "root false true true".to_owned(),
            "constructor true true true false".to_owned(),
            "keys:||".to_owned(),
        ],
    )
}

#[test]
fn preserves_null_prototype_literal_without_allocating_default_root() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_objects: 1,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let object = { __proto__: null };
        object.__proto__ === null && !('constructor' in object) ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
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
