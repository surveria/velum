use rs_quickjs::{Error, Runtime, RuntimeLimits, Value};

fn eval(source: &str) -> Value {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source).expect("script should evaluate")
}

#[test]
fn evaluates_arithmetic_with_precedence() {
    assert_eq!(eval("1 + 2 * 3 - 4 / 2"), Value::Number(5.0));
}

#[test]
fn evaluates_bindings_and_assignment() {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    assert_eq!(
        context.eval("let x = 40; x = x + 2; x").unwrap(),
        Value::Number(42.0)
    );
    assert_eq!(context.get_global("x"), Some(&Value::Number(42.0)));
}

#[test]
fn keeps_const_bindings_immutable() {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let error = context.eval("const x = 1; x = 2").unwrap_err();
    assert!(matches!(error, Error::Runtime { .. }));
}

#[test]
fn supports_strings_and_host_print() {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context
        .eval(r#"let name = "camera"; print("hello", name); "id-" + 7"#)
        .unwrap();

    assert_eq!(value, Value::String("id-7".to_owned()));
    assert_eq!(context.output(), &["hello camera"]);
}

#[test]
fn short_circuits_logical_operators() {
    assert_eq!(eval("false && missing"), Value::Bool(false));
    assert_eq!(eval(r#""ok" || missing"#), Value::String("ok".to_owned()));
}

#[test]
fn enforces_resource_limits() {
    let limits = RuntimeLimits {
        max_source_len: 8,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();

    let error = context.eval("let x = 10;").unwrap_err();
    assert!(matches!(error, Error::ResourceLimit { .. }));
}
