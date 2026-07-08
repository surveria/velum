use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
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

fn ensure_string(source: &str, expected: &str) -> TestResult {
    ensure_value(&eval(source)?, &Value::String(expected.to_owned()))
}

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let Err(error) = eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error '{message}' to contain '{expected}'").into())
}

#[test]
fn binds_rest_parameters_as_arrays() -> TestResult {
    ensure_string(
        r#"
        function tail(first, ...rest) {
            return first + "|" + rest.length + "|" + rest.join(",");
        }
        const all = (...xs) => xs.join(":");
        tail(1, 2, 3) + ";" + tail("only") + ";" + all("a", "b")
        "#,
        "1|2|2,3;only|0|;a:b",
    )
}

#[test]
fn rest_parameter_supports_binding_patterns() -> TestResult {
    ensure_string(
        r#"
        function pair(...[a, b = "B"]) {
            return "" + a + b;
        }
        pair(1, 2) + ":" + pair(9)
        "#,
        "12:9B",
    )
}

#[test]
fn rest_parameter_does_not_count_toward_length() -> TestResult {
    ensure_string(
        r#"
        function f(a, b, ...r) {}
        function g(...r) {}
        "" + f.length + ":" + g.length
        "#,
        "2:0",
    )
}

#[test]
fn rejects_malformed_rest_parameters() -> TestResult {
    ensure_error_contains(
        "function bad(...r, x) {}",
        "rest parameter must be the last parameter",
    )?;
    ensure_error_contains(
        "function bad(...r = []) {}",
        "rest parameter cannot have a default value",
    )?;
    ensure_error_contains(
        "var o = { set v(...r) {} };",
        "setter parameter cannot be a rest parameter",
    )
}

#[test]
fn spreads_arguments_into_calls() -> TestResult {
    ensure_string(
        r#"
        function join(a, b, c, d) {
            return "" + a + b + c + d;
        }
        join(...[1, 2], 3, ...[4]) + ":" + Math.max(...[3, 9, 4])
        "#,
        "1234:9",
    )
}

#[test]
fn spreads_arguments_into_method_and_computed_calls() -> TestResult {
    ensure_string(
        r#"
        const target = {
            base: 10,
            add(a, b) { return this.base + a + b; }
        };
        const key = "add";
        "" + target.add(...[1, 2]) + ":" + target[key](...[20, 2])
        "#,
        "13:32",
    )
}

#[test]
fn spreads_arguments_into_constructors() -> TestResult {
    ensure_string(
        r#"
        function Pair(a, b) {
            this.sum = a + b;
        }
        "" + new Pair(...[40, 2]).sum
        "#,
        "42",
    )
}

#[test]
fn spreads_iterables_into_array_literals() -> TestResult {
    ensure_string(
        r#"
        let custom = {};
        custom[Symbol.iterator] = function () {
            let index = 0;
            return {
                next: function () {
                    index = index + 1;
                    return { done: index > 2, value: index * 10 };
                }
            };
        };
        [0, ...[1, 2], ..."ab", ...custom].join("|")
        "#,
        "0|1|2|a|b|10|20",
    )
}

#[test]
fn spreads_own_enumerable_properties_into_object_literals() -> TestResult {
    ensure_string(
        r#"
        const base = { x: 1, y: 2 };
        const merged = { w: 0, ...base, y: 9, ...null, ...undefined };
        "" + merged.w + merged.x + merged.y + ":" + base.y
        "#,
        "019:2",
    )
}

#[test]
fn spread_evaluation_order_is_left_to_right() -> TestResult {
    ensure_string(
        r#"
        let log = "";
        function note(tag, value) {
            log = log + tag;
            return value;
        }
        function three(a, b, c) {
            return "" + a + b + c;
        }
        const result = three(note("A", 1), ...note("B", [2]), note("C", 3));
        result + ":" + log
        "#,
        "123:ABC",
    )
}

#[test]
fn throws_type_error_for_non_iterable_spread() -> TestResult {
    ensure_string(
        r#"
        function kind(callback) {
            try {
                callback();
                return "none";
            } catch (error) {
                return error instanceof TypeError ? "TypeError" : "other";
            }
        }
        kind(function () { return [...5]; })
            + ":" + kind(function () { return [...null]; })
            + ":" + kind(function () { Math.max(...{}); })
        "#,
        "TypeError:TypeError:TypeError",
    )
}

#[test]
fn propagates_throws_from_spread_iterators() -> TestResult {
    ensure_string(
        r#"
        let iterable = {};
        iterable[Symbol.iterator] = function () {
            return {
                next: function () { throw new Error("spread boom"); }
            };
        };
        let caught = "";
        try {
            const spreadInto = [...iterable];
        } catch (error) {
            caught = error.message;
        }
        caught
        "#,
        "spread boom",
    )
}

#[test]
fn combines_spread_calls_with_rest_parameters() -> TestResult {
    ensure_string(
        r#"
        function collect(...rest) {
            return rest.join(":");
        }
        collect(...[1, 2], 3, ..."xy")
        "#,
        "1:2:3:x:y",
    )
}
