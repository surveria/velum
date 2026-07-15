use velum::{Runtime, Value};

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

#[test]
fn primitive_prototype_proxy_reads_keep_original_receiver() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let number = 7;
        let prototype = Object.getPrototypeOf(number);
        let proxy = new Proxy({}, {
            get(target, property, receiver) {
                return property === "receiverProbe" && receiver === number ? 42 : 0;
            }
        });
        Object.setPrototypeOf(prototype, proxy);
        number.receiverProbe
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn primitive_property_mutation_follows_set_and_delete_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let setterReceiver = false;
        Object.defineProperty(Number.prototype, "receiverProbe", {
            set(value) {
                "use strict";
                setterReceiver = this === 7 && value === 42;
            }
        });
        (7).receiverProbe = 42;

        let strictFailures = 0;
        try { (function() { "use strict"; (7).missing = 1; })(); } catch (error) {
            strictFailures += error.constructor === TypeError;
        }
        try { (function() { "use strict"; "foo".length = 1; })(); } catch (error) {
            strictFailures += error.constructor === TypeError;
        }
        try { (function() { "use strict"; delete "foo"[0]; })(); } catch (error) {
            strictFailures += error.constructor === TypeError;
        }

        let key = "missing";
        (setterReceiver ? 1 : 0) +
            (((7).missing = 2) === 2 ? 2 : 0) +
            (Reflect.set({}, key, 3, 7) === false ? 4 : 0) +
            (delete "foo".length === false ? 8 : 0) +
            (delete "foo"[0] === false ? 16 : 0) +
            (strictFailures === 3 ? 32 : 0)
        "#,
    )?;

    ensure_value(&value, &Value::Number(63.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
