use rs_quickjs::{Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn promise_all_keyed_preserves_keys_and_settlement_order() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let firstResolve;
        let secondResolve;
        let observed = "pending";
        let combined = Promise.allKeyed({
            first: new Promise(function(resolve) { firstResolve = resolve; }),
            second: new Promise(function(resolve) { secondResolve = resolve; })
        });
        secondResolve(2);
        firstResolve(1);
        combined.then(function(result) {
            observed = Object.getPrototypeOf(result) === null &&
                Object.keys(result).join(",") === "first,second" &&
                result.first === 1 && result.second === 2 ? "ok" : "bad";
        });
        "#,
    )?;
    ensure_value(&context.eval("observed")?, &Value::String("ok".to_owned()))
}

#[test]
fn promise_all_settled_keyed_materializes_standard_entries() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let observed = "pending";
        Promise.allSettledKeyed({
            fulfilled: Promise.resolve(1),
            rejected: Promise.reject(2)
        }).then(function(result) {
            observed = result.fulfilled.status + ":" + result.fulfilled.value + "|" +
                result.rejected.status + ":" + result.rejected.reason;
        });
        "#,
    )?;
    ensure_value(
        &context.eval("observed")?,
        &Value::String("fulfilled:1|rejected:2".to_owned()),
    )
}

#[test]
fn keyed_combinators_filter_descriptors_and_preserve_symbols() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let symbol = Symbol("value");
        let input = { visible: Promise.resolve(1) };
        input[symbol] = Promise.resolve(2);
        Object.defineProperty(input, "hidden", {
            enumerable: false,
            value: Promise.resolve(3)
        });
        let observed = "pending";
        Promise.allKeyed(input).then(function(result) {
            let keys = Reflect.ownKeys(result);
            let descriptor = Object.getOwnPropertyDescriptor(result, "visible");
            observed = keys.length === 2 && keys[0] === "visible" && keys[1] === symbol &&
                result.visible === 1 && result[symbol] === 2 &&
                descriptor.writable && descriptor.enumerable && descriptor.configurable ?
                "ok" : "bad";
        });
        "#,
    )?;
    ensure_value(&context.eval("observed")?, &Value::String("ok".to_owned()))
}

#[test]
fn keyed_combinators_reject_invalid_and_abrupt_inputs() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let invalid = "pending";
        let abrupt = "pending";
        let marker = {};
        Promise.allKeyed(1).then(undefined, function(error) {
            invalid = error instanceof TypeError ? "type" : "bad";
        });
        let input = {};
        Object.defineProperty(input, "value", {
            enumerable: true,
            get: function() { throw marker; }
        });
        Promise.allSettledKeyed(input).then(undefined, function(error) {
            abrupt = error === marker ? "marker" : "bad";
        });
        "#,
    )?;
    ensure_value(
        &context.eval("invalid + '|' + abrupt")?,
        &Value::String("type|marker".to_owned()),
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
