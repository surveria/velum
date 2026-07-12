use rs_quickjs::{Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn standard_error_prototypes_and_to_string_are_available() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let error = new TypeError("typed");
        let aggregate = new AggregateError([], "many");
        [
            error.toString(),
            Object.getPrototypeOf(error) === TypeError.prototype,
            Object.getPrototypeOf(TypeError.prototype) === Error.prototype,
            TypeError.prototype.toString === Error.prototype.toString,
            error instanceof Error,
            error instanceof TypeError,
            error instanceof SyntaxError,
            Error.prototype.toString.call({ name: "Custom", message: "message" }),
            Error.prototype.toString.call({ name: "", message: "message" }),
            Error.prototype.toString.call({ name: "OnlyName", message: "" }),
            aggregate.name + ":" + aggregate.message + ":" + AggregateError.length,
        ].join("|")
        "#,
    )?;

    ensure_value(
        &value,
        "TypeError: typed|true|true|true|true|true|false|Custom: message|message|OnlyName|AggregateError:many:2",
    )
}

#[test]
fn error_prototype_properties_are_non_enumerable() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        [
            Object.keys(Error.prototype).join(","),
            Object.getOwnPropertyDescriptor(Error.prototype, "name").enumerable,
            Object.getOwnPropertyDescriptor(Error.prototype, "message").enumerable,
            Object.getOwnPropertyDescriptor(Error.prototype, "toString").enumerable,
            "toString" in new RangeError("range"),
            Object.getPrototypeOf(new RangeError("range")) === RangeError.prototype,
        ].join("|")
        "#,
    )?;

    ensure_value(&value, "|false|false|false|true|true")
}

fn ensure_value(value: &Value, expected: &str) -> TestResult {
    let actual = match value {
        Value::String(value) => value.as_str(),
        _ => return Err(format!("expected string value, got {value:?}").into()),
    };
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected '{expected}', got '{actual}'").into())
}
