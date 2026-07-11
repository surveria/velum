use rs_quickjs::{Error, Runtime, Value};

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
    error_contains(&error, expected)
}

fn error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error '{message}' to contain '{expected}'").into())
}

#[test]
fn destructures_object_declarations() -> TestResult {
    ensure_string(
        r#"
        const {a, b: renamed, missing = "fallback"} = {a: 1, b: 2};
        "" + a + ":" + renamed + ":" + missing
        "#,
        "1:2:fallback",
    )
}

#[test]
fn destructures_array_declarations_with_elisions_and_defaults() -> TestResult {
    ensure_string(
        r#"
        let [first, , third = 30, absent = "d"] = [1, 2, undefined];
        "" + first + ":" + third + ":" + absent
        "#,
        "1:30:d",
    )
}

#[test]
fn destructures_nested_patterns() -> TestResult {
    ensure_string(
        r#"
        const {outer: {inner}, list: [head, {deep}]} =
            {outer: {inner: "i"}, list: ["h", {deep: "d"}]};
        inner + ":" + head + ":" + deep
        "#,
        "i:h:d",
    )
}

#[test]
fn collects_object_and_array_rest() -> TestResult {
    ensure_string(
        r#"
        const {a, ...others} = {a: 1, b: 2, c: 3};
        const [x, ...tail] = [10, 20, 30];
        "" + a + ":" + others.b + ":" + others.c + ":" + (others.a === undefined)
            + "|" + x + ":" + tail.length + ":" + tail[0] + ":" + tail[1]
        "#,
        "1:2:3:true|10:2:20:30",
    )
}

#[test]
fn supports_computed_keys_and_defaults_reading_earlier_names() -> TestResult {
    ensure_string(
        r#"
        const key = "dynamic";
        const {[key + "Key"]: value, base, doubled = base * 2} =
            {dynamicKey: "found", base: 21};
        value + ":" + doubled
        "#,
        "found:42",
    )
}

#[test]
fn destructures_strings_and_custom_iterators() -> TestResult {
    ensure_string(
        r#"
        const [sa, sb] = "xy";
        let iterable = {};
        iterable[Symbol.iterator] = function () {
            let index = 0;
            return {
                next: function () {
                    index = index + 1;
                    return { done: index > 2, value: index * 5 };
                }
            };
        };
        const [ia, ib, ic] = iterable;
        sa + sb + ":" + ia + ":" + ib + ":" + (ic === undefined)
        "#,
        "xy:5:10:true",
    )
}

#[test]
fn closes_iterator_when_pattern_finishes_early() -> TestResult {
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
        const [only] = iterable;
        "" + only + ":" + closed
        "#,
        "1:true",
    )
}

#[test]
fn destructures_function_and_arrow_parameters() -> TestResult {
    ensure_string(
        r#"
        function join({left, right = "R"}, [a, b]) {
            return left + right + a + b;
        }
        const pick = ({v}) => v;
        const swap = ([p, q]) => q + p;
        join({left: "L"}, ["1", "2"]) + ":" + pick({v: "x"}) + ":" + swap(["a", "b"])
        "#,
        "LR12:x:ba",
    )
}

#[test]
fn destructures_parameter_with_whole_pattern_default() -> TestResult {
    ensure_string(
        r#"
        function normal({n} = {n: 1}) { return n; }
        normal() + ":" + normal({n: 5})
        "#,
        "1:5",
    )
}

#[test]
fn destructures_for_of_and_for_in_heads() -> TestResult {
    ensure_string(
        r##"
        let out = "";
        for (const {id} of [{id: 1}, {id: 2}]) {
            out = out + ":" + id;
        }
        for (const [a, b] of [[1, 2], [3, 4]]) {
            out = out + ";" + (a + b);
        }
        for (var {length} in {xy: 0}) {
            out = out + "#" + length;
        }
        out
        "##,
        ":1:2;3;7#2",
    )
}

#[test]
fn keeps_lexical_pattern_bindings_fresh_per_iteration() -> TestResult {
    ensure_string(
        r#"
        let getters = [];
        for (const {n} of [{n: 1}, {n: 2}]) {
            getters.push(function () { return n; });
        }
        "" + getters[0]() + ":" + getters[1]()
        "#,
        "1:2",
    )
}

