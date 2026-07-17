use std::{
    cell::Cell,
    rc::Rc,
    task::{Context as TaskContext, Waker},
};

use parking_lot::Mutex;
use velum::{Engine, Error, JsValueRef, LocalValue, OwnedValue, PropertyKeyRef, RetainedValue};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const PROXY_RECEIVER_SOURCE: &str = r#"
var hostReceiverTarget = { receiverKind: "target" };
var hostReceiverProxy;
hostReceiverProxy = new Proxy(hostReceiverTarget, {
    get(target, property, receiver) {
        if (property === "receiverKind") {
            return receiver === hostReceiverProxy ? "proxy" : "wrong";
        }
        return Reflect.get(target, property, receiver);
    }
});
"#;

#[test]
fn exposes_plain_and_primitive_receivers_without_coercion() -> TestResult {
    let mut vm = Engine::new().create_vm();
    let callable =
        vm.create_host_function_typed("echoReceiver", |call| call.receiver().to_owned_value())?;

    ensure_owned(&vm.call_owned(&callable, &[])?, &OwnedValue::Undefined)?;
    ensure_owned(
        &vm.call_with_receiver_owned(&callable, JsValueRef::Null, &[])?,
        &OwnedValue::Null,
    )?;
    ensure_owned(
        &vm.call_with_receiver_owned(&callable, JsValueRef::Bool(true), &[])?,
        &OwnedValue::Bool(true),
    )?;
    ensure_owned(
        &vm.call_with_receiver_owned(&callable, JsValueRef::Number(42.0), &[])?,
        &OwnedValue::Number(42.0),
    )?;
    ensure_owned(
        &vm.call_with_receiver_owned(&callable, JsValueRef::String("camera"), &[])?,
        &OwnedValue::String("camera".to_owned()),
    )?;

    callable.release()?;
    Ok(())
}

#[test]
fn public_and_bytecode_method_calls_preserve_proxy_receiver_identity() -> TestResult {
    let mut vm = Engine::new().create_vm();
    vm.eval(PROXY_RECEIVER_SOURCE)?;
    let proxy = required_global(&vm, "hostReceiverProxy")?;
    let captures = Rc::new(Mutex::new(Vec::new()));
    let captures_for_callback = Rc::clone(&captures);
    let callable = vm.create_host_function_typed("captureReceiver", move |call| {
        capture_receiver(&captures_for_callback, call.receiver())?;
        Ok(42.0)
    })?;
    vm.set_property_or_throw(
        (&proxy).into(),
        PropertyKeyRef::Name("hostMethod"),
        (&callable).into(),
    )?;
    callable.release()?;

    ensure_owned(
        &vm.call_method_owned((&proxy).into(), PropertyKeyRef::Name("hostMethod"), &[])?,
        &OwnedValue::Number(42.0),
    )?;
    vm.eval(
        r"
        for (var hostReceiverIndex = 0; hostReceiverIndex < 2; hostReceiverIndex += 1) {
            hostReceiverProxy.hostMethod();
        }
        ",
    )?;

    let receivers = take_receivers(&captures, 3)?;
    for receiver in receivers {
        ensure_owned(
            &vm.get_property_owned((&receiver).into(), PropertyKeyRef::Name("receiverKind"))?,
            &OwnedValue::String("proxy".to_owned()),
        )?;
        receiver.release()?;
    }
    ensure_true(
        vm.delete_property((&proxy).into(), PropertyKeyRef::Name("hostMethod"))?,
        "host method should be deletable",
    )?;
    proxy.release()?;
    vm.collect_garbage()?;
    Ok(())
}

#[test]
fn async_host_callbacks_can_retain_receivers_beyond_the_call_frame() -> TestResult {
    let mut vm = Engine::new().create_vm();
    let receiver = vm.create_object()?;
    vm.set_property_or_throw(
        (&receiver).into(),
        PropertyKeyRef::Name("marker"),
        JsValueRef::String("async-receiver"),
    )?;
    let captures = Rc::new(Mutex::new(Vec::new()));
    let captures_for_callback = Rc::clone(&captures);
    let callable = vm.create_async_host_function_typed("captureAsyncReceiver", move |call| {
        capture_receiver(&captures_for_callback, call.receiver())?;
        Ok(async move { Ok(42.0) })
    })?;
    let promise =
        vm.call_with_receiver_retained(&callable, JsValueRef::Retained(&receiver), &[])?;
    callable.release()?;
    receiver.release()?;
    vm.collect_garbage()?;

    let mut receivers = take_receivers(&captures, 1)?;
    let Some(receiver_handle) = receivers.pop() else {
        return Err("async callback did not retain its receiver".into());
    };
    ensure_owned(
        &vm.get_property_owned((&receiver_handle).into(), PropertyKeyRef::Name("marker"))?,
        &OwnedValue::String("async-receiver".to_owned()),
    )?;

    let mut task_context = TaskContext::from_waker(Waker::noop());
    let poll = vm.poll_host_futures(&mut task_context)?;
    ensure_usize(poll.completed(), 1, "completed async receiver callbacks")?;
    ensure_usize(poll.pending(), 0, "pending async receiver callbacks")?;
    receiver_handle.release()?;
    promise.release()?;
    vm.collect_garbage()?;
    Ok(())
}

