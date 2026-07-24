use std::sync::Arc;

use parking_lot::Mutex;
use velum::{Engine, Error, OwnedValue, SharedArrayBufferHandle};
use velum_tokio::VmRuntime;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::builder(Engine::new())
        .worker_threads(2)
        .build()?;
    let captured = Arc::new(Mutex::new(None::<SharedArrayBufferHandle>));
    let captured_handle = Arc::clone(&captured);
    let producer = runtime
        .spawn_vm_with(move |vm| {
            vm.register_host_function_typed("exportShared", move |call| {
                let handle = call.required_value(0, "buffer")?.to_shared_array_buffer()?;
                *captured_handle.lock() = Some(handle);
                Ok(())
            })
        })
        .await?;
    producer
        .run(|vm| {
            vm.eval(
                r"
                const shared = new SharedArrayBuffer(4);
                Atomics.store(new Int32Array(shared), 0, 7);
                exportShared(shared);
                ",
            )?;
            Ok(())
        })
        .await?;
    let handle = captured
        .lock()
        .take()
        .ok_or("the producer did not export its shared buffer")?;

    let consumer_handle = handle.clone();
    let consumer = runtime
        .spawn_vm_with(move |vm| vm.register_shared_array_buffer("shared", &consumer_handle))
        .await?;
    let previous = consumer
        .run(|vm| owned_number(&vm.eval_owned("Atomics.add(new Int32Array(shared), 0, 35)")?))
        .await?;
    let observed = producer
        .run(|vm| owned_number(&vm.eval_owned("Atomics.load(new Int32Array(shared), 0)")?))
        .await?;
    if (previous - 7.0).abs() > f64::EPSILON || (observed - 42.0).abs() > f64::EPSILON {
        return Err(format!("shared values were {previous} and {observed}").into());
    }
    println!(
        "Shared {}-byte backing store: previous={previous}, producer sees={observed}",
        handle.byte_length()
    );
    Ok(())
}

fn owned_number(value: &OwnedValue) -> velum::Result<f64> {
    let OwnedValue::Number(number) = value else {
        return Err(Error::runtime(
            "shared memory operation did not return a number",
        ));
    };
    Ok(*number)
}
