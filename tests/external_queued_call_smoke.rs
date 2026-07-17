use std::{
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll, Waker},
};

use velum::{
    Engine, EngineConfig, HostFutureError, JsValueRef, PropertyKeyRef, QueuedCallRequest,
    QueuedCallResult, RuntimeLimits, Value, Vm, VmStorageKind, VmStorageLimits,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const EVENT_COUNT: u32 = 10;

#[test]
fn repeated_external_calls_preserve_receiver_arguments_fifo_and_roots() -> TestResult {
    let mut vm = Engine::new().create_vm();
    vm.eval(
        r"
        var eventTrace = [];
        var eventTotal = 0;
        var eventHits = 0;
        var eventReceiver = { label: 'timer', total: 0 };
        var eventPayload = { hits: 0 };
        function onExternalEvent(index, payload) {
            this.total += index;
            payload.hits += 1;
            eventTotal = this.total;
            eventHits = payload.hits;
            eventTrace.push(this.label + ':' + index);
            return Promise.resolve(index * 2);
        }
        ",
    )?;
    let callback = retained_global(&vm, "onExternalEvent")?;
    let receiver = retained_global(&vm, "eventReceiver")?;
    let payload = retained_global(&vm, "eventPayload")?;
    vm.eval("onExternalEvent = null; eventReceiver = null; eventPayload = null;")?;

    let mut requests = Vec::new();
    requests
        .try_reserve(usize::try_from(EVENT_COUNT)?)
        .map_err(|_| "external request capacity exceeded")?;
    for index in 1..=EVENT_COUNT {
        let number = f64::from(index);
        let args = [JsValueRef::Number(number), JsValueRef::Retained(&payload)];
        let request =
            vm.enqueue_call_with_receiver(&callback, JsValueRef::Retained(&receiver), &args)?;
        requests.push(Box::pin(request));
    }
    ensure_usize(
        vm.pending_host_command_count(),
        usize::try_from(EVENT_COUNT)?,
        "queued external calls",
    )?;
    callback.release()?;
    receiver.release()?;
    payload.release()?;
    vm.collect_garbage()?;

    ensure_usize(
        vm.run_host_commands()?,
        usize::try_from(EVENT_COUNT)?,
        "dispatched external calls",
    )?;
    vm.run_jobs()?;
    for (offset, request) in requests.iter_mut().enumerate() {
        let result = ready_request(request)?;
        let QueuedCallResult::Owned(value) = result else {
            return Err("external numeric result was retained unexpectedly".into());
        };
        let expected = f64::from(u32::try_from(offset)?.saturating_add(1)) * 2.0;
        ensure_value(
            Some(&Value::from(value)),
            &Value::Number(expected),
            "external call result",
        )?;
    }
    ensure_usize(vm.pending_host_command_count(), 0, "completed calls")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::RetainedHandle),
        0,
        "released duplicated roots",
    )?;
    ensure_value(
        vm.get_global("eventTotal").as_ref(),
        &Value::Number(55.0),
        "receiver state",
    )?;
    ensure_value(
        vm.get_global("eventHits").as_ref(),
        &Value::Number(10.0),
        "retained argument state",
    )?;
    let trace = vm.eval("eventTrace.join('|')")?;
    ensure_value(
        Some(&trace),
        &Value::from(
            "timer:1|timer:2|timer:3|timer:4|timer:5|timer:6|timer:7|timer:8|timer:9|timer:10",
        ),
        "external FIFO trace",
    )
}

#[test]
fn queued_async_call_returns_a_rooted_object_result() -> TestResult {
    let mut vm = Engine::new().create_vm();
    vm.eval(
        r"
        var makeQueuedObject = (function () {
            var queuedObject = { answer: 42 };
            return async function makeQueuedObjectClosure() {
                await Promise.resolve();
                return queuedObject;
            };
        })();
        ",
    )?;
    let callable = retained_global(&vm, "makeQueuedObject")?;
    let mut request = Box::pin(vm.enqueue_call(&callable, &[])?);
    vm.eval("makeQueuedObject = null;")?;
    callable.release()?;
    vm.collect_garbage()?;

    ensure_usize(vm.run_host_commands()?, 1, "async object dispatch")?;
    vm.run_jobs()?;
    vm.collect_garbage()?;
    let QueuedCallResult::Retained(result) = ready_request(&mut request)? else {
        return Err("queued object result was copied unexpectedly".into());
    };
    ensure_usize(vm.pending_host_command_count(), 0, "async object request")?;
    let answer = vm.get_property(
        JsValueRef::Retained(&result),
        PropertyKeyRef::Name("answer"),
    )?;
    ensure_value(
        Some(&answer),
        &Value::Number(42.0),
        "retained object result",
    )?;
    result.release()?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::RetainedHandle),
        0,
        "released async object result",
    )
}

#[test]
fn queued_rejection_preserves_exact_javascript_identity() -> TestResult {
    let mut vm = Engine::new().create_vm();
    vm.eval(
        r"
        var rejectQueuedCall = (function () {
            var queuedRejection = { source: 'external-event' };
            return async function rejectQueuedCallClosure() {
                await Promise.resolve();
                throw queuedRejection;
            };
        })();
        ",
    )?;
    let callable = retained_global(&vm, "rejectQueuedCall")?;
    let mut request = Box::pin(vm.enqueue_call(&callable, &[])?);
    vm.eval("rejectQueuedCall = null;")?;
    callable.release()?;
    vm.run_host_commands()?;
    vm.run_jobs()?;
    vm.collect_garbage()?;

    let Poll::Ready(Err(HostFutureError::JavaScript(reason))) = poll_request(&mut request) else {
        return Err("queued rejection did not retain its JavaScript value".into());
    };
    let source = vm.get_property(
        JsValueRef::Retained(&reason),
        PropertyKeyRef::Name("source"),
    )?;
    ensure_value(
        Some(&source),
        &Value::from("external-event"),
        "queued rejection identity",
    )?;
    reason.release()?;
    Ok(())
}

