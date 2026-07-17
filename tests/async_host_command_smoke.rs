use std::{
    cell::RefCell,
    future::Future,
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

use velum::{
    Engine, EngineConfig, Error, HostFutureError, JsValueRef, OwnedValue, PropertyKeyRef,
    RuntimeLimits, Value, Vm, VmStorageKind, VmStorageLimits,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn async_rust_function_awaits_async_javascript_callback() -> TestResult {
    let mut vm = Engine::new().create_vm();
    register_round_trip(&mut vm)?;
    vm.eval(
        r"
        var roundTripTrace = [];
        var roundTripResult = 'pending';
        async function javascriptWork(message) {
            roundTripTrace.push('start:' + message);
            await Promise.resolve();
            roundTripTrace.push('finish:' + message);
            return message + ':javascript';
        }
        rustRoundTrip(javascriptWork, 'hello').then(function (value) {
            roundTripResult = value;
        });
        ",
    )?;

    let first_poll = poll_once(&mut vm)?;
    ensure_usize(first_poll.completed(), 0, "initial completed host futures")?;
    ensure_usize(first_poll.pending(), 1, "initial pending host futures")?;
    ensure_usize(vm.pending_host_command_count(), 1, "pending host commands")?;
    ensure_usize(vm.queued_host_command_count(), 1, "queued host commands")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::HostCommand),
        1,
        "host command storage",
    )?;
    ensure_usize(
        vm.storage_snapshot()?
            .payload_bytes(VmStorageKind::HostCommand),
        "hello".len(),
        "queued host command payload",
    )?;
    vm.collect_garbage()?;

    ensure_usize(vm.run_host_commands()?, 1, "executed host commands")?;
    ensure_usize(vm.queued_host_command_count(), 0, "commands after dispatch")?;
    ensure_usize(
        vm.pending_host_command_count(),
        1,
        "commands awaiting result",
    )?;
    ensure_usize(
        vm.storage_snapshot()?
            .payload_bytes(VmStorageKind::HostCommand),
        0,
        "dispatched host command payload",
    )?;
    ensure_true(
        vm.pending_job_count() > 0,
        "JavaScript callback jobs were not queued",
    )?;
    vm.run_jobs()?;
    ensure_usize(
        vm.storage_snapshot()?
            .payload_bytes(VmStorageKind::HostCommand),
        "hello:javascript".len(),
        "completed host command payload",
    )?;

    let second_poll = poll_once(&mut vm)?;
    ensure_usize(
        second_poll.completed(),
        1,
        "completed round-trip host futures",
    )?;
    ensure_usize(
        second_poll.pending(),
        0,
        "remaining round-trip host futures",
    )?;
    ensure_usize(
        vm.pending_host_command_count(),
        0,
        "completed host commands",
    )?;
    ensure_usize(vm.run_jobs()?, 1, "outer Promise reaction jobs")?;
    ensure_value(
        vm.get_global("roundTripResult").as_ref(),
        &Value::from("hello:javascript:rust"),
        "round-trip result",
    )?;
    let trace = vm.eval("roundTripTrace.join('|')")?;
    ensure_value(
        Some(&trace),
        &Value::from("start:hello|finish:hello"),
        "round-trip trace",
    )?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::HostCommand),
        0,
        "released host command storage",
    )
}

#[test]
fn javascript_rejection_identity_survives_rust_round_trip_and_gc() -> TestResult {
    let mut vm = Engine::new().create_vm();
    register_round_trip(&mut vm)?;
    vm.eval(
        r"
        var rejectionMarker = { source: 'javascript' };
        var rejectionWasSame = false;
        async function rejectInJavascript() {
            await Promise.resolve();
            throw rejectionMarker;
        }
        rustRoundTrip(rejectInJavascript, 'ignored').catch(function (error) {
            rejectionWasSame = error === rejectionMarker;
        });
        ",
    )?;

    poll_once(&mut vm)?;
    vm.run_host_commands()?;
    vm.collect_garbage()?;
    vm.run_jobs()?;
    vm.collect_garbage()?;
    poll_once(&mut vm)?;
    vm.run_jobs()?;
    ensure_value(
        vm.get_global("rejectionWasSame").as_ref(),
        &Value::Bool(true),
        "JavaScript rejection identity",
    )
}

