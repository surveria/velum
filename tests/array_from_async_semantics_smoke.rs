use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn array_from_async_maps_sync_iterables_and_array_like_values() -> TestResult {
    ensure_after_jobs(
        r#"
        let observed = "pending";
        let receiver = { multiplier: 2 };
        Array.fromAsync([20, 21], function(value, index) {
            return Promise.resolve(value * this.multiplier + index);
        }, receiver).then(function(values) {
            observed = values.join(",");
        });
        "#,
        "observed",
        &Value::from("40,43"),
    )?;
    ensure_after_jobs(
        r#"
        let observed = "pending";
        Array.fromAsync({ length: 2, 0: Promise.resolve(20), 1: 22 })
            .then(function(values) { observed = values.join(","); });
        "#,
        "observed",
        &Value::from("20,22"),
    )
}

#[test]
fn array_from_async_preserves_async_iterator_values() -> TestResult {
    ensure_after_jobs(
        r#"
        let observed = "pending";
        let yielded = Promise.resolve({ marker: 42 });
        let count = 0;
        let iterator = {
            next: function() {
                count = count + 1;
                return Promise.resolve(count === 1
                    ? { value: yielded, done: false }
                    : { done: true });
            }
        };
        iterator[Symbol.asyncIterator] = function() { return this; };
        Array.fromAsync(iterator).then(function(values) {
            observed = values.length === 1 && values[0] === yielded ? 42 : 0;
        });
        "#,
        "observed",
        &Value::Number(42.0),
    )
}

#[test]
fn array_from_async_closes_sync_iterators_on_await_rejection() -> TestResult {
    ensure_after_jobs(
        r#"
        let closed = 0;
        let rejected = false;
        let iterator = {
            next: function() {
                return { value: { then: function(resolve, reject) { reject("stop"); } }, done: false };
            },
            return: function() {
                closed = closed + 1;
                return { done: true };
            }
        };
        iterator[Symbol.iterator] = function() { return this; };
        Array.fromAsync(iterator).then(undefined, function(reason) {
            rejected = reason === "stop";
        });
        "#,
        "closed === 1 && rejected ? 42 : 0",
        &Value::Number(42.0),
    )
}

#[test]
fn array_from_async_uses_custom_constructors_and_throwing_length_set() -> TestResult {
    ensure_after_jobs(
        r#"
        let observed = "pending";
        function Result(length) {
            this.constructedLength = arguments.length === 0 ? "none" : length;
        }
        Array.fromAsync.call(Result, { length: 2, 0: 20, 1: 22 })
            .then(function(value) {
                observed = value.constructedLength + "|" + value[0] + "|" + value[1];
            });
        "#,
        "observed",
        &Value::from("2|20|22"),
    )?;
    ensure_after_jobs(
        r#"
        let rejected = false;
        function ReadonlyLength() {
            Object.defineProperty(this, "length", {
                value: 99,
                writable: false,
                configurable: true
            });
        }
        Array.fromAsync.call(ReadonlyLength, [1]).then(undefined, function(error) {
            rejected = error instanceof TypeError;
        });
        "#,
        "rejected ? 42 : 0",
        &Value::Number(42.0),
    )
}

#[test]
fn array_from_async_rejects_oversized_intrinsic_arrays() -> TestResult {
    ensure_after_jobs(
        r"
        let rejected = false;
        Array.fromAsync.call({}, { length: 4294967296 }).then(undefined, function(error) {
            rejected = error instanceof RangeError;
        });
        ",
        "rejected ? 42 : 0",
        &Value::Number(42.0),
    )
}

fn ensure_after_jobs(source: &str, expression: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)?;
    let value = context.eval(expression)?;
    if &value != expected {
        return Err(format!(
            "expected {expected:?}, received {value:?}; output: {:?}",
            context.output()
        )
        .into());
    }
    Ok(())
}
