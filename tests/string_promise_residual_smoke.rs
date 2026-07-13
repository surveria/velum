use rs_quickjs::{Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn promise_try_forwards_arguments_and_rejects_abrupt_completions() -> TestResult {
    ensure_after_setup(
        r#"
        let fulfilled = "pending";
        let rejected = "pending";
        Promise.try(function(first, second) {
            "use strict";
            return this === undefined ? first + second : 0;
        }, 19, 23).then(function(value) { fulfilled = value; });
        Promise.try(function() { throw "reason"; }).then(
            undefined,
            function(reason) { rejected = reason; }
        );
        "#,
        "fulfilled === 42 && rejected === 'reason' ? 42 : 0",
        &Value::Number(42.0),
    )
}

#[test]
fn promise_with_resolvers_returns_live_capability_record() -> TestResult {
    ensure_after_setup(
        r#"
        let capability = Promise.withResolvers();
        let keys = Object.keys(capability).join(",");
        let settled = "pending";
        capability.promise.then(function(value) { settled = value; });
        capability.resolve(42);
        "#,
        "keys === 'promise,resolve,reject' && settled === 42 ? 42 : 0",
        &Value::Number(42.0),
    )
}

#[test]
fn string_search_and_trim_use_ecmascript_abstract_operations() -> TestResult {
    ensure_eval(
        r#"
        let legacy = "\u180Evalue\u180E";
        let modern = "\uFEFF\u2028value\u2029\uFEFF";
        let pattern = /value/;
        pattern[Symbol.match] = false;
        let accepted = "value".includes(pattern) === false &&
            "value".startsWith(pattern) === false &&
            "value".endsWith(pattern) === false;
        let rejected = 0;
        try { "value".includes(/value/); } catch (error) {
            if (error instanceof TypeError) { rejected += 1; }
        }
        try { "value".startsWith(/value/); } catch (error) {
            if (error instanceof TypeError) { rejected += 1; }
        }
        try { "value".endsWith(/value/); } catch (error) {
            if (error instanceof TypeError) { rejected += 1; }
        }
        accepted && rejected === 3 && modern.trim() === "value" &&
            legacy.trim() === legacy ? 42 : 0
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

fn ensure_after_setup(setup: &str, check: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(setup)?;
    let actual = context.eval(check)?;
    if actual == *expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
