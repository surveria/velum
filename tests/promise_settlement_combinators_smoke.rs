use rs_quickjs::{Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn settlement_combinators_preserve_standard_results() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let settled = "pending";
        let any = "pending";
        let race = "pending";
        Promise.allSettled([Promise.resolve(1), Promise.reject(2)]).then(function(results) {
            settled = results[0].status + ":" + results[0].value + "," +
                results[1].status + ":" + results[1].reason;
        });
        Promise.any([Promise.reject("left"), Promise.resolve("right")]).then(function(value) {
            any = value;
        });
        Promise.race([Promise.resolve("first"), Promise.resolve("second")]).then(function(value) {
            race = value;
        });
        "#,
    )?;
    ensure_value(
        &context.eval("settled + '|' + any + '|' + race")?,
        &Value::from("fulfilled:1,rejected:2|right|first"),
    )
}

#[test]
fn settlement_combinators_handle_empty_inputs() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let settled = "pending";
        let any = "pending";
        let race = "pending";
        Promise.allSettled([]).then(function(values) {
            settled = values.length === 0 ? "empty" : "invalid";
        });
        Promise.any([]).then(undefined, function(error) {
            any = error instanceof AggregateError && error.errors.length === 0 ?
                "aggregate" : "invalid";
        });
        Promise.race([]).then(function() { race = "fulfilled"; }, function() {
            race = "rejected";
        });
        "#,
    )?;
    ensure_value(
        &context.eval("settled + '|' + any + '|' + race")?,
        &Value::from("empty|aggregate|pending"),
    )
}

#[test]
fn promise_any_uses_generic_resolve_and_rejects_when_capability_resolve_throws() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let marker = {};
        let observed = "pending";
        function Constructor(executor) {
            return new Promise(function(_, reject) {
                executor(function() { throw marker; }, reject);
            });
        }
        Constructor.resolve = Promise.resolve;
        Promise.any.call(Constructor, [1]).then(undefined, function(reason) {
            observed = reason === marker ? "marker" : "invalid";
        });
        "#,
    )?;
    ensure_value(&context.eval("observed")?, &Value::from("marker"))
}

#[test]
fn settlement_combinators_close_iterators_after_resolve_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r"
        let marker = {};
        let closed = 0;
        let rejected = 0;
        let originalResolve = Promise.resolve;
        Promise.resolve = function() { throw marker; };
        function iterable() {
            return {
                [Symbol.iterator]: function() {
                    return {
                        next: function() { return { value: 1, done: false }; },
                        return: function() { closed = closed + 1; return {}; }
                    };
                }
            };
        }
        Promise.allSettled(iterable()).then(undefined, function(reason) {
            if (reason === marker) rejected = rejected + 1;
        });
        Promise.any(iterable()).then(undefined, function(reason) {
            if (reason === marker) rejected = rejected + 1;
        });
        Promise.race(iterable()).then(undefined, function(reason) {
            if (reason === marker) rejected = rejected + 1;
        });
        Promise.resolve = originalResolve;
        ",
    )?;
    ensure_value(
        &context.eval("closed === 3 && rejected === 3 ? 42 : 0")?,
        &Value::Number(42.0),
    )
}

#[test]
fn aggregate_error_materializes_iterable_errors_with_standard_descriptor() -> TestResult {
    ensure_eval(
        r#"
        let error = new AggregateError(new Set([1, 2]), "many");
        let descriptor = Object.getOwnPropertyDescriptor(error, "errors");
        error.errors.join(",") === "1,2" &&
            descriptor.writable && !descriptor.enumerable && descriptor.configurable &&
            error.message === "many" && Object.getPrototypeOf(AggregateError) === Error &&
            Object.getPrototypeOf(Error.prototype) === Object.prototype &&
            !AggregateError.prototype.hasOwnProperty("errors") ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn promise_resolve_validates_receiver_and_observes_promise_constructor() -> TestResult {
    ensure_eval(
        r"
        let promise = new Promise(function() {});
        promise.constructor = null;
        let distinct = Promise.resolve(promise) !== promise;
        let rejected = false;
        try {
            Promise.resolve.call(null, promise);
        } catch (error) {
            rejected = error instanceof TypeError;
        }
        distinct && rejected ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, expected)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
