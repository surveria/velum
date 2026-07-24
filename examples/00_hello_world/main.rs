use velum::Engine;
use velum_tokio::VmRuntime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::new(Engine::new())?;
    let vm = runtime.spawn_vm().await?;
    let output = vm
        .run(|vm| {
            vm.eval_named(
                "examples/00_hello_world/main.js",
                r#"print("Hello, world!");"#,
            )?;
            Ok(vm.take_output())
        })
        .await?;
    for line in output {
        println!("{line}");
    }
    Ok(())
}
