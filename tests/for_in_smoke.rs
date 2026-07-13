use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_for_in_bindings_and_property_order() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { first: 1, second: 2, third: 3 };
        delete object.second;
        object.second = 20;

        let seen = "";
        for (let key in object) {
            seen = seen + key + ":" + object[key] + ";";
        }
        print(seen, typeof key);

        let values = [10, 20];
        values[3] = 40;
        let indexes = "";
        for (const index in values) {
            indexes = indexes + index + "=" + values[index] + ";";
        }
        print(indexes, typeof index);

        var hoisted = "start";
        for (var name in { alpha: 1, beta: 2 }) {
            hoisted = name;
        }
        print(hoisted, typeof name, name);

        seen === "first:1;third:3;second:20;" &&
            indexes === "0=10;1=20;3=40;" &&
            hoisted === "beta" &&
            name === "beta" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "first:1;third:3;second:20; undefined".to_owned(),
            "0=10;1=20;3=40; undefined".to_owned(),
            "beta string beta".to_owned(),
        ],
    )
}

#[test]
fn supports_for_in_assignment_targets_and_control_flow() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let target = { slot: "" };
        let seen = "";
        for (target.slot in { a: 1, b: 2, c: 3 }) {
            if (target.slot === "b") {
                continue;
            }
            seen = seen + target.slot;
            if (target.slot === "c") {
                break;
            }
        }
        print(seen, target.slot);

        let bag = { key: "" };
        let property = "key";
        for (bag[property] in { left: 1, right: 2 }) {
        }
        print(bag.key);

        let pick = function() {
            for (let key in { first: 1, second: 2 }) {
                return key;
            }
            return "none";
        };
        print(pick());

        seen === "ac" && target.slot === "c" && bag.key === "right" && pick() === "first" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["ac c".to_owned(), "right".to_owned(), "first".to_owned()],
    )
}

#[test]
fn creates_fresh_const_binding_per_for_in_iteration() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let first = function() { return "none"; };
        let second = function() { return "none"; };
        let index = 0;

        for (const key in { alpha: 1, beta: 2 }) {
            if (index === 0) {
                first = function() { return key; };
            }
            if (index === 1) {
                second = function() { return key; };
            }
            index = index + 1;
        }

        print(first(), second(), typeof key);
        first() === "alpha" && second() === "beta" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["alpha beta undefined".to_owned()])
}

#[test]
fn creates_fresh_let_binding_per_for_in_iteration() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let first = function() { return "none"; };
        let second = function() { return "none"; };
        let index = 0;

        for (let key in { left: 1, right: 2 }) {
            if (index === 0) {
                first = function() { return key; };
            }
            if (index === 1) {
                second = function() { return key; };
            }
            index = index + 1;
        }

        print(first(), second(), typeof key);
        first() === "left" && second() === "right" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["left right undefined".to_owned()])
}

#[test]
fn skips_for_in_over_nullish_values() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let count = 0;
        for (let key in null) { count = count + 1; }
        for (let key in undefined) { count = count + 1; }
        count === 0 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn non_enumerable_own_property_shadows_enumerable_prototype_property() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let prototype = { property: 1 };
        let object = Object.create(prototype);
        Object.defineProperty(object, "property", {
            value: 2,
            enumerable: false
        });
        let seen = false;
        for (let key in object) {
            if (key === "property") { seen = true; }
        }
        seen ? 0 : 42
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_declarations_and_labelled_functions_as_loop_bodies() -> TestResult {
    ensure_error_contains("for (;;) class C {}", "declaration")?;
    ensure_error_contains("for (;;) function f() {}", "function declaration")?;
    ensure_error_contains(
        "while (false) first: second: function f() {}",
        "function declaration",
    )?;
    ensure_error_contains("do class C {} while (false)", "declaration")
}

#[test]
fn lexical_for_in_head_uses_a_tdz_and_iteration_scope() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let x = "outside";
        let headProbe;
        let declarationProbe;
        let bodyProbe;
        for (
            let [x, unused = declarationProbe = function() { return x; }]
            in
            { i: headProbe = function() { return typeof x; } }
        ) {
            bodyProbe = function() { return x; };
        }
        let headThrows = false;
        try { headProbe(); } catch (error) { headThrows = error instanceof ReferenceError; }
        headThrows && declarationProbe() === "i" && bodyProbe() === "i" ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn skips_properties_deleted_before_their_turn() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        var object = Object.create(null);
        object.first = 1;
        object.deleted = 2;
        object.last = 3;
        var names = "";
        for (var name in object) {
            delete object.deleted;
            names = names + name;
        }
        names
        "#,
    )?;
    ensure_value(&value, &Value::from("firstlast"))
}

#[test]
fn rejects_unparenthesized_in_in_classic_for_initializer() -> TestResult {
    ensure_error_contains(
        "var values = [1]; for (1 in values; true;) { break; }",
        "unparenthesized 'in'",
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
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
