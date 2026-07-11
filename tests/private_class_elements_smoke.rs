use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
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

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let actual = eval(source)?;
    let Value::String(actual) = actual else {
        return Err(format!("expected string '{expected}', got {actual:?}").into());
    };
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected string '{expected}', got '{actual}'").into())
}

#[test]
fn rejects_duplicate_private_names() -> TestResult {
    ensure_error_contains("class C { #x; #x; }", "duplicate private name '#x'")?;
    ensure_error_contains("class C { #m() {} #m() {} }", "duplicate private name '#m'")?;
    ensure_error_contains("class C { #x; #x() {} }", "duplicate private name '#x'")?;
    ensure_error_contains(
        "class C { get #a() {} get #a() {} }",
        "duplicate private name '#a'",
    )?;
    ensure_error_contains(
        "class C { static get #a() {} set #a(v) {} }",
        "duplicate private name '#a'",
    )
}

#[test]
fn rejects_private_constructor_name() -> TestResult {
    ensure_error_contains(
        "class C { #constructor; }",
        "class private name cannot be '#constructor'",
    )?;
    ensure_error_contains(
        "class C { #constructor() {} }",
        "class private name cannot be '#constructor'",
    )
}

#[test]
fn rejects_undeclared_private_references() -> TestResult {
    ensure_error_contains(
        "class C { m() { return this.#missing; } }",
        "private name '#missing' must be declared in an enclosing class",
    )?;
    ensure_error_contains(
        "({}).#x;",
        "private name '#x' must be declared in an enclosing class",
    )?;
    ensure_error_contains(
        "class C { m(o) { return #x in o; } }",
        "private name '#x' must be declared in an enclosing class",
    )?;
    ensure_error_contains(
        "class Outer { #a; m() { class Inner { n(o) { return o.#b; } } } }",
        "private name '#b' must be declared in an enclosing class",
    )
}

#[test]
fn rejects_private_deletion() -> TestResult {
    ensure_error_contains(
        "class C { #x; m() { delete this.#x; } }",
        "private members cannot be deleted",
    )?;
    ensure_error_contains(
        "class C { #x; m() { delete (this.#x); } }",
        "private members cannot be deleted",
    )
}

#[test]
fn rejects_standalone_private_names() -> TestResult {
    ensure_error_contains(
        "class C { #x; m() { return #x; } }",
        "only valid in member access",
    )?;
    ensure_error_contains("#x;", "parser error")?;
    ensure_error_contains("const o = { #x: 1 };", "parser error")
}

#[test]
fn rejects_malformed_private_tokens() -> TestResult {
    ensure_error_contains(
        "class C { # x; }",
        "expected identifier after private name marker",
    )?;
    ensure_error_contains(
        "class C { #1; }",
        "expected identifier after private name marker",
    )
}

#[test]
fn allows_private_names_across_nested_functions_and_classes() -> TestResult {
    // Inner classes may reference outer private names; parsing must accept
    // both even while runtime support is pending.
    let source = r"
        class Outer {
            #a;
            m() {
                const arrow = () => this.#a;
                class Inner { n(o) { return o.#a; } }
                return arrow;
            }
        }
    ";
    match eval(source) {
        Ok(_) => Ok(()),
        Err(error) => {
            let message = error.to_string();
            if message.contains("parser error") || message.contains("lexer error") {
                return Err(format!("expected '{source}' to parse, got '{message}'").into());
            }
            Ok(())
        }
    }
}

#[test]
fn supports_private_fields_updates_and_brand_checks() -> TestResult {
    ensure_string(
        r#"
        class Counter {
            #value = 1;
            update() {
                this.#value += 4;
                const previous = this.#value++;
                return previous + ":" + this.#value;
            }
            has(value) { return #value in value; }
        }
        const counter = new Counter();
        counter.update() + ":" + counter.has(counter) + ":" + counter.has({})
        "#,
        "5:6:true:false",
    )
}

#[test]
fn supports_private_methods_accessors_and_static_elements() -> TestResult {
    ensure_string(
        r#"
        class Box {
            #value = 3;
            #double() { return this.#value * 2; }
            get #current() { return this.#double(); }
            set #current(value) { this.#value = value; }
            read() { return this.#current; }
            write(value) { this.#current = value; return this.#double(); }
            static #count = 1;
            static #next() { return ++this.#count; }
            static take() { return this.#next(); }
        }
        const box = new Box();
        box.read() + ":" + box.write(7) + ":" + Box.take() + ":" + Box.take()
        "#,
        "6:14:2:3",
    )
}

#[test]
fn private_brands_are_fresh_per_class_evaluation() -> TestResult {
    ensure_string(
        r#"
        function make() {
            return class {
                #value = 9;
                read(other) { return other.#value; }
                has(other) { return #value in other; }
            };
        }
        const A = make();
        const B = make();
        const a = new A();
        const b = new B();
        a.read(a) + ":" + a.has(a) + ":" + a.has(b) + ":" + b.has(b)
        "#,
        "9:true:false:true",
    )
}

#[test]
fn reports_private_brand_failures() -> TestResult {
    ensure_error_contains(
        "class C { #x = 1; read(value) { return value.#x; } } new C().read({});",
        "required private brand",
    )?;
    ensure_error_contains(
        "class C { #m() {} write() { this.#m = 1; } } new C().write();",
        "private method is not writable",
    )
}

#[test]
fn preserves_private_environments_in_nested_classes_and_closures() -> TestResult {
    ensure_string(
        r#"
        class Outer {
            #value = 11;
            reader() {
                class Inner {
                    read(outer) {
                        const arrow = () => outer.#value;
                        return arrow();
                    }
                }
                return new Inner();
            }
        }
        const outer = new Outer();
        "" + outer.reader().read(outer)
        "#,
        "11",
    )
}

#[test]
fn supports_private_logical_assignments_and_derived_instances() -> TestResult {
    ensure_string(
        r#"
        class Base {
            #value;
            constructor(value) { this.#value = value; }
            fill() {
                this.#value ??= 4;
                this.#value &&= this.#value + 1;
                return this.#value;
            }
        }
        class Derived extends Base {}
        new Derived(null).fill() + ":" + new Derived(2).fill()
        "#,
        "5:3",
    )
}

#[test]
fn private_access_is_proxy_transparent() -> TestResult {
    ensure_string(
        r#"
        class Box {
            #value = 8;
            read() { return this.#value; }
            has(value) { return #value in value; }
        }
        const box = new Box();
        const proxy = new Proxy(box, {});
        proxy.read() + ":" + box.has(proxy)
        "#,
        "8:true",
    )
}
