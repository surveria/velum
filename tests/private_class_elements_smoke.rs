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
