use velum::{Engine, OwnedValue};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new();
    let compiler = engine.create_vm();
    let script = compiler.compile_named(
        "examples/08_compile_once_run_many/counter.js",
        r"
        globalThis.runCount = (globalThis.runCount ?? 0) + 1;
        runCount * 10;
        ",
    )?;

    let mut first = engine.create_vm();
    let mut second = engine.create_vm();
    let first_run = first.eval_compiled_owned(&script)?;
    tokio::task::yield_now().await;
    let second_run = first.eval_compiled_owned(&script)?;
    let isolated_run = second.eval_compiled_owned(&script)?;

    let expected = [
        OwnedValue::Number(10.0),
        OwnedValue::Number(20.0),
        OwnedValue::Number(10.0),
    ];
    let actual = [first_run, second_run, isolated_run];
    if actual != expected {
        return Err(format!("unexpected compiled results: {actual:?}").into());
    }
    println!("One immutable CompiledScript, isolated results: {actual:?}");
    Ok(())
}
