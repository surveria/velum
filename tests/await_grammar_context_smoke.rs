use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn direct_eval_rejects_top_level_await_as_script_syntax() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let caught = false;
        try {
            eval("await 10");
        } catch (error) {
            caught = error instanceof SyntaxError;
        }
        caught ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn class_fields_do_not_inherit_async_await_expressions() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        var await = 42;
        let observed = 0;
        async function createClass() {
            return class { value = await; };
        }
        createClass().then(function(ClassValue) {
            observed = new ClassValue().value;
        });
        ",
    )?;

    ensure_value(&context.eval("observed")?, &Value::Number(42.0))
}

#[test]
fn module_nested_functions_keep_await_reserved() -> TestResult {
    let runtime = Runtime::new();
    let result = runtime.compile_module_named(
        "nested-await.js",
        "function nested() { await; } export { nested };",
    );
    if result.is_ok() {
        return Err("module nested function accepted await as an identifier".into());
    }
    Ok(())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
