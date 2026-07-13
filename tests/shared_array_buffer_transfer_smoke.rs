use std::{sync::Arc, thread};

use parking_lot::Mutex;
use rs_quickjs::{Error, OwnedValue, Runtime, SharedArrayBufferHandle};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn shared_backing_store_crosses_vm_boundaries() -> TestResult {
    let captured = Arc::new(Mutex::new(None::<SharedArrayBufferHandle>));
    let callback_capture = captured.clone();
    let runtime = Runtime::new();
    let mut source = runtime.context();
    source.register_host_function_typed("captureShared", move |call| {
        let handle = call.required_value(0, "buffer")?.to_shared_array_buffer()?;
        *callback_capture.lock() = Some(handle);
        Ok(())
    })?;
    source.eval("captureShared(new SharedArrayBuffer(8));")?;
    let Some(handle) = captured.lock().take() else {
        return Err("host callback did not capture the shared buffer".into());
    };
    if handle.byte_length() != 8 {
        return Err("captured shared buffer has the wrong length".into());
    }

    let waiter_handle = handle.clone();
    let waiter = thread::spawn(move || -> Result<(), String> {
        let runtime = Runtime::new();
        let mut context = runtime.context();
        context.set_agent_can_block(true);
        context
            .register_shared_array_buffer("shared", &waiter_handle)
            .map_err(|error| error.to_string())?;
        let result = context
            .eval_owned("Atomics.wait(new Int32Array(shared), 0, 0, 2000)")
            .map_err(|error| error.to_string())?;
        if result == OwnedValue::String("ok".to_owned()) {
            return Ok(());
        }
        Err(format!("waiter returned {result:?}"))
    });

    let runtime = Runtime::new();
    let mut main_context = runtime.context();
    main_context.register_shared_array_buffer("shared", &handle)?;
    let mut did_notify = false;
    for _ in 0..10_000 {
        let result = main_context.eval_owned("Atomics.notify(new Int32Array(shared), 0, 1)")?;
        if result == OwnedValue::Number(1.0) {
            did_notify = true;
            break;
        }
        thread::yield_now();
    }
    if !did_notify {
        return Err(Error::runtime("notifier did not observe the waiter").into());
    }
    let worker_result = waiter
        .join()
        .map_err(|_| Error::runtime("waiter thread terminated unexpectedly"))?;
    worker_result.map_err(Into::into)
}
