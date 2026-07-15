use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn array_from_maps_array_like_values_with_a_custom_constructor() -> TestResult {
    ensure_eval(
        r"
        function Result(length) {
            this.constructedLength = length;
        }
        let receiver = {};
        let result = Array.from.call(
            Result,
            { 0: 20, 1: 21, length: 2 },
            function(value, index) {
                return this === receiver ? value + index : 0;
            },
            receiver
        );
        result instanceof Result && result.constructedLength === 2 &&
            result.length === 2 && result[0] === 20 && result[1] === 22 ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn array_from_uses_live_array_iterator_values() -> TestResult {
    ensure_eval(
        r#"
        let source = [1, 2, 3];
        let result = Array.from(source, function(value, index) {
            if (index === 0) source[1] = 20;
            return value;
        });
        result.join(",") === "1,20,3" ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn array_from_closes_iterators_when_mapping_throws() -> TestResult {
    ensure_eval(
        r"
        let marker = {};
        let closed = 0;
        let iterable = {};
        iterable[Symbol.iterator] = function() {
            return {
                next: function() { return { value: 1, done: false }; },
                return: function() { closed = closed + 1; return {}; }
            };
        };
        let caught = false;
        try {
            Array.from(iterable, function() { throw marker; });
        } catch (error) {
            caught = error === marker;
        }
        caught && closed === 1 ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn array_from_does_not_close_iterators_when_stepping_throws() -> TestResult {
    ensure_eval(
        r"
        let marker = {};
        let closed = 0;
        let iterable = {};
        iterable[Symbol.iterator] = function() {
            return {
                next: function() { throw marker; },
                return: function() { closed = closed + 1; return {}; }
            };
        };
        let caught = false;
        try {
            Array.from(iterable);
        } catch (error) {
            caught = error === marker;
        }
        caught && closed === 0 ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn array_from_redefines_configurable_result_slots_as_data_properties() -> TestResult {
    ensure_eval(
        r#"
        function Result() {
            Object.defineProperty(this, "0", {
                value: 1,
                writable: false,
                enumerable: false,
                configurable: true
            });
        }
        let result = Array.from.call(Result, [42]);
        let descriptor = Object.getOwnPropertyDescriptor(result, "0");
        descriptor.value === 42 && descriptor.writable && descriptor.enumerable &&
            descriptor.configurable && result.length === 1 ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn array_from_redefines_existing_slots_on_non_extensible_results() -> TestResult {
    ensure_eval(
        r"
        function Result() {
            this[0] = this[1] = 0;
            Object.preventExtensions(this);
        }
        let result = new Result();
        let caught = false;
        try {
            Array.from.call(function() { return result; }, [10, 20]);
        } catch (error) {
            caught = error instanceof TypeError;
        }
        caught && result[0] === 10 && result[1] === 20 ? 42 : 0
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
