use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn primitive_prototype_accessors_keep_original_receiver() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function receiverMarker(expected, prototype) {
            return function() {
                return this === prototype ? "bad" : expected;
            };
        }

        Object.defineProperty(Boolean.prototype, "receiverProbe", {
            get: receiverMarker("boolean", Boolean.prototype)
        });
        Object.defineProperty(Number.prototype, "receiverProbe", {
            get: receiverMarker("number", Number.prototype)
        });
        Object.defineProperty(String.prototype, "receiverProbe", {
            get: receiverMarker("string", String.prototype)
        });
        Object.defineProperty(Symbol.prototype, "receiverProbe", {
            get: receiverMarker("symbol", Symbol.prototype)
        });

        let symbol = Symbol("slot");
        true.receiverProbe === "boolean" &&
            (7).receiverProbe === "number" &&
            "text".receiverProbe === "string" &&
            symbol.receiverProbe === "symbol" ? 42 : 0
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
