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
fn constructs_instances_with_prototype_methods() -> TestResult {
    ensure_string(
        r#"
        class Point {
            constructor(x, y) {
                this.x = x;
                this.y = y;
            }
            sum() {
                return this.x + this.y;
            }
        }
        const p = new Point(40, 2);
        "" + p.sum() + ":" + (p instanceof Point) + ":"
            + (Point.prototype.constructor === Point) + ":" + typeof Point
        "#,
        "42:true:true:function",
    )
}

#[test]
fn default_constructor_creates_plain_instances() -> TestResult {
    ensure_string(
        r#"
        class Empty {}
        const e = new Empty();
        "" + (e instanceof Empty) + ":" + Empty.name + ":" + Empty.length
        "#,
        "true:Empty:0",
    )
}

#[test]
fn supports_static_methods_on_the_constructor() -> TestResult {
    ensure_string(
        r#"
        class Registry {
            static create(tag) {
                return new Registry(tag);
            }
            constructor(tag) {
                this.tag = tag;
            }
            static describe() {
                return "static";
            }
        }
        Registry.create("r").tag + ":" + Registry.describe()
            + ":" + (Registry.prototype.create === undefined)
        "#,
        "r:static:true",
    )
}

#[test]
fn merges_getters_and_setters_on_the_prototype() -> TestResult {
    ensure_string(
        r#"
        class Boxed {
            get value() {
                return this.stored * 2;
            }
            set value(next) {
                this.stored = next / 2;
            }
        }
        const b = new Boxed();
        b.value = 42;
        "" + b.stored + ":" + b.value
        "#,
        "21:42",
    )
}

#[test]
fn supports_computed_string_and_numeric_member_keys() -> TestResult {
    ensure_string(
        r#"
        const suffix = "puted";
        class Keys {
            ["com" + suffix]() { return "c"; }
            "quoted"() { return "q"; }
            42() { return "n"; }
        }
        const k = new Keys();
        k.computed() + ":" + k.quoted() + ":" + k[42]()
        "#,
        "c:q:n",
    )
}

#[test]
fn class_methods_are_not_enumerable() -> TestResult {
    ensure_string(
        r#"
        class Quiet {
            visible() {}
            static hidden() {}
        }
        const q = new Quiet();
        q.own = 1;
        let seen = "";
        for (const key in q) {
            seen = seen + key;
        }
        seen
        "#,
        "own",
    )
}

#[test]
fn class_constructor_requires_new() -> TestResult {
    ensure_string(
        r#"
        class Guarded {}
        let caught = "";
        try {
            Guarded();
        } catch (error) {
            caught = (error instanceof TypeError) + ":" + error.message;
        }
        caught
        "#,
        "true:Class constructor cannot be invoked without 'new'",
    )
}

#[test]
fn class_declarations_bind_with_tdz() -> TestResult {
    ensure_string(
        r#"
        let caught = "";
        try {
            Later;
            class Later {}
        } catch (error) {
            caught = "" + (error instanceof ReferenceError);
        }
        caught
        "#,
        "true",
    )
}

#[test]
fn class_expressions_carry_optional_names() -> TestResult {
    ensure_string(
        r#"
        const Anon = class {
            m() { return "anon"; }
        };
        const Named = class Inner {
            m() { return "named"; }
        };
        new Anon().m() + ":" + new Named().m() + ":" + Named.name
        "#,
        "anon:named:Inner",
    )
}

#[test]
fn constructor_parameters_support_patterns_and_rest() -> TestResult {
    ensure_string(
        r#"
        class Wide {
            constructor({a, b = 2}, ...rest) {
                this.sum = a + b + rest.length;
            }
        }
        "" + new Wide({a: 1}, 9, 9, 9).sum
        "#,
        "6",
    )
}

#[test]
fn constructor_return_object_overrides_instance() -> TestResult {
    ensure_string(
        r#"
        class Override {
            constructor() {
                return {custom: "yes"};
            }
        }
        new Override().custom
        "#,
        "yes",
    )
}

#[test]
fn rejects_class_early_errors() -> TestResult {
    ensure_error_contains(
        "class Dup { constructor() {} constructor() {} }",
        "class body cannot declare two constructors",
    )?;
    ensure_error_contains(
        "class SP { static prototype() {} }",
        "class static member cannot be named 'prototype'",
    )?;
    ensure_error_contains(
        "class GA { get constructor() {} }",
        "class constructor cannot be an accessor",
    )
}

#[test]
fn reports_unsupported_class_features_explicitly() -> TestResult {
    ensure_error_contains(
        "class E extends Object {}",
        "class inheritance is not supported yet",
    )?;
    ensure_error_contains("class F { x = 1 }", "class fields are not supported yet")?;
    ensure_error_contains(
        "class G { *gen() {} }",
        "class generator methods are not supported yet",
    )?;
    ensure_error_contains(
        "class H { async m() {} }",
        "class async methods are not supported yet",
    )?;
    ensure_error_contains(
        "class I { static get x() { return 1 } }",
        "class static accessors are not supported yet",
    )
}
