use velum::{Engine, Error, OwnedValue};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Engine::new().create_vm();
    let value = vm.eval_owned(
        r#"
        const project = "Velum";
        print(`running ${project}`);
        6 * 7;
        "#,
    )?;
    if value != OwnedValue::Number(42.0) {
        return Err(format!("expected 42, got {value:?}").into());
    }

    for line in vm.take_output() {
        println!("JavaScript output: {line}");
    }
    println!("Owned result: {value:?}");

    let error = vm
        .eval_named(
            "examples/01_basic_eval/failure.js",
            "throw new Error('boom');",
        )
        .err()
        .ok_or("the failing script unexpectedly succeeded")?;
    print_error(&error);
    Ok(())
}

fn print_error(error: &Error) {
    if let Some(name) = error.javascript_error_name() {
        let message = error.javascript_error_message().unwrap_or_default();
        println!("JavaScript error: {name}: {message}");
        return;
    }
    println!("Engine error: {error}");
}
