use rs_quickjs::{Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_annex_b_escape_globals() -> TestResult {
    ensure_eval(
        r#"
        escape("A @/😀") === "A%20@/%uD83D%uDE00" &&
            unescape("A%20@/%uD83D%uDE00") === "A @/😀" &&
            escape.length === 1 && unescape.length === 1 &&
            escape.name === "escape" && unescape.name === "unescape" ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn json_applies_bigint_to_json_before_serialization() -> TestResult {
    ensure_eval(
        r#"
        BigInt.prototype.toJSON = function(key) {
            "use strict";
            return typeof this === "bigint" && key === "value" ? this.toString() : "bad";
        };
        JSON.stringify({ value: 7n }) === '{"value":"7"}' &&
            JSON.stringify(Object(8n)) === '"bad"' ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn json_observes_proxy_arrays_and_replacers() -> TestResult {
    ensure_eval(
        r#"
        let array = new Proxy([], {
            get(_target, key) {
                if (key === "length") return 2;
                return Number(key) + 1;
            }
        });
        let replacer = new Proxy(["b"], {});
        JSON.stringify(array) === "[1,2]" &&
            JSON.stringify({ a: 1, b: 2 }, replacer) === '{"b":2}' ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    if actual == *expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
