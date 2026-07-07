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

#[test]
fn iterates_array_values_with_lexical_bindings() -> TestResult {
    ensure_string(
        r#"
        let out = "";
        for (const item of [1, 2, 3]) {
            out = out + ":" + item;
        }
        for (let item of ["a", "b"]) {
            out = out + ":" + item;
        }
        out
        "#,
        ":1:2:3:a:b",
    )
}

#[test]
fn iterates_with_var_and_assignment_targets() -> TestResult {
    ensure_string(
        r#"
        let out = "";
        for (var item of ["v1", "v2"]) {
            out = out + ":" + item;
        }
        var assigned = "";
        for (assigned of ["p", "q"]) {}
        var holder = {};
        for (holder.prop of [7]) {}
        out + "|" + assigned + "|" + holder.prop
        "#,
        ":v1:v2|q|7",
    )
}

#[test]
fn iterates_strings_by_code_points() -> TestResult {
    ensure_string(
        r#"
        let out = "";
        for (const ch of "ab¢") {
            out = out + "|" + ch;
        }
        for (const ch of new String("hi")) {
            out = out + "|" + ch;
        }
        out
        "#,
        "|a|b|¢|h|i",
    )
}

#[test]
fn honors_break_continue_and_labels() -> TestResult {
    ensure_string(
        r##"
        let out = "";
        for (const item of [1, 2, 3]) {
            if (item === 2) { break; }
            out = out + ":" + item;
        }
        for (const item of [1, 2, 3]) {
            if (item === 2) { continue; }
            out = out + ";" + item;
        }
        outer: for (const a of [1, 2]) {
            for (const b of [10, 20]) {
                if (b === 20) { continue outer; }
                out = out + "#" + (a * b);
            }
        }
        out
        "##,
        ":1;1;3#10#20",
    )
}

#[test]
fn reads_live_array_length_during_iteration() -> TestResult {
    ensure_string(
        r#"
        let out = "";
        let live = [1, 2];
        for (const item of live) {
            if (item === 1) { live.push(30); }
            out = out + ":" + item;
        }
        out
        "#,
        ":1:2:30",
    )
}

#[test]
fn consumes_custom_iterator_protocol_objects() -> TestResult {
    ensure_string(
        r#"
        let iterable = {};
        iterable[Symbol.iterator] = function () {
            let index = 0;
            return {
                next: function () {
                    index = index + 1;
                    return { done: index > 3, value: index * 10 };
                }
            };
        };
        let out = "";
        for (const item of iterable) {
            out = out + ":" + item;
        }
        out
        "#,
        ":10:20:30",
    )
}

#[test]
fn closes_iterator_on_break_but_not_on_exhaustion() -> TestResult {
    ensure_string(
        r#"
        let log = { closed: "" };
        let breakable = {};
        breakable[Symbol.iterator] = function () {
            return {
                next: function () {
                    return { done: false, value: 1 };
                },
                return: function () {
                    log.closed = log.closed + "B";
                    return {};
                }
            };
        };
        let exhaustible = {};
        exhaustible[Symbol.iterator] = function () {
            let index = 0;
            return {
                next: function () {
                    index = index + 1;
                    return { done: index > 2, value: index };
                },
                return: function () {
                    log.closed = log.closed + "E";
                    return {};
                }
            };
        };
        for (const item of breakable) {
            break;
        }
        for (const item of exhaustible) {}
        log.closed
        "#,
        "B",
    )
}

#[test]
fn closes_iterator_when_body_throws() -> TestResult {
    ensure_string(
        r#"
        let closed = false;
        let iterable = {};
        iterable[Symbol.iterator] = function () {
            return {
                next: function () {
                    return { done: false, value: 1 };
                },
                return: function () {
                    closed = true;
                    return {};
                }
            };
        };
        let caught = "";
        try {
            for (const item of iterable) {
                throw new Error("stop");
            }
        } catch (error) {
            caught = error.message;
        }
        caught + ":" + closed
        "#,
        "stop:true",
    )
}

#[test]
fn throws_type_error_for_non_iterables() -> TestResult {
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
        let out = kind(function () { for (const x of 5) {} });
        out = out + ":" + kind(function () { for (const x of null) {} });
        out = out + ":" + kind(function () { for (const x of undefined) {} });
        out = out + ":" + kind(function () { for (const x of {}) {} });
        out
        "#,
        "TypeError:TypeError:TypeError:TypeError",
    )
}

#[test]
fn propagates_throws_from_iterator_next() -> TestResult {
    ensure_string(
        r#"
        let iterable = {};
        iterable[Symbol.iterator] = function () {
            return {
                next: function () {
                    throw new Error("boom");
                }
            };
        };
        let caught = "";
        try {
            for (const item of iterable) {}
        } catch (error) {
            caught = error.message;
        }
        caught
        "#,
        "boom",
    )
}

#[test]
fn const_binding_is_fresh_per_iteration() -> TestResult {
    ensure_string(
        r#"
        let getters = [];
        for (const item of [1, 2]) {
            getters.push(function () { return item; });
        }
        "" + getters[0]() + ":" + getters[1]()
        "#,
        "1:2",
    )
}

#[test]
fn for_of_keyword_stays_contextual() -> TestResult {
    ensure_string(
        r#"
        let of = "identifier";
        for (of of ["still-works"]) {}
        of
        "#,
        "still-works",
    )
}
