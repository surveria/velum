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
    ensure_value(&eval(source)?, &Value::from(expected))
}

#[test]
fn super_call_initializes_parent_state() -> TestResult {
    ensure_string(
        r#"
        class Base {
            constructor(x) {
                this.x = x;
            }
            getX() {
                return this.x;
            }
        }
        class Derived extends Base {
            constructor(x, y) {
                super(x);
                this.y = y;
            }
            sum() {
                return this.getX() + this.y;
            }
        }
        const d = new Derived(40, 2);
        "" + d.sum() + ":" + (d instanceof Derived) + ":" + (d instanceof Base)
        "#,
        "42:true:true",
    )
}

#[test]
fn default_derived_constructor_forwards_arguments() -> TestResult {
    ensure_string(
        r#"
        class Base {
            constructor(...xs) {
                this.parts = xs.join(":");
            }
        }
        class Derived extends Base {}
        new Derived("a", "b", "c").parts
        "#,
        "a:b:c",
    )
}

#[test]
fn super_method_calls_resolve_through_the_chain() -> TestResult {
    ensure_string(
        r#"
        class A {
            describe() {
                return "A";
            }
        }
        class B extends A {
            describe() {
                return super.describe() + "B";
            }
        }
        class C extends B {
            describe() {
                return super.describe() + "C";
            }
        }
        new C().describe()
        "#,
        "ABC",
    )
}

#[test]
fn super_property_reads_use_the_home_prototype() -> TestResult {
    ensure_string(
        r#"
        class Base {
            get magic() {
                return 21;
            }
        }
        class Derived extends Base {
            get magic() {
                return super.magic * 2;
            }
        }
        "" + new Derived().magic
        "#,
        "42",
    )
}

#[test]
fn static_members_inherit_through_the_constructor_chain() -> TestResult {
    ensure_string(
        r#"
        class Base {
            static tag() {
                return "base";
            }
        }
        class Derived extends Base {}
        class Override extends Base {
            static tag() {
                return "override";
            }
        }
        Derived.tag() + ":" + Override.tag()
        "#,
        "base:override",
    )
}

#[test]
fn methods_inherit_when_not_overridden() -> TestResult {
    ensure_string(
        r#"
        class Base {
            hello() {
                return "hi";
            }
        }
        class Derived extends Base {}
        const d = new Derived();
        d.hello() + ":" + (Object.getPrototypeOf(Object.getPrototypeOf(d)) === Base.prototype)
        "#,
        "hi:true",
    )
}

#[test]
fn extends_supports_constructor_functions() -> TestResult {
    ensure_string(
        r#"
        function Legacy(v) {
            this.v = v;
        }
        Legacy.prototype.read = function () {
            return this.v;
        };
        class Modern extends Legacy {
            constructor() {
                super(42);
            }
        }
        "" + new Modern().read()
        "#,
        "42",
    )
}

#[test]
fn super_calls_accept_spread_arguments() -> TestResult {
    ensure_string(
        r#"
        class Base {
            constructor(a, b, c) {
                this.total = a + b + c;
            }
        }
        class Derived extends Base {
            constructor(values) {
                super(...values);
            }
        }
        "" + new Derived([20, 21, 1]).total
        "#,
        "42",
    )
}

#[test]
fn heritage_expressions_are_evaluated() -> TestResult {
    ensure_string(
        r#"
        function pick() {
            return class {
                label() {
                    return "picked";
                }
            };
        }
        class Derived extends pick() {}
        new Derived().label()
        "#,
        "picked",
    )
}

#[test]
fn non_constructor_heritage_throws_type_error() -> TestResult {
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
        kind(function () { class Bad extends 5 {} })
            + ":" + kind(function () { class AlsoBad extends "text" {} })
        "#,
        "TypeError:TypeError",
    )
}

#[test]
fn null_and_symbol_heritage_preserve_constructor_semantics() -> TestResult {
    ensure_string(
        r#"
        class NullBase extends null {}
        class SymbolBase extends Symbol {}
        let rejected = false;
        try {
            new SymbolBase();
        } catch (error) {
            rejected = error instanceof TypeError;
        }
        "" + (Object.getPrototypeOf(NullBase.prototype) === null) + ":"
            + (Object.getPrototypeOf(NullBase) === Function.prototype) + ":" + rejected
        "#,
        "true:true:true",
    )
}

#[test]
fn super_outside_class_contexts_is_rejected() -> TestResult {
    let Err(error) = eval("function plain() { super(); }") else {
        return Err("expected super() outside classes to fail".into());
    };
    let message = error.to_string();
    if !message.contains("is only valid inside") {
        return Err(format!("unexpected error: {message}").into());
    }
    let Err(error) = eval("const o = { m() { super(); } };") else {
        return Err("expected super() in object methods to fail".into());
    };
    let message = error.to_string();
    if !message.contains("derived class constructors") {
        return Err(format!("unexpected error: {message}").into());
    }
    Ok(())
}

#[test]
fn arrow_functions_inherit_super_bindings() -> TestResult {
    ensure_string(
        r#"
        class Base {
            word() {
                return "lex";
            }
        }
        class Derived extends Base {
            word() {
                const grab = () => super.word() + "ical";
                return grab();
            }
        }
        new Derived().word()
        "#,
        "lexical",
    )
}

#[test]
fn three_level_construction_chains_initialize_in_order() -> TestResult {
    ensure_string(
        r#"
        let order = "";
        class L1 {
            constructor() {
                order = order + "1";
            }
        }
        class L2 extends L1 {
            constructor() {
                super();
                order = order + "2";
            }
        }
        class L3 extends L2 {
            constructor() {
                super();
                order = order + "3";
            }
        }
        new L3();
        order
        "#,
        "123",
    )
}
