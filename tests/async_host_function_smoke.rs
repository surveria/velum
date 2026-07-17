use std::{
    cell::Cell,
    future,
    rc::{Rc, Weak},
    task::{Context as TaskContext, Waker},
};

use velum::{
    Engine, EngineConfig, Error, JsValueRef, PropertyKeyRef, RuntimeLimits, Value, Vm, VmRootKind,
    VmStorageKind, VmStorageLimits,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn async_host_function_settles_through_promise_jobs() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.register_async_host_function_typed("rustAdd", |call| {
        let left = call.number(0, "left")?;
        let right = call.number(1, "right")?;
        Ok(async move { Ok(left + right) })
    })?;
    vm.eval(
        r"
        var asyncResult = 'pending';
        rustAdd(20, 22).then(function (value) { asyncResult = value; });
        ",
    )?;

    ensure_usize(vm.pending_host_future_count(), 1, "pending host futures")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::HostFuture),
        1,
        "host future storage",
    )?;
    ensure_usize(
        vm.root_snapshot()?.count(VmRootKind::HostFuture),
        1,
        "host future Promise roots",
    )?;
    ensure_value(
        vm.get_global("asyncResult").as_ref(),
        &Value::from("pending"),
        "result before polling",
    )?;

    vm.collect_garbage()?;
    let poll = poll_once(&mut vm)?;
    ensure_usize(poll.completed(), 1, "completed host futures")?;
    ensure_usize(poll.pending(), 0, "remaining host futures")?;
    ensure_true(poll.is_idle(), "host future poll should be idle")?;
    ensure_usize(vm.pending_job_count(), 1, "queued Promise reactions")?;
    ensure_value(
        vm.get_global("asyncResult").as_ref(),
        &Value::from("pending"),
        "result before draining jobs",
    )?;

    ensure_usize(vm.run_jobs()?, 1, "executed Promise reactions")?;
    ensure_value(
        vm.get_global("asyncResult").as_ref(),
        &Value::Number(42.0),
        "fulfilled async result",
    )?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::HostFuture),
        0,
        "released host future storage",
    )
}

#[test]
fn first_class_async_callable_survives_javascript_storage() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval("var asyncHolder = {}; var storedResult = 'pending';")?;
    let holder = vm
        .get_global_retained("asyncHolder")?
        .ok_or("asyncHolder global is missing")?;
    let owner = Rc::new(());
    let owner_weak = Rc::downgrade(&owner);
    let callable = vm.create_async_host_function_typed("doubleLater", move |call| {
        let value = call.number(0, "value")?;
        let future_owner = Rc::clone(&owner);
        Ok(async move {
            drop(future_owner);
            Ok(value * 2.0)
        })
    })?;
    ensure_true(
        vm.get_global_retained("doubleLater")?.is_none(),
        "first-class async callable unexpectedly installed a global",
    )?;
    ensure_true(
        vm.set_property(
            (&holder).into(),
            PropertyKeyRef::Name("work"),
            (&callable).into(),
        )?,
        "storing async callable should succeed",
    )?;
    callable.release()?;
    vm.collect_garbage()?;
    ensure_weak_alive(&owner_weak, "stored async callable capture")?;

    vm.eval("asyncHolder.work(21).then(function (value) { storedResult = value; });")?;
    poll_once(&mut vm)?;
    vm.run_jobs()?;
    ensure_value(
        vm.get_global("storedResult").as_ref(),
        &Value::Number(42.0),
        "stored async callable result",
    )?;

    ensure_true(
        vm.delete_property(JsValueRef::Retained(&holder), PropertyKeyRef::Name("work"))?,
        "deleting async callable should succeed",
    )?;
    vm.collect_garbage()?;
    ensure_weak_released(&owner_weak, "deleted async callable capture")?;
    holder.release()?;
    Ok(())
}

#[test]
fn cancellation_drops_future_and_rejects_promise_once() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let dropped = Rc::new(Cell::new(false));
    let dropped_capture = Rc::clone(&dropped);
    vm.register_async_host_function_typed("never", move |_call| {
        let drop_probe = DropProbe(Rc::clone(&dropped_capture));
        Ok(async move {
            future::pending::<()>().await;
            drop(drop_probe);
            Ok(42_f64)
        })
    })?;
    vm.eval(
        r"
        var cancellation = 'pending';
        never().catch(function (error) { cancellation = error.message; });
        ",
    )?;

    let poll = poll_once(&mut vm)?;
    ensure_usize(poll.completed(), 0, "completed pending futures")?;
    ensure_usize(poll.pending(), 1, "still pending futures")?;
    ensure_false(dropped.get(), "future dropped before cancellation")?;

    ensure_usize(vm.cancel_host_futures()?, 1, "cancelled host futures")?;
    ensure_true(dropped.get(), "cancelled future capture should be dropped")?;
    ensure_usize(
        vm.pending_host_future_count(),
        0,
        "futures after cancellation",
    )?;
    ensure_usize(vm.pending_job_count(), 1, "cancellation rejection jobs")?;
    ensure_usize(vm.run_jobs()?, 1, "executed cancellation jobs")?;
    ensure_value(
        vm.get_global("cancellation").as_ref(),
        &Value::from("async host function was cancelled"),
        "cancellation rejection",
    )?;
    ensure_usize(vm.cancel_host_futures()?, 0, "repeated cancellation")
}

