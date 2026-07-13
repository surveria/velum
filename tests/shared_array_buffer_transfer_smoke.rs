use std::{
    sync::{Arc, Barrier},
    thread,
};

use parking_lot::Mutex;
use rs_quickjs::{Error, OwnedValue, Runtime, SharedArrayBufferHandle};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn shared_buffer_handle(
    byte_length: usize,
) -> Result<SharedArrayBufferHandle, Box<dyn std::error::Error>> {
    let captured = Arc::new(Mutex::new(None::<SharedArrayBufferHandle>));
    let callback_capture = captured.clone();
    let runtime = Runtime::new();
    let mut source = runtime.context();
    source.register_host_function_typed("captureShared", move |call| {
        let handle = call.required_value(0, "buffer")?.to_shared_array_buffer()?;
        *callback_capture.lock() = Some(handle);
        Ok(())
    })?;
    source.eval(&format!(
        "captureShared(new SharedArrayBuffer({byte_length}));"
    ))?;
    let Some(handle) = captured.lock().take() else {
        return Err("host callback did not capture the shared buffer".into());
    };
    if handle.byte_length() != byte_length {
        return Err("captured shared buffer has the wrong length".into());
    }
    Ok(handle)
}

#[test]
fn shared_backing_store_crosses_vm_boundaries() -> TestResult {
    let handle = shared_buffer_handle(8)?;

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

#[test]
fn wait_async_settles_after_another_vm_notifies() -> TestResult {
    let handle = shared_buffer_handle(8)?;
    let notifier_handle = handle.clone();
    let waiter_ready = Arc::new(Barrier::new(2));
    let notifier_ready = waiter_ready.clone();
    let notifier = thread::spawn(move || -> Result<(), String> {
        let runtime = Runtime::new();
        let mut context = runtime.context();
        context
            .register_shared_array_buffer("shared", &notifier_handle)
            .map_err(|error| error.to_string())?;
        notifier_ready.wait();
        let result = context
            .eval_owned("Atomics.notify(new Int32Array(shared), 0, 1)")
            .map_err(|error| error.to_string())?;
        if result == OwnedValue::Number(1.0) {
            return Ok(());
        }
        Err(format!("async notifier returned {result:?}"))
    });
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_shared_array_buffer("shared", &handle)?;
    context.register_host_function_typed("waiterReady", move |_call| {
        waiter_ready.wait();
        Ok(())
    })?;
    context.eval(
        "const waiter = Atomics.waitAsync(new Int32Array(shared), 0, 0, 2000);\
         waiterReady();\
         waiter.value.then(value => globalThis.waitResult = value);",
    )?;
    context.run_jobs()?;
    let result = context.eval_owned("globalThis.waitResult")?;
    if result != OwnedValue::String("ok".to_owned()) {
        return Err(format!("async waiter returned {result:?}").into());
    }
    let notifier_result = notifier
        .join()
        .map_err(|_| Error::runtime("notifier thread terminated unexpectedly"))?;
    notifier_result.map_err(Into::into)
}