#[test]
fn foreign_queued_inputs_fail_without_leaking_transactional_roots() -> TestResult {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    let mut second = engine.create_vm();
    first.eval("function firstCallback(value) { return value; }")?;
    second.eval("var foreignReceiver = {};")?;
    let callback = retained_global(&first, "firstCallback")?;
    let foreign_receiver = retained_global(&second, "foreignReceiver")?;

    let result =
        first.enqueue_call_with_receiver(&callback, JsValueRef::Retained(&foreign_receiver), &[]);
    ensure_true(result.is_err(), "foreign receiver was accepted")?;
    ensure_usize(
        first.pending_host_command_count(),
        0,
        "foreign queued calls",
    )?;
    ensure_usize(
        first
            .storage_snapshot()?
            .count(VmStorageKind::RetainedHandle),
        1,
        "transactional callable root",
    )?;
    callback.release()?;
    foreign_receiver.release()?;
    Ok(())
}

#[test]
fn dropping_external_request_abandons_dispatch_and_releases_roots() -> TestResult {
    let mut vm = Engine::new().create_vm();
    vm.eval(
        r"
        var droppedEventCalled = false;
        var droppedPayload = {};
        function droppedEvent(payload) {
            droppedEventCalled = payload !== null;
        }
        ",
    )?;
    let callback = retained_global(&vm, "droppedEvent")?;
    let payload = retained_global(&vm, "droppedPayload")?;
    let args = [JsValueRef::Retained(&payload)];
    let request = vm.enqueue_call(&callback, &args)?;
    ensure_usize(vm.pending_host_command_count(), 1, "pending dropped event")?;
    drop(request);
    ensure_usize(vm.pending_host_command_count(), 0, "abandoned event")?;
    ensure_usize(vm.run_host_commands()?, 0, "abandoned dispatch")?;
    ensure_value(
        vm.get_global("droppedEventCalled").as_ref(),
        &Value::Bool(false),
        "dropped callback",
    )?;
    callback.release()?;
    payload.release()?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::RetainedHandle),
        0,
        "dropped request roots",
    )
}

#[test]
fn queued_payload_limit_releases_duplicated_callable_root() -> TestResult {
    let storage = VmStorageLimits::unlimited()
        .with_max_payload_bytes(VmStorageKind::HostCommand, "abc".len());
    let mut vm = vm_with_storage_limits(storage);
    vm.eval("function limitedExternal(value) { return value; }")?;
    let callback = retained_global(&vm, "limitedExternal")?;
    let args = [JsValueRef::String("four")];
    let result = vm.enqueue_call(&callback, &args);
    ensure_true(result.is_err(), "oversized external payload was accepted")?;
    ensure_usize(vm.pending_host_command_count(), 0, "limited external calls")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::RetainedHandle),
        1,
        "limited duplicated roots",
    )?;
    callback.release()?;
    Ok(())
}

#[test]
fn queued_exact_string_result_remains_vm_local_and_rooted() -> TestResult {
    let mut vm = Engine::new().create_vm();
    vm.eval(r#"function exactQueuedString() { return "\uD800"; }"#)?;
    let callback = retained_global(&vm, "exactQueuedString")?;
    let mut request = Box::pin(vm.enqueue_call(&callback, &[])?);
    callback.release()?;
    vm.run_host_commands()?;
    vm.run_jobs()?;

    let QueuedCallResult::Retained(result) = ready_request(&mut request)? else {
        return Err("ill-formed exact string was copied unexpectedly".into());
    };
    let length = vm.get_property(
        JsValueRef::Retained(&result),
        PropertyKeyRef::Name("length"),
    )?;
    ensure_value(Some(&length), &Value::Number(1.0), "exact string length")?;
    result.release()?;
    Ok(())
}

#[test]
fn queued_request_reports_vm_teardown_to_external_executor() -> TestResult {
    let mut request = {
        let mut vm = Engine::new().create_vm();
        vm.eval("function tornDownQueuedCall() { return 42; }")?;
        let callback = retained_global(&vm, "tornDownQueuedCall")?;
        Box::pin(vm.enqueue_call(&callback, &[])?)
    };

    let Poll::Ready(Err(HostFutureError::Engine(error))) = poll_request(&mut request) else {
        return Err("torn-down queued request did not return an engine error".into());
    };
    ensure_true(
        error.to_string().contains("owner was torn down"),
        "torn-down request error lost context",
    )
}

fn retained_global(
    vm: &Vm,
    name: &str,
) -> Result<velum::RetainedValue, Box<dyn std::error::Error>> {
    vm.get_global_retained(name)?
        .ok_or_else(|| format!("{name} global is missing").into())
}

fn ready_request(
    request: &mut Pin<Box<QueuedCallRequest>>,
) -> Result<QueuedCallResult, Box<dyn std::error::Error>> {
    match poll_request(request) {
        Poll::Ready(result) => result.map_err(Into::into),
        Poll::Pending => Err("queued call request remained pending".into()),
    }
}

fn poll_request(
    request: &mut Pin<Box<QueuedCallRequest>>,
) -> Poll<velum::HostTaskResult<QueuedCallResult>> {
    let mut context = TaskContext::from_waker(Waker::noop());
    request.as_mut().poll(&mut context)
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