#[test]
fn externally_polled_command_error_keeps_javascript_rejection_rooted() -> TestResult {
    let mut vm = Engine::new().create_vm();
    let captured = Rc::new(RefCell::new(None));
    let captured_for_callback = Rc::clone(&captured);
    vm.register_async_host_function_typed("captureAsyncContext", move |call| {
        captured_for_callback.replace(Some(call.async_context()?));
        Ok(async move { Ok(()) })
    })?;
    vm.eval(
        r"
        var externalMarker = { source: 'external-command' };
        async function externalRejector() { throw externalMarker; }
        captureAsyncContext();
        ",
    )?;
    let Some(async_context) = captured.borrow_mut().take() else {
        return Err("async context was not captured".into());
    };
    let callback = vm
        .get_global_retained("externalRejector")?
        .ok_or("externalRejector global is missing")?;
    let request = async_context.call(callback, Vec::new())?;
    vm.run_host_commands()?;
    vm.run_jobs()?;

    let mut request = Box::pin(request);
    let mut task_context = TaskContext::from_waker(Waker::noop());
    let Poll::Ready(Err(HostFutureError::JavaScript(reason))) =
        request.as_mut().poll(&mut task_context)
    else {
        return Err("external command did not preserve a JavaScript rejection".into());
    };
    vm.eval("externalMarker = null;")?;
    vm.collect_garbage()?;
    vm.collect_garbage()?;
    let source = vm.get_property(
        JsValueRef::Retained(&reason),
        PropertyKeyRef::Name("source"),
    )?;
    ensure_value(
        Some(&source),
        &Value::from("external-command"),
        "externally retained rejection",
    )?;
    reason.release()?;
    Ok(())
}

#[test]
fn cancelling_command_wakes_rust_future_and_rejects_outer_promise() -> TestResult {
    let mut vm = Engine::new().create_vm();
    register_round_trip(&mut vm)?;
    vm.eval(
        r"
        var cancellationMessage = 'pending';
        rustRoundTrip(function neverCalled() { return 42; }, 'ignored').catch(
            function (error) { cancellationMessage = error.message; }
        );
        ",
    )?;

    poll_once(&mut vm)?;
    ensure_usize(vm.cancel_host_commands()?, 1, "cancelled host commands")?;
    let poll = poll_once(&mut vm)?;
    ensure_usize(poll.completed(), 1, "host future completed by cancellation")?;
    ensure_usize(vm.run_jobs()?, 1, "cancellation Promise jobs")?;
    ensure_value(
        vm.get_global("cancellationMessage").as_ref(),
        &Value::from(
            "runtime error: host function 'rustRoundTrip': JavaScript host command was cancelled",
        ),
        "command cancellation message",
    )
}

#[test]
fn cancelling_promise_jobs_wakes_command_waiting_on_javascript() -> TestResult {
    let mut vm = Engine::new().create_vm();
    register_round_trip(&mut vm)?;
    vm.eval(
        r"
        rustRoundTrip(function pendingJavaScript() {
            return new Promise(function pendingExecutor() {});
        }, 'ignored');
        ",
    )?;

    poll_once(&mut vm)?;
    vm.run_host_commands()?;
    ensure_usize(vm.pending_host_command_count(), 1, "waiting host commands")?;
    ensure_true(
        vm.cancel_jobs()? > 0,
        "waiting command reaction was not cancelled",
    )?;
    let poll = poll_once(&mut vm)?;
    ensure_usize(poll.completed(), 1, "host future after job cancellation")?;
    ensure_usize(
        vm.pending_host_command_count(),
        0,
        "commands after job cancellation",
    )
}

#[test]
fn cancelling_outer_host_future_abandons_queued_command() -> TestResult {
    let mut vm = Engine::new().create_vm();
    register_round_trip(&mut vm)?;
    vm.eval(
        r"
        rustRoundTrip(function abandonedJavaScript() { return 42; }, 'ignored');
        ",
    )?;

    poll_once(&mut vm)?;
    ensure_usize(
        vm.pending_host_command_count(),
        1,
        "queued command before abandon",
    )?;
    ensure_usize(vm.cancel_host_futures()?, 1, "cancelled outer host futures")?;
    ensure_usize(
        vm.pending_host_command_count(),
        0,
        "abandoned host commands",
    )?;
    ensure_usize(
        vm.queued_host_command_count(),
        0,
        "abandoned queued commands",
    )?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::HostCommand),
        0,
        "abandoned host command storage",
    )
}

