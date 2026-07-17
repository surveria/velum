use std::rc::{Rc, Weak};

use parking_lot::Mutex;
use velum::{
    Engine, EngineConfig, Error, JsValueRef, OwnedValue, PropertyKeyRef, RuntimeLimits, Vm,
    VmStorageKind, VmStorageLimits,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const CALLBACK_STORE_SOURCE: &str = r"
var CallbackStore = class CallbackStore {
    install(callback) {
        this.callback = callback;
    }

    run(value) {
        return this.callback(value);
    }

    clear() {
        return delete this.callback;
    }
};
";

#[test]
fn creates_and_calls_rust_functions_without_global_bindings() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let callable = vm.create_host_function_typed("rustAdd", |call| {
        let left = call.number(0, "left")?;
        let right = call.number(1, "right")?;
        Ok(left + right)
    })?;

    if vm.get_global_retained("rustAdd")?.is_some() {
        return Err("first-class host function unexpectedly created a global binding".into());
    }
    ensure_true(
        vm.is_callable(&callable)?,
        "Rust function should be callable",
    )?;
    ensure_owned(
        &vm.call_owned(
            &callable,
            &[JsValueRef::Number(20.0), JsValueRef::Number(22.0)],
        )?,
        &OwnedValue::Number(42.0),
    )?;
    ensure_owned(
        &vm.get_property_owned((&callable).into(), PropertyKeyRef::Name("name"))?,
        &OwnedValue::String("rustAdd".to_owned()),
    )?;

    let mut other = engine.create_vm();
    let Err(error) = other.call(&callable, &[]) else {
        return Err("foreign VM invoked a first-class Rust function".into());
    };
    ensure_runtime_error(&error, "retained value belongs to another VM")?;

    callable.release()?;
    Ok(())
}

#[test]
fn javascript_storage_keeps_rust_callbacks_alive_until_gc() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(CALLBACK_STORE_SOURCE)?;
    let Some(constructor) = vm.get_global_retained("CallbackStore")? else {
        return Err("CallbackStore constructor is missing".into());
    };
    let store = vm.construct_retained(&constructor, &[])?;
    constructor.release()?;

    let before = host_callback_count(&vm)?;
    let calls = Rc::new(Mutex::new(Vec::new()));
    let calls_capture = Rc::clone(&calls);
    let owner = Rc::new(());
    let owner_weak = Rc::downgrade(&owner);
    let callback = vm.create_host_function_typed("recordValue", move |call| {
        let value = call.number(0, "value")?;
        calls_capture.lock().push(value);
        drop(Rc::clone(&owner));
        Ok(value + 1.0)
    })?;
    let expected_live = before
        .checked_add(1)
        .ok_or("host callback count overflowed in test")?;
    ensure_usize(
        host_callback_count(&vm)?,
        expected_live,
        "host callback after creation",
    )?;

    vm.call_method_owned(
        (&store).into(),
        PropertyKeyRef::Name("install"),
        &[(&callback).into()],
    )?;
    callback.release()?;
    vm.collect_garbage()?;
    ensure_weak_alive(&owner_weak, "stored callback after GC")?;
    ensure_usize(
        host_callback_count(&vm)?,
        expected_live,
        "stored host callback after GC",
    )?;

    ensure_owned(
        &vm.call_method_owned(
            (&store).into(),
            PropertyKeyRef::Name("run"),
            &[JsValueRef::Number(41.0)],
        )?,
        &OwnedValue::Number(42.0),
    )?;
    ensure_numbers(calls.lock().as_slice(), &[41.0])?;

    ensure_owned(
        &vm.call_method_owned((&store).into(), PropertyKeyRef::Name("clear"), &[])?,
        &OwnedValue::Bool(true),
    )?;
    vm.collect_garbage()?;
    ensure_weak_released(&owner_weak, "deleted callback after GC")?;
    ensure_usize(
        host_callback_count(&vm)?,
        before,
        "host callback after collection",
    )?;

    store.release()?;
    Ok(())
}

#[test]
fn callback_failures_keep_function_context() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let callback =
        vm.create_host_function("rustFail", |_call| Err(Error::runtime("camera offline")))?;

    let Err(error) = vm.call(&callback, &[]) else {
        return Err("failing Rust callback unexpectedly returned".into());
    };
    ensure_runtime_error(&error, "host function 'rustFail': camera offline")?;
    callback.release()?;
    Ok(())
}

#[test]
fn retained_root_rejection_rolls_back_host_callback_storage() -> TestResult {
    let storage = VmStorageLimits::unlimited().with_max_count(VmStorageKind::RetainedHandle, 0);
    let mut vm = vm_with_storage_limits(storage);
    vm.register_host_function_typed("prime", |_call| Ok(()))?;
    let before = vm.storage_snapshot()?;

    let Err(error) = vm.create_host_function_typed("blocked", |_call| Ok(())) else {
        return Err("retained handle limit unexpectedly accepted a host function".into());
    };
    ensure_limit_error(&error, "RetainedHandle")?;
    let after = vm.storage_snapshot()?;
    ensure_usize(
        after.count(VmStorageKind::HostCallback),
        before.count(VmStorageKind::HostCallback),
        "host callback count after rollback",
    )?;
    ensure_usize(
        after.payload_bytes(VmStorageKind::HostCallback),
        before.payload_bytes(VmStorageKind::HostCallback),
        "host callback bytes after rollback",
    )?;

    let Err(error) = vm.create_host_function_typed("", |_call| Ok(())) else {
        return Err("empty first-class host function name was accepted".into());
    };
    ensure_runtime_error(&error, "host function name must not be empty")?;
    ensure_usize(
        host_callback_count(&vm)?,
        before.count(VmStorageKind::HostCallback),
        "host callback count after invalid name",
    )
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

fn host_callback_count(vm: &Vm) -> Result<usize, Box<dyn std::error::Error>> {
    Ok(vm.storage_snapshot()?.count(VmStorageKind::HostCallback))
}

fn ensure_owned(actual: &OwnedValue, expected: &OwnedValue) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_numbers(actual: &[f64], expected: &[f64]) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected calls {expected:?}, got {actual:?}").into())
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

fn ensure_runtime_error(error: &Error, expected: &str) -> TestResult {
    if matches!(error, Error::Runtime { .. }) && error.to_string().contains(expected) {
        return Ok(());
    }
    Err(format!("expected runtime error containing {expected:?}, got {error:?}").into())
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
