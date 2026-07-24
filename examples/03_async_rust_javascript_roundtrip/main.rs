use velum::{Engine, Error, OwnedValue, QueuedCallResult};
use velum_tokio::{RuntimeError, VmRuntime};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::new(Engine::new())?;
    let vm = runtime
        .spawn_vm_with(|vm| {
            vm.register_async_host_task_typed("rustRoundTrip", |call| {
                let callback = call.required_value(0, "callback")?.retain()?;
                let message = call.string(1, "message")?.to_owned();
                let javascript = call.async_context()?;
                Ok(async move {
                    tokio::task::yield_now().await;
                    let result = javascript
                        .call(callback, vec![OwnedValue::String(message)])?
                        .await?;
                    let OwnedValue::String(result) = result else {
                        return Err(Error::runtime("jsLog must return a string").into());
                    };
                    Ok(format!("{result}:rust"))
                })
            })
        })
        .await?;
    vm.run(|vm| {
        vm.eval(
            r#"
            async function jsLog(message) {
                await Promise.resolve();
                print(`JavaScript received: ${message}`);
                return `${message}:javascript`;
            }

            async function jsEntry() {
                return await rustRoundTrip(jsLog, "hello");
            }
            "#,
        )?;
        Ok(())
    })
    .await?;
    let result = vm
        .run_local(|vm| {
            let entry = vm
                .get_global_retained("jsEntry")?
                .ok_or_else(|| Error::runtime("jsEntry was not defined"))?;
            let request = vm.enqueue_call(&entry, &[])?;
            entry.release()?;
            Ok(async move {
                match request.await.map_err(RuntimeError::from)? {
                    QueuedCallResult::Owned(OwnedValue::String(result)) => Ok(result),
                    QueuedCallResult::Owned(other) => Err(RuntimeError::Engine(format!(
                        "jsEntry returned {other:?} instead of a string"
                    ))),
                    QueuedCallResult::Retained(value) => {
                        value.release().map_err(|error| {
                            RuntimeError::Engine(format!(
                                "jsEntry returned a VM-local value and release failed: {error}"
                            ))
                        })?;
                        Err(RuntimeError::Engine(
                            "jsEntry returned a VM-local value instead of a string".to_owned(),
                        ))
                    }
                }
            })
        })
        .await?;
    let output = vm.run(|vm| Ok(vm.take_output())).await?;
    for line in output {
        println!("{line}");
    }
    println!("Rust received: {result}");
    Ok(())
}