#[test]
fn throws_type_errors_for_invalid_sources() -> TestResult {
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
        kind(function () { const {a} = null; })
            + ":" + kind(function () { const {b} = undefined; })
            + ":" + kind(function () { const [c] = 5; })
            + ":" + kind(function () { const {d: {e}} = {d: null}; })
        "#,
        "TypeError:TypeError:TypeError:TypeError",
    )
}

#[test]
fn propagates_throws_from_defaults_and_iterators() -> TestResult {
    ensure_string(
        r#"
        function boom() { throw new Error("default boom"); }
        let caught = "";
        try {
            const {missing = boom()} = {};
        } catch (error) {
            caught = error.message;
        }
        let iterable = {};
        iterable[Symbol.iterator] = function () {
            return {
                next: function () { throw new Error("next boom"); }
            };
        };
        try {
            const [broken] = iterable;
        } catch (error) {
            caught = caught + ":" + error.message;
        }
        caught
        "#,
        "default boom:next boom",
    )
}

#[test]
fn mixed_declaration_lists_combine_patterns_and_identifiers() -> TestResult {
    ensure_string(
        r#"
        let {a} = {a: 1}, [b] = [2], plain = 3;
        var {c} = {c: 4}, d = 5;
        "" + a + b + plain + c + d
        "#,
        "12345",
    )
}

#[test]
fn rejects_pattern_declarations_without_initializers() -> TestResult {
    ensure_error_contains(
        "let {a};",
        "destructuring declaration requires an initializer",
    )?;
    ensure_error_contains(
        "const [b];",
        "destructuring declaration requires an initializer",
    )?;
    ensure_error_contains(
        "var {c};",
        "destructuring declaration requires an initializer",
    )
}

#[test]
// The braced JavaScript sources below are not Rust formatting placeholders.
#[allow(clippy::literal_string_with_formatting_args)]
fn rejects_malformed_patterns() -> TestResult {
    ensure_error_contains("let {a: };", "expected binding name")?;
    ensure_error_contains(
        "let {1};",
        "expected ':' after object binding property name",
    )?;
    ensure_error_contains(
        "let [a b] = [1, 2];",
        "expected ']' after array binding pattern",
    )
}

#[test]
fn assigns_nested_patterns_and_preserves_rhs_value() -> TestResult {
    ensure_string(
        r#"
        let first, second, tail, rest;
        let values = [undefined, {value: 2}, 3, 4];
        let result = [first = 1, {value: second}, ...tail] = values;
        ({keep: first, ...rest} = {keep: 5, extra: 6});
        "" + first + ":" + second + ":" + tail.length + ":" + tail[0]
            + ":" + rest.extra + ":" + (result === values)
        "#,
        "5:2:2:3:6:true",
    )
}

#[test]
fn assigns_literal_member_targets() -> TestResult {
    ensure_string(
        r#"
        let assigned = 0;
        [{
            set value(next) { assigned = next; }
        }.value = 9] = [];
        "" + assigned
        "#,
        "9",
    )
}

#[test]
fn evaluates_array_target_reference_before_iterator_and_default() -> TestResult {
    ensure_string(
        r#"
        let log = [];
        let target = {
            set value(next) { log.push("set:" + next); }
        };
        function key() { log.push("target"); return "value"; }
        function fallback() { log.push("default"); return 7; }
        let iterable = {};
        iterable[Symbol.iterator] = function () {
            return {
                next: function () {
                    log.push("next");
                    return {done: false, value: undefined};
                },
                return: function () {
                    log.push("return");
                    return {};
                }
            };
        };
        [target[key()] = fallback()] = iterable;
        log.join("|")
        "#,
        "target|next|default|set:7|return",
    )
}

#[test]
fn creates_sloppy_globals_and_rejects_strict_targets() -> TestResult {
    ensure_string(
        r#"
        [createdByPattern] = [11];
        "" + createdByPattern + ":" + globalThis.createdByPattern
        "#,
        "11:11",
    )?;
    ensure_error_contains(
        r#""use strict"; [arguments] = [];"#,
        "invalid strict assignment target",
    )
}

#[test]
fn infers_names_for_assignment_pattern_defaults() -> TestResult {
    ensure_string(
        r#"
        let callback, Constructor;
        [callback = function() {}] = [];
        ({value: Constructor = class {}} = {});
        callback.name + ":" + Constructor.name
        "#,
        "callback:Constructor",
    )
}
