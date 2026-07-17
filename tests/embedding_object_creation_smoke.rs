use velum::{
    Engine, EngineConfig, Error, JsValueRef, ObjectOptions, OwnedValue, PropertyKeyRef,
    RuntimeLimits, Vm, VmStorageKind, VmStorageLimits,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn creates_ordinary_objects_without_source_generation() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let object = vm.create_object()?;

    ensure_true(
        vm.set_property(
            (&object).into(),
            PropertyKeyRef::Name("answer"),
            JsValueRef::Number(42.0),
        )?,
        "ordinary object property assignment",
    )?;
    ensure_owned(
        &vm.get_property_owned((&object).into(), PropertyKeyRef::Name("answer"))?,
        &OwnedValue::Number(42.0),
    )?;
    ensure_owned(
        &vm.call_method_owned(
            (&object).into(),
            PropertyKeyRef::Name("hasOwnProperty"),
            &[JsValueRef::String("answer")],
        )?,
        &OwnedValue::Bool(true),
    )?;

    object.release()?;
    vm.collect_garbage()?;
    Ok(())
}

#[test]
fn explicit_and_null_prototypes_use_ordinary_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let prototype = vm.create_object()?;
    vm.set_property_or_throw(
        (&prototype).into(),
        PropertyKeyRef::Name("inherited"),
        JsValueRef::Number(7.0),
    )?;
    let child = vm.create_object_with_options(ObjectOptions::new().with_prototype(&prototype))?;
    let null_object = vm.create_object_with_options(ObjectOptions::new().with_null_prototype())?;

    ensure_owned(
        &vm.get_property_owned((&child).into(), PropertyKeyRef::Name("inherited"))?,
        &OwnedValue::Number(7.0),
    )?;
    vm.set_property_or_throw(
        (&prototype).into(),
        PropertyKeyRef::Name("inherited"),
        JsValueRef::Number(9.0),
    )?;
    ensure_owned(
        &vm.get_property_owned((&child).into(), PropertyKeyRef::Name("inherited"))?,
        &OwnedValue::Number(9.0),
    )?;
    ensure_owned(
        &vm.get_property_owned((&null_object).into(), PropertyKeyRef::Name("toString"))?,
        &OwnedValue::Undefined,
    )?;

    prototype.release()?;
    vm.collect_garbage()?;
    ensure_owned(
        &vm.get_property_owned((&child).into(), PropertyKeyRef::Name("inherited"))?,
        &OwnedValue::Number(9.0),
    )?;

    child.release()?;
    null_object.release()?;
    vm.collect_garbage()?;
    Ok(())
}

#[test]
fn callable_values_can_be_explicit_prototypes() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let callable = vm.create_host_function_typed("prototypeCallable", |_call| Ok(17.0))?;
    vm.set_property_or_throw(
        (&callable).into(),
        PropertyKeyRef::Name("kind"),
        JsValueRef::String("callable-prototype"),
    )?;
    let object = vm.create_object_with_options(ObjectOptions::new().with_prototype(&callable))?;

    ensure_owned(
        &vm.get_property_owned((&object).into(), PropertyKeyRef::Name("kind"))?,
        &OwnedValue::String("callable-prototype".to_owned()),
    )?;
    ensure_false(
        vm.is_callable(&object)?,
        "ordinary object with callable prototype became callable",
    )?;

    callable.release()?;
    vm.collect_garbage()?;
    ensure_owned(
        &vm.get_property_owned((&object).into(), PropertyKeyRef::Name("kind"))?,
        &OwnedValue::String("callable-prototype".to_owned()),
    )?;

    object.release()?;
    vm.collect_garbage()?;
    Ok(())
}

#[test]
fn rejects_foreign_and_primitive_prototypes_before_allocation() -> TestResult {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    let mut second = engine.create_vm();
    let foreign = second.create_object()?;
    let before_foreign = first.storage_snapshot()?;
    let Err(error) =
        first.create_object_with_options(ObjectOptions::new().with_prototype(&foreign))
    else {
        return Err("foreign prototype unexpectedly created an object".into());
    };
    ensure_runtime_error(&error, "retained value belongs to another VM")?;
    ensure_snapshot(&first, &before_foreign, "foreign prototype rejection")?;

    let primitive = first.eval_retained("17")?;
    let before_primitive = first.storage_snapshot()?;
    let Err(error) =
        first.create_object_with_options(ObjectOptions::new().with_prototype(&primitive))
    else {
        return Err("primitive prototype unexpectedly created an object".into());
    };
    ensure_runtime_error(
        &error,
        "embedding object prototype must be an object or null",
    )?;
    ensure_snapshot(&first, &before_primitive, "primitive prototype rejection")?;

    foreign.release()?;
    primitive.release()?;
    Ok(())
}

#[test]
fn root_and_object_limits_roll_back_without_leaking_objects() -> TestResult {
    let root_limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::RetainedHandle, 0);
    let mut root_limited = vm_with_storage_limits(root_limits, None);
    drop(root_limited.eval("({ priming: true })")?);
    root_limited.collect_garbage()?;
    let before = root_limited.storage_snapshot()?;

    let Err(error) = root_limited.create_object() else {
        return Err("retained-root limit unexpectedly accepted an object".into());
    };
    ensure_limit_error(&error, "RetainedHandle")?;
    ensure_snapshot(&root_limited, &before, "retained-root rollback")?;

    let mut object_limited = vm_with_storage_limits(VmStorageLimits::unlimited(), Some(0));
    let Err(error) =
        object_limited.create_object_with_options(ObjectOptions::new().with_null_prototype())
    else {
        return Err("object limit unexpectedly accepted an object".into());
    };
    ensure_limit_error(&error, "Object")?;
    ensure_usize(
        object_limited
            .storage_snapshot()?
            .count(VmStorageKind::Object),
        0,
        "object records after limit rejection",
    )
}

fn vm_with_storage_limits(storage: VmStorageLimits, max_objects: Option<usize>) -> Vm {
    let mut limits = RuntimeLimits {
        storage,
        ..RuntimeLimits::default()
    };
    if let Some(max_objects) = max_objects {
        limits.max_objects = max_objects;
    }
    Engine::with_config(EngineConfig::with_default_vm_config(
        velum::VmConfig::with_limits(limits),
    ))
    .create_vm()
}

fn ensure_snapshot(vm: &Vm, expected: &velum::VmStorageSnapshot, label: &str) -> TestResult {
    let actual = vm.storage_snapshot()?;
    if actual == *expected {
        return Ok(());
    }
    Err(format!("{label} changed storage: expected {expected:?}, got {actual:?}").into())
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
