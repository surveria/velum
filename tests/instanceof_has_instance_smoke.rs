use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn ordinary_instanceof_still_works() -> TestResult {
    eval_is_42(
        r"
        function Animal() {}
        function Dog() {}
        Dog.prototype = Object.create(Animal.prototype);
        function Cat() {}
        var dog = new Dog();
        dog instanceof Dog &&
            dog instanceof Animal &&
            (dog instanceof Cat) === false &&
            (dog instanceof Function) === false &&
            ({}) instanceof Object &&
            [] instanceof Array
            ? 42
            : 0
        ",
    )
}

#[test]
fn instanceof_consults_custom_function_handler() -> TestResult {
    eval_is_42(
        r"
        var matcher = function () {};
        Object.defineProperty(matcher, Symbol.hasInstance, {
            value: function (v) { return v === 40; },
        });
        (40 instanceof matcher) === true &&
            (41 instanceof matcher) === false &&
            (({}) instanceof matcher) === false
            ? 42
            : 0
        ",
    )
}

#[test]
fn instanceof_consults_plain_object_handler() -> TestResult {
    eval_is_42(
        r#"
        var stringMatcher = {
            [Symbol.hasInstance](v) { return typeof v === "string"; },
        };
        var alwaysTruthy = { [Symbol.hasInstance]() { return 1; } };
        ("x" instanceof stringMatcher) === true &&
            (5 instanceof stringMatcher) === false &&
            (null instanceof alwaysTruthy) === true &&
            (undefined instanceof alwaysTruthy) === true
            ? 42
            : 0
        "#,
    )
}

#[test]
fn instanceof_handler_side_effects_run_once_per_use() -> TestResult {
    eval_is_42(
        r"
        var counter = 0;
        var counting = { [Symbol.hasInstance]() { counter += 1; return counter === 1; } };
        var first = {} instanceof counting;
        var second = {} instanceof counting;
        first === true && second === false && counter === 2 ? 42 : 0
        ",
    )
}

#[test]
fn instanceof_raises_catchable_type_errors() -> TestResult {
    eval_is_42(
        r"
        var count = 0;
        try { ({}) instanceof 5; } catch (e) { if (e instanceof TypeError) count += 1; }
        try { ({}) instanceof 'x'; } catch (e) { if (e instanceof TypeError) count += 1; }
        try { ({}) instanceof {}; } catch (e) { if (e instanceof TypeError) count += 1; }
        try {
            var bad = {};
            Object.defineProperty(bad, Symbol.hasInstance, { value: 5 });
            ({}) instanceof bad;
        } catch (e) { if (e instanceof TypeError) count += 1; }
        count === 4 ? 42 : 0
        ",
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
