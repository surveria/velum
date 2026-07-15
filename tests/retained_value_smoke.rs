use std::rc::Rc;

use parking_lot::Mutex;
use velum::{Engine, Error, OwnedValue, RetainedValue, VmRootKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn retains_objects_and_functions_as_explicit_roots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let object = vm.eval_retained("({ camera: 42 })")?;
    ensure_same_identity(&vm, &object)?;
    ensure_text(vm.retained_type_name(&object)?, "object", "object type")?;
    ensure_retained_roots(&vm, 1)?;

    let function = vm.eval_retained("(function camera() { return 42; })")?;
    ensure_text(
        vm.retained_type_name(&function)?,
        "function",
        "function type",
    )?;
    ensure_retained_roots(&vm, 2)?;

    object.release()?;
    ensure_retained_roots(&vm, 1)?;
    function.release()?;
    ensure_retained_roots(&vm, 0)
}

#[test]
fn retains_compiled_global_and_portable_primitive_results() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(r#""compiled-camera""#)?;
    let compiled = vm.eval_compiled_retained(&script)?;
    ensure_owned(
        &vm.retained_to_owned(&compiled)?,
        &OwnedValue::String("compiled-camera".to_owned()),
    )?;

    vm.eval("var camera = 42;")?;
    let Some(global) = vm.get_global_retained("camera")? else {
        return Err("expected camera global to be retained".into());
    };
    ensure_owned(&vm.retained_to_owned(&global)?, &OwnedValue::Number(42.0))?;
    if vm.get_global_retained("missing")?.is_some() {
        return Err("missing global unexpectedly produced a retained handle".into());
    }

    compiled.release()?;
    global.release()?;
    ensure_retained_roots(&vm, 0)
}

#[test]
fn rejects_a_foreign_vm_before_resolving_a_colliding_slot() -> TestResult {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    let handle = first.eval_retained("({ owner: 'first' })")?;
    let mut second = engine.create_vm();
    let second_handle = second.eval_retained("({ owner: 'second' })")?;

    let Err(error) = second.retained_type_name(&handle) else {
        return Err("foreign VM accepted a retained handle".into());
    };
    ensure_runtime_error(&error, "retained value belongs to another VM")?;
    ensure_text(first.retained_type_name(&handle)?, "object", "owner type")?;

    handle.release()?;
    second_handle.release()?;
    Ok(())
}

#[test]
fn callback_local_values_can_be_retained_beyond_the_host_call() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let retained = Rc::new(Mutex::new(None));
    let capture = Rc::clone(&retained);
    vm.register_host_function_typed("retainCamera", move |call| {
        let handle = call.required_value(0, "camera")?.retain()?;
        *capture.lock() = Some(handle);
        Ok(())
    })?;

    vm.eval("retainCamera({ lens: 42 });")?;
    ensure_retained_roots(&vm, 1)?;
    let Some(handle) = retained.lock().take() else {
        return Err("host callback did not retain its argument".into());
    };
    ensure_text(
        vm.retained_type_name(&handle)?,
        "object",
        "callback value type",
    )?;
    handle.release()?;
    ensure_retained_roots(&vm, 0)
}

#[test]
fn dropping_a_handle_releases_its_root_and_reused_slots_stay_valid() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let first = vm.eval_retained("({ first: true })")?;
    ensure_retained_roots(&vm, 1)?;
    drop(first);
    ensure_retained_roots(&vm, 0)?;

    let second = vm.eval_retained("({ second: true })")?;
    ensure_text(
        vm.retained_type_name(&second)?,
        "object",
        "reused slot type",
    )?;
    ensure_retained_roots(&vm, 1)?;
    second.release()?;
    ensure_retained_roots(&vm, 0)
}

#[test]
fn explicit_release_reports_owner_teardown() -> TestResult {
    let engine = Engine::new();
    let handle = {
        let mut vm = engine.create_vm();
        vm.eval_retained("({ camera: 42 })")?
    };

    let Err(error) = handle.release() else {
        return Err("release succeeded after VM teardown".into());
    };
    ensure_runtime_error(&error, "retained value owner has been torn down")
}

fn ensure_same_identity(vm: &velum::Vm, handle: &RetainedValue) -> TestResult {
    if vm.identity() == handle.identity() {
        return Ok(());
    }
    Err("retained handle did not preserve its VM identity".into())
}

fn ensure_retained_roots(vm: &velum::Vm, expected: usize) -> TestResult {
    let actual = vm.root_snapshot()?.count(VmRootKind::RetainedHandle);
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected} retained roots, got {actual}").into())
}

fn ensure_owned(actual: &OwnedValue, expected: &OwnedValue) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_text(actual: &str, expected: &str, label: &str) -> TestResult {
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
