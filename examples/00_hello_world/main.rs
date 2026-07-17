use velum::Engine;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Engine::new().create_vm();
    vm.eval_named(
        "examples/00_hello_world/main.js",
        r#"print("Hello, world!");"#,
    )?;

    for line in vm.take_output() {
        println!("{line}");
    }
    Ok(())
}