#[test]
fn async_rust_errors_reject_with_function_context() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.register_async_host_function("rustOffline", |_call| {
        Ok(async move { Err(Error::runtime("camera offline")) })
    })?;
    vm.eval(
        r"
        var rejectionName = 'pending';
        var rejectionMessage = 'pending';
        rustOffline().catch(function (error) {
            rejectionName = error.name;
            rejectionMessage = error.message;
        });
        ",
    )?;

    poll_once(&mut vm)?;
    vm.run_jobs()?;
    ensure_value(
        vm.get_global("rejectionName").as_ref(),
        &Value::from("Error"),
        "rejection error name",
    )?;
    let Some(Value::String(message)) = vm.get_global("rejectionMessage") else {
        return Err("async rejection message is not a string".into());
    };
    let Some(message) = message.as_utf8() else {
        return Err("async rejection message is not well-formed UTF-8".into());
    };
    ensure_contains(
        message,
        "host function 'rustOffline': camera offline",
        "async rejection context",
    )
}

#[test]
fn host_future_limit_rejects_before_starting_callback() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::HostFuture, 0);
    let mut vm = vm_with_storage_limits(limits);
    let started = Rc::new(Cell::new(false));
    let started_capture = Rc::clone(&started);
    vm.register_async_host_function_typed("blocked", move |_call| {
        started_capture.set(true);
        Ok(async move { Ok(42_f64) })
    })?;
    let before = vm.storage_snapshot()?;

    let Err(error) = vm.eval("blocked()") else {
        return Err("host future limit unexpectedly accepted async work".into());
    };
    ensure_limit_error(&error, "HostFuture")?;
    ensure_false(started.get(), "callback started before future admission")?;
    ensure_usize(vm.pending_host_future_count(), 0, "limited host futures")?;
    let after = vm.storage_snapshot()?;
    ensure_usize(
        after.count(VmStorageKind::Promise),
        before.count(VmStorageKind::Promise),
        "Promise count after limit rejection",
    )
}

#[test]
fn async_start_errors_return_rejected_promises() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.register_async_host_function_typed("needsNumber", |call| {
        let value = call.number(0, "value")?;
        Ok(async move { Ok(value) })
    })?;
    vm.eval(
        r"
        var startFailure = 'not-a-promise';
        var startFailurePromise = needsNumber('wrong');
        startFailurePromise.catch(function (error) {
            startFailure = error.message;
        });
        ",
    )?;

    ensure_usize(
        vm.pending_host_future_count(),
        0,
        "future after start error",
    )?;
    ensure_usize(vm.pending_job_count(), 0, "jobs drained by eval")?;
    let Some(Value::String(message)) = vm.get_global("startFailure") else {
        return Err("start failure message is not a string".into());
    };
    let Some(message) = message.as_utf8() else {
        return Err("start failure message is not well-formed UTF-8".into());
    };
    ensure_contains(
        message,
        "host function 'needsNumber'",
        "start failure context",
    )
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

struct DropProbe(Rc<Cell<bool>>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(true);
    }
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

fn ensure_false(value: bool, message: &str) -> TestResult {
    if !value {
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

fn ensure_contains(actual: &str, expected: &str, label: &str) -> TestResult {
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!("expected {label} to contain {expected:?}, got {actual:?}").into())
}

fn ensure_limit_error(error: &Error, expected: &str) -> TestResult {
    if matches!(error, Error::ResourceLimit { .. }) && error.to_string().contains(expected) {
        return Ok(());
    }
    Err(format!("expected limit error containing {expected:?}, got {error:?}").into())
}

fn ensure_weak_alive(value: &Weak<()>, label: &str) -> TestResult {
    if value.upgrade().is_some() {
        return Ok(());
    }
    Err(format!("{label} was released unexpectedly").into())
}

fn ensure_weak_released(value: &Weak<()>, label: &str) -> TestResult {
    if value.upgrade().is_none() {
        return Ok(());
    }
    Err(format!("{label} remains alive unexpectedly").into())
}
