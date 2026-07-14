use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_vm_global_object_through_global_this() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let original = globalThis;
        let descriptor = Object.getOwnPropertyDescriptor(this, "globalThis");
        let names = Object.getOwnPropertyNames(this);
        let hasBuiltinProperties =
            globalThis.Object === Object &&
            globalThis.Array === Array &&
            globalThis.Math === Math &&
            globalThis.parseInt === parseInt;
        globalThis.camera = "front";
        let propertyRoundTrip = this.camera === "front" && globalThis.camera === "front";
        let shadow = (function() {
            let globalThis = 17;
            return globalThis;
        })();
        globalThis = { replacement: 42 };
        let assignmentRoundTrip =
            this.globalThis.replacement === 42 &&
            globalThis.replacement === 42;
        this.globalThis = original;

        print(
            this === globalThis,
            globalThis.globalThis === globalThis,
            descriptor.writable,
            descriptor.enumerable,
            descriptor.configurable,
            names.includes("globalThis"),
            hasBuiltinProperties,
            propertyRoundTrip,
            shadow,
            assignmentRoundTrip,
            this === original
        );

        this === globalThis &&
            globalThis.globalThis === globalThis &&
            descriptor.writable === true &&
            descriptor.enumerable === false &&
            descriptor.configurable === true &&
            names.includes("globalThis") &&
            hasBuiltinProperties &&
            propertyRoundTrip &&
            shadow === 17 &&
            assignmentRoundTrip &&
            this === original ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["true true true false true true true true 17 true true".to_owned()],
    )
}

#[test]
fn keeps_global_this_isolated_per_context() -> TestResult {
    let runtime = Runtime::new();
    let mut first = runtime.context();
    let mut second = runtime.context();

    let first_value = first.eval(
        r#"
        globalThis.shared = "first";
        this.shared === "first" ? 42 : 0
        "#,
    )?;
    let second_value = second.eval(
        r#"
        typeof globalThis.shared === "undefined" ? 42 : 0
        "#,
    )?;

    ensure_value(&first_value, &Value::Number(42.0))?;
    ensure_value(&second_value, &Value::Number(42.0))
}

#[test]
fn lists_unmaterialized_standard_global_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let names = Object.getOwnPropertyNames(globalThis);
        let expected = [
            "NaN", "Infinity", "undefined", "eval", "parseInt", "parseFloat",
            "isNaN", "isFinite", "decodeURI", "decodeURIComponent", "encodeURI",
            "encodeURIComponent", "Object", "Function", "Array", "String",
            "Boolean", "Number", "Date", "RegExp", "Error", "EvalError",
            "RangeError", "ReferenceError", "SyntaxError", "TypeError", "URIError",
            "Math", "JSON"
        ];
        expected.every(function (name) { return names.includes(name); }) ? 42 : 0
        "#,
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
