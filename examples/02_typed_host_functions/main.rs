use std::sync::Arc;

use parking_lot::Mutex;
use velum::{Engine, OwnedValue};
use velum_tokio::VmRuntime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let messages = Arc::new(Mutex::new(Vec::new()));
    let captured_messages = Arc::clone(&messages);
    let runtime = VmRuntime::new(Engine::new())?;
    let vm = runtime
        .spawn_vm_with(move |vm| {
            vm.register_host_function_typed("rustAdd", |call| {
                let left = call.number(0, "left")?;
                let right = call.number(1, "right")?;
                Ok(left + right)
            })?;
            vm.register_host_function_typed("rustGreet", move |call| {
                let name = call.string(0, "name")?;
                let greeting = format!("Hello, {name}, from Rust!");
                captured_messages.lock().push(greeting.clone());
                Ok(greeting)
            })
        })
        .await?;
    let result = vm
        .run(|vm| {
            let result = vm.eval_owned(
                r#"
                const total = rustAdd(20, 22);
                `${rustGreet("JavaScript")} Answer: ${total}`;
                "#,
            )?;
            let OwnedValue::String(result) = result else {
                return Err(velum::Error::runtime("expected a string result"));
            };
            Ok(result)
        })
        .await?;
    println!("{result}");
    println!("Captured by Rust: {:?}", messages.lock().as_slice());
    Ok(())
}
