use velum::{Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_array_callback_methods_on_arrays() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [1, 2, 3, 4];
        let thisArg = { total: 0 };
        let forEachReturn = values.forEach(function(value, index, array) {
            this.total = this.total + value + index + (array === values ? 10 : 0);
        }, thisArg);

        let mapped = values.map(function(value, index) { return value * 10 + index; });
        let filtered = values.filter(function(value, index) { return value > 2 && index < 4; });
        let some = values.some(function(value) { return value === 3; });
        let every = values.every(function(value) { return value > 0; });
        let found = values.find(function(value) { return value > 2; });
        let foundIndex = values.findIndex(function(value) { return value > 2; });
        let reduced = values.reduce(function(acc, value, index) {
            return acc + value + index;
        }, 0);
        let reducedRight = values.reduceRight(function(acc, value) {
            return acc + "" + value;
        }, "");

        forEachReturn === undefined &&
            thisArg.total === 56 &&
            mapped.join("|") === "10|21|32|43" &&
            filtered.join("|") === "3|4" &&
            some === true &&
            every === true &&
            found === 3 &&
            foundIndex === 2 &&
            reduced === 16 &&
            reducedRight === "4321" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn preserves_sparse_array_callback_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let sparse = Array(4);
        sparse[1] = 2;
        sparse[3] = 4;

        let visited = "";
        sparse.forEach(function(value, index) {
            visited = visited + index + ":" + value + ";";
        });

        let mapped = sparse.map(function(value, index) { return value * 10 + index; });
        let filtered = sparse.filter(function(value) { return value > 2; });
        let findVisited = "";
        let found = sparse.find(function(value, index) {
            findVisited = findVisited + index + ":" + value + ";";
            return index === 0;
        });
        let foundIndex = sparse.findIndex(function(value, index) { return index === 2; });
        let reduced = sparse.reduce(function(acc, value, index) {
            return acc + value + index;
        }, 0);
        let reducedRight = sparse.reduceRight(function(acc, value, index) {
            return acc + "" + index + value;
        }, "");

        visited === "1:2;3:4;" &&
            mapped.length === 4 &&
            !("0" in mapped) &&
            mapped[1] === 21 &&
            !("2" in mapped) &&
            mapped[3] === 43 &&
            filtered.length === 1 &&
            filtered[0] === 4 &&
            findVisited === "0:undefined;" &&
            found === undefined &&
            foundIndex === 2 &&
            reduced === 10 &&
            reducedRight === "3412" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn packed_array_callbacks_observe_mutations_on_generic_path() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [1, 2, 3];
        let mapped = values.map(function(value, index, array) {
            if (index === 0) {
                array[1] = 20;
            }
            return value;
        });
        let reduced = values.reduce(function(acc, value, index, array) {
            if (index === 0) {
                array[1] = 30;
            }
            return acc + value;
        }, 0);

        mapped.join("|") === "1|20|3" &&
            values[1] === 30 &&
            reduced === 34 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_callback_methods_on_array_like_objects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { length: 3, 0: 1, 2: 3 };
        let seen = "";
        let mapped = Array.prototype.map.call(object, function(value, index, receiver) {
            seen = seen + index + ":" + value + ":" + (receiver === object) + ";";
            return value + 1;
        });
        let filtered = Array.prototype.filter.call(object, function(value) {
            return value > 1;
        });
        let some = Array.prototype.some.call(object, function(value) { return value === 3; });
        let every = Array.prototype.every.call(object, function(value) { return value > 0; });
        let found = Array.prototype.find.call(object, function(value, index) {
            return index === 1;
        });
        let foundIndex = Array.prototype.findIndex.call(object, function(value) {
            return value === 3;
        });
        let reduced = Array.prototype.reduce.call(object, function(acc, value, index) {
            return acc + value + index;
        }, 0);
        let reducedRight = Array.prototype.reduceRight.call(object, function(acc, value) {
            return acc + value;
        }, 0);

        seen === "0:1:true;2:3:true;" &&
            mapped.length === 3 &&
            mapped[0] === 2 &&
            !("1" in mapped) &&
            mapped[2] === 4 &&
            filtered.length === 1 &&
            filtered[0] === 3 &&
            some === true &&
            every === true &&
            found === undefined &&
            foundIndex === 2 &&
            reduced === 6 &&
            reducedRight === 4 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_missing_callbacks_and_empty_reduce_without_initial_value() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let missingCallback = false;
        let emptyReduce = false;
        try {
            [1].map();
        } catch (error) {
            missingCallback = true;
        }
        try {
            [].reduce(function(acc, value) { return acc + value; });
        } catch (error) {
            emptyReduce = true;
        }
        missingCallback && emptyReduce ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn callback_methods_observe_length_before_validating_callback() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let marker = {};
        let receiver = {};
        let lengthReads = 0;
        Object.defineProperty(receiver, "length", {
            get: function() {
                lengthReads = lengthReads + 1;
                throw marker;
            }
        });
        let methods = [
            Array.prototype.forEach,
            Array.prototype.some,
            Array.prototype.every,
            Array.prototype.find,
            Array.prototype.findIndex,
            Array.prototype.reduce,
            Array.prototype.reduceRight
        ];
        let markerErrors = 0;
        methods.forEach(function(method) {
            try {
                method.call(receiver, undefined);
            } catch (error) {
                if (error === marker) {
                    markerErrors = markerErrors + 1;
                }
            }
        });
        markerErrors === methods.length && lengthReads === methods.length ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn failed_array_length_shrink_preserves_set_failure_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let array = [0, 1, 2];
        Object.defineProperty(array, "2", {
            get: function() { return "locked"; },
            configurable: false
        });
        Object.defineProperty(array, "1", {
            get: function() {
                array.length = 2;
                return 1;
            },
            configurable: true
        });
        let everyResult = array.every(function(value) {
            return value !== "locked";
        });
        let reflectResult = Reflect.set(array, "length", 2);
        let strictThrew = false;
        try {
            (function() {
                "use strict";
                array.length = 2;
            })();
        } catch (error) {
            strictThrew = error instanceof TypeError;
        }
        everyResult === false && array.length === 3 &&
            reflectResult === false && strictThrew ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn limits_callback_methods_on_large_array_like_lengths() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_runtime_steps: 128,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();

    let Err(error) = context.eval(
        r"
        Array.prototype.some.call({ length: 1000 }, function() { return false; });
        ",
    ) else {
        return Err("expected Array.prototype.some to hit runtime step limit".into());
    };

    ensure_error_contains(&error, "runtime steps")
}

