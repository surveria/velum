use std::{sync::Arc, time::Duration};

use parking_lot::Mutex;
use velum::{Engine, Error, JsValueRef, OwnedValue, PropertyKeyRef, QueuedCallResult};
use velum_tokio::{RuntimeError, VmRuntime};

const TICK_COUNT: u32 = 10;
const TICK_DELAY: Duration = Duration::from_millis(1);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::new(Engine::new())?;
    let vm = runtime
        .spawn_vm_with(|vm| {
            vm.register_async_host_function_typed("rustSleep", |call| {
                let delay = call.number(0, "delay")?;
                if (delay - 1.0).abs() > f64::EPSILON {
                    return Err(Error::runtime(
                        "this example expects a one-millisecond delay",
                    ));
                }
                Ok(async {
                    tokio::time::sleep(TICK_DELAY).await;
                    Ok(())
                })
            })
        })
        .await?;
    vm.run(|vm| {
        vm.eval(
            r"
            async function setInterval(callback, delay) {
                for (let tick = 1; tick <= 10; tick += 1) {
                    await rustSleep(delay);
                    callback(tick);
                }
            }

            class CallbackTicker {
                constructor(callback) {
                    this.callback = callback;
                }

                start() {
                    return setInterval(this.callback, 1);
                }
            }
            ",
        )?;
        Ok(())
    })
    .await?;

    let calls = Arc::new(Mutex::new(Vec::new()));
    let captured_calls = Arc::clone(&calls);
    vm.run_local(move |vm| {
        let rust_callback = vm.create_host_function_typed("rustTick", move |call| {
            let tick = call.number(0, "tick")?;
            captured_calls.lock().push(tick);
            println!("Rust callback tick {tick}");
            Ok(())
        })?;
        let constructor = vm
            .get_global_retained("CallbackTicker")?
            .ok_or_else(|| Error::runtime("CallbackTicker was not defined"))?;
        let ticker =
            vm.construct_retained(&constructor, &[JsValueRef::Retained(&rust_callback)])?;
        let start = vm.get_property_retained((&ticker).into(), PropertyKeyRef::Name("start"))?;
        let request = vm.enqueue_call_with_receiver(&start, (&ticker).into(), &[])?;
        start.release()?;
        ticker.release()?;
        constructor.release()?;
        rust_callback.release()?;
        Ok(async move {
            match request.await.map_err(RuntimeError::from)? {
                QueuedCallResult::Owned(OwnedValue::Undefined) => Ok(()),
                QueuedCallResult::Owned(other) => Err(RuntimeError::Engine(format!(
                    "CallbackTicker.start returned {other:?}"
                ))),
                QueuedCallResult::Retained(value) => {
                    value.release().map_err(|error| {
                        RuntimeError::Engine(format!(
                            "unexpected interval result could not be released: {error}"
                        ))
                    })?;
                    Err(RuntimeError::Engine(
                        "CallbackTicker.start returned a VM-local value".to_owned(),
                    ))
                }
            }
        })
    })
    .await?;

    let expected = (1..=TICK_COUNT).map(f64::from).collect::<Vec<_>>();
    if calls.lock().as_slice() != expected.as_slice() {
        return Err(format!("expected {expected:?}, got {:?}", calls.lock()).into());
    }
    println!("JavaScript retained and invoked the Rust callback ten times");
    Ok(())
}
