use std::{
    future::Future,
    task::{Context as TaskContext, Poll, Waker},
};

use velum::{Engine, Error, HostFutureError, OwnedValue, QueuedCallRequest, QueuedCallResult, Vm};

const MAX_PUMP_TURNS: usize = 32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Engine::new().create_vm();
    vm.register_async_host_task_typed("rustRoundTrip", |call| {
        let callback = call.required_value(0, "callback")?.retain()?;
        let message = call.string(1, "message")?.to_owned();
        let javascript = call.async_context()?;
        Ok(async move {
            let result = javascript
                .call(callback, vec![OwnedValue::String(message)])?
                .await?;
            let OwnedValue::String(result) = result else {
                return Err(Error::runtime("jsLog must return a string").into());
            };
            Ok(format!("{result}:rust"))
        })
    })?;

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
    let entry = vm
        .get_global_retained("jsEntry")?
        .ok_or("jsEntry was not defined")?;
    let request = vm.enqueue_call(&entry, &[])?;
    entry.release()?;

    let result = drive_request(&mut vm, request)?;
    let QueuedCallResult::Owned(OwnedValue::String(result)) = result else {
        return Err("jsEntry did not return an owned string".into());
    };
    for line in vm.take_output() {
        println!("{line}");
    }
    println!("Rust received: {result}");
    Ok(())
}

fn drive_request(
    vm: &mut Vm,
    request: QueuedCallRequest,
) -> Result<QueuedCallResult, Box<dyn std::error::Error>> {
    let mut request = Box::pin(request);
    let mut task_context = TaskContext::from_waker(Waker::noop());
    for _ in 0..MAX_PUMP_TURNS {
        if let Poll::Ready(result) = request.as_mut().poll(&mut task_context) {
            return result.map_err(host_error);
        }
        vm.poll_host_futures(&mut task_context)?;
        vm.run_host_commands()?;
        vm.run_jobs()?;
    }
    Err("the async round trip did not settle".into())
}

fn host_error(error: HostFutureError) -> Box<dyn std::error::Error> {
    Box::new(error)
}