#[test]
fn queued_external_calls_deliver_receiver_to_rust_callbacks() -> TestResult {
    let mut vm = Engine::new().create_vm();
    let receiver = vm.create_object()?;
    vm.set_property_or_throw(
        (&receiver).into(),
        PropertyKeyRef::Name("marker"),
        JsValueRef::String("queued-receiver"),
    )?;
    let captures = Rc::new(Mutex::new(Vec::new()));
    let captures_for_callback = Rc::clone(&captures);
    let callable = vm.create_host_function_typed("captureQueuedReceiver", move |call| {
        capture_receiver(&captures_for_callback, call.receiver())?;
        Ok(42.0)
    })?;
    let request = vm.enqueue_call_with_receiver(&callable, JsValueRef::Retained(&receiver), &[])?;
    callable.release()?;
    receiver.release()?;
    vm.collect_garbage()?;

    ensure_usize(vm.run_host_commands()?, 1, "queued receiver callbacks")?;
    let mut receivers = take_receivers(&captures, 1)?;
    let Some(receiver_handle) = receivers.pop() else {
        return Err("queued callback did not retain its receiver".into());
    };
    ensure_owned(
        &vm.get_property_owned((&receiver_handle).into(), PropertyKeyRef::Name("marker"))?,
        &OwnedValue::String("queued-receiver".to_owned()),
    )?;
    ensure_usize(
        vm.pending_host_command_count(),
        1,
        "completed queued receiver response",
    )?;

    drop(request);
    ensure_usize(
        vm.pending_host_command_count(),
        0,
        "pending queued receivers",
    )?;
    receiver_handle.release()?;
    vm.collect_garbage()?;
    Ok(())
}

#[test]
fn foreign_receivers_fail_before_entering_the_host_callback() -> TestResult {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    let mut second = engine.create_vm();
    let started = Rc::new(Cell::new(false));
    let started_for_callback = Rc::clone(&started);
    let callable = first.create_host_function_typed("foreignReceiverProbe", move |_call| {
        started_for_callback.set(true);
        Ok(())
    })?;
    let foreign = second.create_object()?;

    let result = first.call_with_receiver(&callable, JsValueRef::Retained(&foreign), &[]);
    ensure_true(result.is_err(), "foreign receiver should be rejected")?;
    ensure_true(!started.get(), "foreign receiver entered the Rust callback")?;

    callable.release()?;
    foreign.release()?;
    Ok(())
}

fn capture_receiver(
    captures: &Mutex<Vec<RetainedValue>>,
    receiver: LocalValue<'_>,
) -> velum::Result<()> {
    let retained = receiver.retain()?;
    let mut captures = captures.lock();
    if captures.try_reserve(1).is_err() {
        retained.release()?;
        return Err(Error::limit("host receiver capture capacity exceeded"));
    }
    captures.push(retained);
    drop(captures);
    Ok(())
}

fn take_receivers(
    captures: &Mutex<Vec<RetainedValue>>,
    expected: usize,
) -> Result<Vec<RetainedValue>, Box<dyn std::error::Error>> {
    let receivers = std::mem::take(&mut *captures.lock());
    if receivers.len() == expected {
        return Ok(receivers);
    }
    let actual = receivers.len();
    for receiver in receivers {
        receiver.release()?;
    }
    Err(format!("expected {expected} captured receivers, got {actual}").into())
}

fn required_global(
    vm: &velum::Vm,
    name: &str,
) -> Result<RetainedValue, Box<dyn std::error::Error>> {
    let Some(value) = vm.get_global_retained(name)? else {
        return Err(format!("global '{name}' is missing").into());
    };
    Ok(value)
}

fn ensure_owned(actual: &OwnedValue, expected: &OwnedValue) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
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
