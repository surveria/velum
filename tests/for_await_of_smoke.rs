use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> velum::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_string_after(source: &str, expression: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)?;
    ensure_value(&context.eval(expression)?, &Value::from(expected))
}

#[test]
fn consumes_sync_iterables_and_awaits_each_value() -> TestResult {
    ensure_string_after(
        r#"
        let trace = "";
        async function consume() {
            for await (const value of [Promise.resolve(20), 22]) {
                trace = trace + ":" + value;
            }
        }
        consume();
        "#,
        "trace",
        ":20:22",
    )
}

#[test]
fn consumes_the_async_iterator_protocol() -> TestResult {
    ensure_string_after(
        r#"
        let trace = "";
        const iterable = {};
        iterable[Symbol.asyncIterator] = function() {
            let index = 0;
            return {
                next: function() {
                    index = index + 1;
                    return Promise.resolve({ value: index * 10, done: index > 2 });
                }
            };
        };
        async function consume() {
            for await (const value of iterable) {
                trace = trace + ":" + value;
            }
        }
        consume();
        "#,
        "trace",
        ":10:20",
    )
}

#[test]
fn closes_async_iterators_on_break() -> TestResult {
    ensure_string_after(
        r#"
        let trace = "";
        const iterable = {};
        iterable[Symbol.asyncIterator] = function() {
            return {
                next: function() {
                    return Promise.resolve({ value: 42, done: false });
                },
                return: function() {
                    trace = trace + ":closed";
                    return Promise.resolve({ done: true });
                }
            };
        };
        async function consume() {
            for await (const value of iterable) {
                trace = trace + ":" + value;
                break;
            }
            trace = trace + ":done";
        }
        consume();
        "#,
        "trace",
        ":42:closed:done",
    )
}

#[test]
fn resumes_destructuring_targets_without_replaying_the_head() -> TestResult {
    ensure_string_after(
        r#"
        let trace = "";
        let headCount = 0;
        let holder = {};
        function values() {
            headCount = headCount + 1;
            return [Promise.resolve({ value: 40 }), { value: 42 }];
        }
        async function consume() {
            for await ({ value: holder.result } of values()) {
                trace = trace + ":" + holder.result;
            }
        }
        consume();
        "#,
        "trace + ':heads=' + headCount",
        ":40:42:heads=1",
    )
}

#[test]
fn rejected_next_results_enter_async_control_flow() -> TestResult {
    ensure_string_after(
        r#"
        let trace = "";
        const iterable = {};
        iterable[Symbol.asyncIterator] = function() {
            return {
                next: function() {
                    return Promise.reject(new Error("stop"));
                }
            };
        };
        async function consume() {
            try {
                for await (const value of iterable) {
                    trace = trace + value;
                }
            } catch (error) {
                trace = error.message;
            }
        }
        consume();
        "#,
        "trace",
        "stop",
    )
}

#[test]
fn preserves_sync_iterator_wrapper_promises_across_gc() -> TestResult {
    ensure_string_after(
        r#"
        const values = new Float64Array(2131);
        let result = "pending";
        async function consume() {
            for await (const value of values) {
            }
            return WeakMap;
        }
        consume().then(
            function(value) { result = value === WeakMap ? "ok" : "wrong"; },
            function(error) { result = error.name + ":" + error.message; }
        );
        "#,
        "result",
        "ok",
    )
}

#[test]
fn allows_for_await_only_in_async_functions() -> TestResult {
    let error = eval("for await (const value of []) {}")
        .err()
        .ok_or("for-await-of unexpectedly parsed outside an async function")?;
    if error.to_string().contains("for-await-of") {
        return Ok(());
    }
    Err(format!("unexpected parser error: {error}").into())
}

#[test]
fn rejects_resource_declarations_in_for_await_heads() -> TestResult {
    for source in [
        "async function f() { for await (using value of []) {} }",
        "async function f() { for await (await using value of []) {} }",
    ] {
        let error = eval(source)
            .err()
            .ok_or("resource declaration unexpectedly parsed in for-await head")?;
        if !error
            .to_string()
            .contains("resource declarations are not allowed in for-await heads")
        {
            return Err(format!("unexpected parser error: {error}").into());
        }
    }
    Ok(())
}

#[test]
fn allows_async_identifier_as_for_await_target() -> TestResult {
    ensure_string_after(
        r"
        let async;
        async function consume() {
            for await (async of [7]);
        }
        consume();
        ",
        "'' + async",
        "7",
    )
}

#[test]
fn rejects_escaped_for_await_of_keyword() -> TestResult {
    let source = r"async function f() { for await (var value o\u0066 []) {} }";
    if eval(source).is_err() {
        return Ok(());
    }
    Err("escaped for-await-of keyword unexpectedly parsed".into())
}