#[test]
fn host_command_limit_rejects_without_dispatching_javascript() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::HostCommand, 0);
    let mut vm = vm_with_storage_limits(limits);
    register_round_trip(&mut vm)?;
    vm.eval(
        r"
        var limitedCallbackCalled = false;
        var limitedCommandRejected = false;
        rustRoundTrip(function limitedCallback() {
            limitedCallbackCalled = true;
        }, 'ignored').catch(function (error) {
            limitedCommandRejected = error instanceof RangeError;
        });
        ",
    )?;

    let poll = poll_once(&mut vm)?;
    ensure_usize(poll.completed(), 1, "limited host future completion")?;
    vm.run_jobs()?;
    ensure_value(
        vm.get_global("limitedCallbackCalled").as_ref(),
        &Value::Bool(false),
        "limited callback dispatch",
    )?;
    ensure_value(
        vm.get_global("limitedCommandRejected").as_ref(),
        &Value::Bool(true),
        "limited command rejection",
    )
}

#[test]
fn rejection_root_limit_completes_request_instead_of_stranding_it() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::RetainedHandle, 1);
    let mut vm = vm_with_storage_limits(limits);
    register_round_trip(&mut vm)?;
    vm.eval(
        r"
        var retainedLimitMarker = { source: 'limit' };
        var retainedLimitRejected = false;
        async function retainedLimitRejector() {
            await Promise.resolve();
            throw retainedLimitMarker;
        }
        rustRoundTrip(retainedLimitRejector, 'ignored').catch(function (error) {
            retainedLimitRejected = error instanceof RangeError;
        });
        ",
    )?;

    poll_once(&mut vm)?;
    vm.run_host_commands()?;
    let saturation = vm
        .get_global_retained("retainedLimitMarker")?
        .ok_or("retainedLimitMarker global is missing")?;
    vm.run_jobs()?;
    let poll = poll_once(&mut vm)?;
    ensure_usize(poll.completed(), 1, "retained-limit host future")?;
    ensure_usize(
        vm.pending_host_command_count(),
        0,
        "retained-limit host commands",
    )?;
    saturation.release()?;
    vm.run_jobs()?;
    ensure_value(
        vm.get_global("retainedLimitRejected").as_ref(),
        &Value::Bool(true),
        "retained-limit rejection",
    )
}

fn register_round_trip(vm: &mut Vm) -> Result<(), Error> {
    vm.register_async_host_task_typed("rustRoundTrip", |call| {
        let callback = call.required_value(0, "callback")?.retain()?;
        let message = call.string(1, "message")?.to_owned();
        let async_context = call.async_context()?;
        Ok(async move {
            let result = async_context
                .call(callback, vec![OwnedValue::String(message)])?
                .await?;
            let OwnedValue::String(result) = result else {
                return Err(
                    Error::runtime("JavaScript callback result must be an owned string").into(),
                );
            };
            Ok(format!("{result}:rust"))
        })
    })
}

fn poll_once(vm: &mut Vm) -> Result<velum::HostFuturePoll, Error> {
    let mut context = TaskContext::from_waker(Waker::noop());
    vm.poll_host_futures(&mut context)
}

fn vm_with_storage_limits(storage: VmStorageLimits) -> Vm {
    let limits = RuntimeLimits {
        storage,
        ..RuntimeLimits::default()
    };
    Engine::with_config(EngineConfig::with_default_vm_config(
        velum::VmConfig::with_limits(limits),
    ))
    .create_vm()
}

fn ensure_value(actual: Option<&Value>, expected: &Value, label: &str) -> TestResult {
    if actual == Some(expected) {
        return Ok(());
    }
    Err(format!("expected {label} {expected:?}, got {actual:?}").into())
}

fn ensure_true(value: bool, message: &str) -> TestResult {
    if value {
        return Ok(());
    }
    Err(message.into())
}

fn ensure_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {label} {expected}, got {actual}").into())
}
