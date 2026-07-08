use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_array_from_iterables_and_array_like_sources() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let fromArray = Array.from([1, 2, 3], function(value, index) {
            return value * 10 + index;
        });
        let thisArg = { offset: 5 };
        let fromObject = Array.from({ 0: "a", 2: "c", length: 3 }, function(value, index) {
            return String(value) + ":" + (index + this.offset);
        }, thisArg);
        let fromString = Array.from("az");

        fromArray.join("|") === "10|21|32" &&
            fromObject.length === 3 &&
            fromObject[0] === "a:5" &&
            fromObject[1] === "undefined:6" &&
            fromObject[2] === "c:7" &&
            fromString.join("|") === "a|z" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_array_of_and_from_custom_constructors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function Capture(length) {
            this.constructedLength = length;
        }

        let ofResult = Array.of.call(Capture, "x", "y");
        let fromArrayLike = Array.from.call(Capture, { 0: "a", 1: "b", length: 2 });
        let fromIterable = Array.from.call(Capture, ["i", "j"]);

        ofResult instanceof Capture &&
            ofResult.constructedLength === 2 &&
            ofResult.length === 2 &&
            ofResult[0] === "x" &&
            ofResult[1] === "y" &&
            fromArrayLike instanceof Capture &&
            fromArrayLike.constructedLength === 2 &&
            fromArrayLike.length === 2 &&
            fromArrayLike[0] === "a" &&
            fromIterable instanceof Capture &&
            fromIterable.constructedLength === undefined &&
            fromIterable.length === 2 &&
            fromIterable[1] === "j" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_array_iterator_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = ["a", "b"];
        let keys = values.keys();
        let entries = values.entries();
        let vals = values.values();
        let stringValues = Array.prototype.values.call("xy");
        let objectValues = Array.prototype.values.call({ 0: "o", 2: "p", length: 3 });

        let selfIterable = vals[Symbol.iterator]() === vals;
        let symbolAlias = Array.prototype[Symbol.iterator] === Array.prototype.values;
        values.push("c");

        keys.next().value === 0 &&
            keys.next().value === 1 &&
            keys.next().value === 2 &&
            keys.next().done === true &&
            entries.next().value.join(":") === "0:a" &&
            vals.next().value === "a" &&
            vals.next().value === "b" &&
            vals.next().value === "c" &&
            vals.next().done === true &&
            stringValues.next().value === "x" &&
            stringValues.next().value === "y" &&
            objectValues.next().value === "o" &&
            objectValues.next().value === undefined &&
            objectValues.next().value === "p" &&
            selfIterable &&
            symbolAlias ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn compiled_calls_mark_array_static_iterator_direct_targets() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        let fromValues = Array.from([1, 2, 3]);
        let ofValues = Array.of(4, 5);
        let keys = fromValues.keys();
        let values = ofValues.values();
        let entries = ofValues.entries();

        fromValues.length === 3 &&
            ofValues.length === 2 &&
            keys.next().value === 0 &&
            values.next().value === 4 &&
            entries.next().value[1] === 4 ? 42 : 0
        ",
    )?;

    ensure_min_usize(script.usage().bytecode_direct_native_call_count(), 5)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn for_of_uses_array_symbol_iterator_with_live_length() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [1, 2];
        let seen = "";
        for (let value of values) {
            seen = seen + value;
            if (value === 1) {
                values.push(3);
            }
        }
        seen === "123" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_min_usize(actual: usize, expected_minimum: usize) -> TestResult {
    if actual >= expected_minimum {
        return Ok(());
    }
    Err(format!("expected at least {expected_minimum}, got {actual}").into())
}
