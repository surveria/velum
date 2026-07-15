use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn promise_all_uses_custom_constructor_and_idempotent_elements() -> TestResult {
    ensure_eval(
        r#"
        let resolveCalls = 0;
        let resolved = "";
        function Constructor(executor) {
            executor(function(values) {
                resolveCalls = resolveCalls + 1;
                resolved = values.join(",");
            }, function(reason) {
                resolved = "rejected:" + reason;
            });
        }
        Constructor.resolve = function(value) { return value; };
        let later;
        let thenable = {
            then: function(resolve) {
                later = resolve;
                resolve(20);
                resolve(99);
            }
        };
        Promise.all.call(Constructor, [thenable, {
            then: function(resolve) { resolve(22); }
        }]);
        later(100);
        resolveCalls === 1 && resolved === "20,22" ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn promise_all_preserves_subclass_new_target() -> TestResult {
    ensure_eval(
        r"
        let executorCalls = 0;
        class SubPromise extends Promise {
            constructor(executor) {
                super(executor);
                executorCalls = executorCalls + 1;
            }
        }
        let result = Promise.all.call(SubPromise, []);
        result instanceof SubPromise && result.constructor === SubPromise &&
            executorCalls === 1 ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn promise_all_closes_iterator_and_rejects_on_abrupt_resolve() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let marker = {};
        let closed = 0;
        let rejected = "pending";
        let originalResolve = Promise.resolve;
        Promise.resolve = function() { throw marker; };
        let iterable = {};
        iterable[Symbol.iterator] = function() {
            return {
                next: function() { return { value: 1, done: false }; },
                return: function() { closed = closed + 1; return {}; }
            };
        };
        let combined = Promise.all(iterable);
        Promise.resolve = originalResolve;
        "#,
    )?;
    context.eval(
        r#"
        combined.then(undefined, function(reason) {
            rejected = reason === marker ? "marker" : String(reason);
        });
        "#,
    )?;
    let value = context.eval("closed + '|' + rejected")?;
    if value != Value::from("1|marker") {
        return Err(format!("unexpected close/rejection state: {value:?}").into());
    }
    Ok(())
}

#[test]
fn promise_all_calls_custom_capability_reject_on_abrupt_resolve() -> TestResult {
    ensure_eval(
        r"
        let marker = {};
        let rejected = false;
        function Constructor(executor) {
            executor(function() {}, function(reason) {
                rejected = reason === marker;
            });
        }
        Constructor.resolve = function() { throw marker; };
        Promise.all.call(Constructor, [1]);
        rejected ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if &value != expected {
        return Err(format!(
            "expected {expected:?}, received {value:?}; output: {:?}",
            context.output()
        )
        .into());
    }
    Ok(())
}
