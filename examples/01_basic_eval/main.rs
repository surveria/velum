use velum::{Engine, Error, OwnedValue};
use velum_tokio::VmRuntime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::new(Engine::new())?;
    let vm = runtime.spawn_vm().await?;
    let (value, output, rendered_error) = vm
        .run(|vm| {
            let value = vm.eval_owned(
                r#"
                const project = "Velum";
                print(`running ${project}`);
                6 * 7;
                "#,
            )?;
            if value != OwnedValue::Number(42.0) {
                return Err(Error::runtime(format!("expected 42, got {value:?}")));
            }
            let rendered_error = vm
                .eval_named(
                    "examples/01_basic_eval/failure.js",
                    "throw new Error('boom');",
                )
                .err()
                .map_or_else(
                    || Err(Error::runtime("the failing script unexpectedly succeeded")),
                    |error| Ok(render_error(&error)),
                )?;
            Ok((format!("{value:?}"), vm.take_output(), rendered_error))
        })
        .await?;
    for line in output {
        println!("JavaScript output: {line}");
    }
    println!("Owned result: {value}");
    println!("{rendered_error}");
    Ok(())
}

fn render_error(error: &Error) -> String {
    if let Some(name) = error.javascript_error_name() {
        let message = error.javascript_error_message().unwrap_or_default();
        return format!("JavaScript error: {name}: {message}");
    }
    format!("Engine error: {error}")
}
