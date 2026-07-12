use rs_quickjs::{Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn promise_finally_preserves_fulfillment_and_rejection() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let fulfilled = "pending";
        let rejected = "pending";
        let overridden = "pending";
        Promise.resolve("value").finally(function() { return "ignored"; }).then(function(value) {
            fulfilled = value;
        });
        Promise.reject("reason").finally(function() { return "ignored"; }).then(
            undefined,
            function(reason) { rejected = reason; }
        );
        Promise.resolve("value").finally(function() {
            return Promise.reject("override");
        }).then(undefined, function(reason) { overridden = reason; });
        "#,
    )?;
    let actual = context.eval("fulfilled + '|' + rejected + '|' + overridden")?;
    ensure_value(&actual, &Value::String("value|reason|override".to_owned()))
}

#[test]
fn promise_finally_invokes_dynamic_then_with_standard_handlers() -> TestResult {
    ensure_eval(
        r#"
        let target = Promise.resolve(1);
        let marker = {};
        let observed = "";
        target.then = function(onFulfilled, onRejected) {
            observed = this === target && arguments.length === 2 &&
                onFulfilled.length === 1 && onRejected.length === 1 &&
                onFulfilled.name === "" && onRejected.name === "" ? "ok" : "bad";
            return marker;
        };
        let result = Promise.prototype.finally.call(target, function() {});
        observed === "ok" && result === marker ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn promise_finally_passes_non_callable_handlers_through() -> TestResult {
    ensure_eval(
        r"
        let target = Promise.resolve(1);
        let marker = {};
        let observed = false;
        target.then = function(onFulfilled, onRejected) {
            observed = arguments.length === 2 && onFulfilled === 7 && onRejected === 7;
            return marker;
        };
        Promise.prototype.finally.call(target, 7) === marker && observed ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn promise_finally_honors_species_constructors() -> TestResult {
    ensure_eval(
        r"
        class DefaultPromise extends Promise {}
        class IntrinsicPromise extends Promise {
            static get [Symbol.species]() { return Promise; }
        }
        let defaultResult = DefaultPromise.resolve(1).finally(function() {});
        let intrinsicResult = IntrinsicPromise.resolve(1).finally(function() {});
        defaultResult instanceof DefaultPromise &&
            intrinsicResult instanceof Promise &&
            !(intrinsicResult instanceof IntrinsicPromise) ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn promise_catch_observably_invokes_then() -> TestResult {
    ensure_eval(
        r#"
        let target = Promise.reject("reason");
        let marker = {};
        let observed = false;
        target.then = function(onFulfilled, onRejected) {
            observed = onFulfilled === undefined && typeof onRejected === "function";
            return marker;
        };
        target.catch(function() {}) === marker && observed ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    ensure_value(&actual, expected)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