#[test]
fn map_and_filter_honor_custom_array_species_results() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let mapLength = -1;
        function MapResult(length) {
            mapLength = length;
            return { kind: "mapped" };
        }
        let values = [2, , 4];
        values.constructor = { [Symbol.species]: MapResult };
        let mapped = values.map(function(value) { return value * 10; });
        let mapDescriptor = Object.getOwnPropertyDescriptor(mapped, "0");

        let filterLength = -1;
        function FilterResult(length) {
            filterLength = length;
            return { kind: "filtered" };
        }
        values.constructor = { [Symbol.species]: FilterResult };
        let filtered = values.filter(function(value) { return value > 2; });
        let filterDescriptor = Object.getOwnPropertyDescriptor(filtered, "0");

        mapLength === 3 &&
            mapped.kind === "mapped" &&
            mapped[0] === 20 &&
            !("1" in mapped) &&
            mapped[2] === 40 &&
            mapDescriptor.writable && mapDescriptor.enumerable && mapDescriptor.configurable &&
            filterLength === 0 &&
            filtered.kind === "filtered" &&
            filtered[0] === 4 &&
            filterDescriptor.writable && filterDescriptor.enumerable && filterDescriptor.configurable &&
            !("length" in filtered) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn callback_result_creation_preserves_observable_order_and_length_snapshot() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let log = "";
        let values = [1, 2, 3];
        function Result(length) {
            log = log + "construct:" + length + ";";
            values.length = 1;
            return {};
        }
        Object.defineProperty(values, "constructor", {
            get: function() {
                log = log + "constructor;";
                return {
                    get [Symbol.species]() {
                        log = log + "species;";
                        return Result;
                    }
                };
            }
        });

        let mapped = values.map(function(value, index) {
            log = log + "callback:" + index + ";";
            return value;
        });

        log === "constructor;species;construct:3;callback:0;" &&
            mapped[0] === 1 &&
            !("1" in mapped) &&
            !("2" in mapped) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn callback_result_creation_propagates_length_and_property_failures() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let lengthError = {};
        let lengthFirst = false;
        try {
            Array.prototype.map.call({
                get length() { throw lengthError; }
            }, null);
        } catch (error) {
            lengthFirst = error === lengthError;
        }

        let callbackCount = 0;
        function ClosedResult() {
            return Object.preventExtensions({});
        }
        let values = [1, 2];
        values.constructor = { [Symbol.species]: ClosedResult };
        let propertyFailure = false;
        try {
            values.filter(function() {
                callbackCount = callbackCount + 1;
                return true;
            });
        } catch (error) {
            propertyFailure = error instanceof TypeError;
        }

        let rangeCallbackCount = 0;
        let rangeFailure = false;
        try {
            Array.prototype.map.call({ length: 4294967296 }, function() {
                rangeCallbackCount = rangeCallbackCount + 1;
            });
        } catch (error) {
            rangeFailure = error instanceof RangeError;
        }

        lengthFirst && propertyFailure && callbackCount === 1 &&
            rangeFailure && rangeCallbackCount === 0 ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(error: &velum::Error, text: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(text) {
        return Ok(());
    }

    Err(format!("expected error containing '{text}', got '{message}'").into())
}
