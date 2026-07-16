use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn prototype_cycle_checks_stop_at_proxy_exotics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        var trapCount = 0;
        var root = {};
        var intermediary = new Proxy(Object.create(root), {
            getPrototypeOf() {
                trapCount += 1;
                throw new Error("unexpected getPrototypeOf trap");
            }
        });
        var leaf = Object.create(intermediary);
        var updated = Reflect.setPrototypeOf(root, leaf);
        updated && Object.getPrototypeOf(root) === leaf && trapCount === 0
        "#,
    )?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected Proxy-bounded prototype update, got {value:?}").into())
}

#[test]
fn supports_literal_prototype_lookup_and_enumeration() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let proto = {
            shared: 40,
            duplicate: "proto",
            read: function(delta) {
                return this.own + this.shared + delta;
            },
        };
        let child = { __proto__: proto, own: 1, duplicate: "own" };
        let method = child.read(1);
        child.shared = 41;

        let seen = "";
        for (let key in child) {
            seen = seen + key + ";";
        }
        print(child.shared, method, "shared" in child, "read" in child);
        print(seen);

        child.shared === 41 &&
            method === 42 &&
            ("shared" in child) &&
            ("read" in child) &&
            seen === "own;duplicate;shared;read;" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "41 42 true true".to_owned(),
            "own;duplicate;shared;read;".to_owned(),
        ],
    )
}

#[test]
fn supports_prototype_assignment_shadowing_and_null() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let proto = { value: 40 };
        let child = {};
        child.__proto__ = proto;

        let inherited = child.value;
        child.value = 41;
        let own = child.value;
        delete child.value;
        let restored = child.value;
        child.__proto__ = null;
        let cleared = child.value;

        print(inherited, own, restored, cleared, child.__proto__ === undefined);

        inherited === 40 &&
            own === 41 &&
            restored === 40 &&
            cleared === undefined &&
            child.__proto__ === undefined ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["40 41 40 undefined true".to_owned()])
}

#[test]
fn walks_deep_prototype_chains_with_cycle_guard() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let root = { shared: 7 };
        let current = root;
        for (let index = 0; index < 12; index++) {
            current = { __proto__: current };
        }
        let inherited = current.shared;
        let hasInherited = "shared" in current;
        current.own = 35;
        print(inherited, hasInherited, current.own);

        inherited === 7 && hasInherited && current.own === 35 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["7 true 35".to_owned()])
}

#[test]
fn rejects_prototype_cycles() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let left = {};
        let right = { __proto__: left };
        let rejected = false;
        try {
            left.__proto__ = right;
        } catch (error) {
            rejected = error instanceof TypeError;
        }
        rejected && Object.getPrototypeOf(left) === Object.prototype ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_function_prototype_cycles() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        function Regular() {}
        let original = Object.getPrototypeOf(Regular);
        let reflectRejected = Reflect.setPrototypeOf(Regular, Regular) === false;
        let objectRejected = false;
        try {
            Object.setPrototypeOf(Regular, Regular);
        } catch (error) {
            objectRejected = error instanceof TypeError;
        }
        let legacyRejected = false;
        try {
            Regular.__proto__ = Regular;
        } catch (error) {
            legacyRejected = error instanceof TypeError;
        }
        reflectRejected &&
            objectRejected &&
            legacyRejected &&
            Object.getPrototypeOf(Regular) === original ? 42 : 0
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

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}
