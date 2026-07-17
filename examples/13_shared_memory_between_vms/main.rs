use std::sync::Arc;

use parking_lot::Mutex;
use velum::{Engine, OwnedValue, SharedArrayBufferHandle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new();
    let captured = Arc::new(Mutex::new(None::<SharedArrayBufferHandle>));
    let captured_handle = Arc::clone(&captured);
    let mut producer = engine.create_vm();
    producer.register_host_function_typed("exportShared", move |call| {
        let handle = call.required_value(0, "buffer")?.to_shared_array_buffer()?;
        *captured_handle.lock() = Some(handle);
        Ok(())
    })?;
    producer.eval(
        r"
        const shared = new SharedArrayBuffer(4);
        Atomics.store(new Int32Array(shared), 0, 7);
        exportShared(shared);
        ",
    )?;
    let handle = captured
        .lock()
        .take()
        .ok_or("the producer did not export its shared buffer")?;

    let mut consumer = engine.create_vm();
    consumer.register_shared_array_buffer("shared", &handle)?;
    let previous = consumer.eval_owned("Atomics.add(new Int32Array(shared), 0, 35)")?;
    let observed = producer.eval_owned("Atomics.load(new Int32Array(shared), 0)")?;
    if previous != OwnedValue::Number(7.0) || observed != OwnedValue::Number(42.0) {
        return Err(format!("shared values were {previous:?} and {observed:?}").into());
    }
    println!(
        "Shared {}-byte backing store: previous={previous:?}, producer sees={observed:?}",
        handle.byte_length()
    );
    Ok(())
}
