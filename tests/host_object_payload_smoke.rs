use std::{cell::Cell, rc::Rc};

use velum::{
    Engine, EngineConfig, Error, HostObjectOptions, JsValueRef, OwnedValue, PropertyKeyRef,
    RuntimeLimits, Vm, VmObjectEdgeKind, VmStorageKind, VmStorageLimits,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[derive(Debug)]
struct SharedState {
    value: Cell<u32>,
}

#[derive(Debug)]
struct DropProbe {
    drops: Rc<Cell<usize>>,
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.drops.set(self.drops.get().saturating_add(1));
    }
}

#[derive(Debug)]
struct DifferentPayload;

#[test]
fn host_wrappers_keep_ordinary_object_semantics_and_share_only_payload() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let prototype = vm.eval_retained("({ inherited: 7 })")?;
    let object = vm.create_host_object(
        SharedState {
            value: Cell::new(1),
        },
        HostObjectOptions::new(24).with_prototype(&prototype),
    )?;
    ensure_true(
        vm.set_property(
            (&object).into(),
            PropertyKeyRef::Name("ownValue"),
            JsValueRef::Number(9.0),
        )?,
        "host wrapper own-property assignment",
    )?;
    let cloned = vm.clone_host_object(&object)?;

    let original_payload = vm.host_payload::<SharedState>(&object)?;
    let cloned_payload = vm.host_payload::<SharedState>(&cloned)?;
    ensure_true(
        std::ptr::eq(original_payload, cloned_payload),
        "cloned wrapper should share the exact payload allocation",
    )?;
    original_payload.value.set(42);
    ensure_usize(
        vm.host_payload::<SharedState>(&cloned)?.value.get() as usize,
        42,
        "shared payload mutation",
    )?;
    ensure_owned(
        &vm.get_property_owned((&object).into(), PropertyKeyRef::Name("ownValue"))?,
        &OwnedValue::Number(9.0),
    )?;
    ensure_owned(
        &vm.get_property_owned((&cloned).into(), PropertyKeyRef::Name("ownValue"))?,
        &OwnedValue::Undefined,
    )?;
    ensure_owned(
        &vm.get_property_owned((&cloned).into(), PropertyKeyRef::Name("inherited"))?,
        &OwnedValue::Number(7.0),
    )?;

    let snapshot = vm.storage_snapshot()?;
    ensure_usize(
        snapshot.count(VmStorageKind::HostInstance),
        2,
        "host wrapper count",
    )?;
    ensure_usize(
        snapshot.count(VmStorageKind::HostPayload),
        1,
        "shared host payload count",
    )?;
    ensure_usize(
        snapshot.payload_bytes(VmStorageKind::HostPayload),
        24,
        "shared host payload bytes",
    )?;
    vm.update_host_payload_bytes(&cloned, 32)?;
    ensure_host_storage(&vm, 2, 1, 32)?;

    object.release()?;
    cloned.release()?;
    prototype.release()?;
    vm.collect_garbage()?;
    ensure_host_storage(&vm, 0, 0, 0)
}

#[test]
fn traced_values_are_internal_edges_and_payload_cycles_are_collectible() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let child = vm.eval_retained("({ marker: 42 })")?;
    let internal_edges_before = vm
        .object_edge_snapshot()?
        .count(VmObjectEdgeKind::InternalSlot);
    let drops = Rc::new(Cell::new(0));
    let wrapper = vm.create_host_object(
        DropProbe {
            drops: Rc::clone(&drops),
        },
        HostObjectOptions::new(8).with_traced_values(std::slice::from_ref(&child)),
    )?;
    let cloned = vm.clone_host_object(&wrapper)?;
    let expected_internal_edges = internal_edges_before
        .checked_add(2)
        .ok_or("internal edge count overflowed in test")?;
    ensure_usize(
        vm.object_edge_snapshot()?
            .count(VmObjectEdgeKind::InternalSlot),
        expected_internal_edges,
        "host payload internal edge count",
    )?;
    vm.set_property_or_throw(
        (&child).into(),
        PropertyKeyRef::Name("wrapper"),
        (&cloned).into(),
    )?;

    child.release()?;
    wrapper.release()?;
    vm.collect_garbage()?;
    ensure_usize(
        drops.get(),
        0,
        "payload drops while cloned wrapper is rooted",
    )?;
    ensure_host_storage(&vm, 1, 1, 8)?;

    cloned.release()?;
    vm.collect_garbage()?;
    ensure_usize(drops.get(), 1, "payload drops after cycle collection")?;
    ensure_host_storage(&vm, 0, 0, 0)
}

#[test]
fn host_wrapper_can_select_a_null_prototype_without_source() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let host = vm.create_host_object(
        SharedState {
            value: Cell::new(5),
        },
        HostObjectOptions::new(4).with_null_prototype(),
    )?;

    ensure_owned(
        &vm.get_property_owned((&host).into(), PropertyKeyRef::Name("toString"))?,
        &OwnedValue::Undefined,
    )?;
    ensure_usize(
        vm.host_payload::<SharedState>(&host)?.value.get() as usize,
        5,
        "null-prototype host payload",
    )?;

    host.release()?;
    vm.collect_garbage()?;
    ensure_host_storage(&vm, 0, 0, 0)
}

#[test]
fn checked_access_rejects_foreign_plain_and_mismatched_values() -> TestResult {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    let mut second = engine.create_vm();
    let foreign_prototype = second.eval_retained("({ foreign: true })")?;
    let rejected_drops = Rc::new(Cell::new(0));
    let Err(error) = first.create_host_object(
        DropProbe {
            drops: Rc::clone(&rejected_drops),
        },
        HostObjectOptions::new(1).with_prototype(&foreign_prototype),
    ) else {
        return Err("foreign prototype unexpectedly created a host object".into());
    };
    ensure_runtime_error(&error, "retained value belongs to another VM")?;
    ensure_usize(rejected_drops.get(), 1, "rejected payload drops")?;
    ensure_host_storage(&first, 0, 0, 0)?;

    let host = first.create_host_object(
        SharedState {
            value: Cell::new(3),
        },
        HostObjectOptions::new(4),
    )?;
    let plain = first.eval_retained("({ plain: true })")?;
    let primitive = first.eval_retained("17")?;
    let invalid_prototype_drops = Rc::new(Cell::new(0));
    let Err(error) = first.create_host_object(
        DropProbe {
            drops: Rc::clone(&invalid_prototype_drops),
        },
        HostObjectOptions::new(2).with_prototype(&primitive),
    ) else {
        return Err("primitive prototype unexpectedly created a host object".into());
    };
    ensure_runtime_error(
        &error,
        "embedding object prototype must be an object or null",
    )?;
    ensure_usize(
        invalid_prototype_drops.get(),
        1,
        "invalid-prototype payload drops",
    )?;
    ensure_host_storage(&first, 1, 1, 4)?;

    let Err(error) = first.host_payload::<DifferentPayload>(&host) else {
        return Err("mismatched host payload type was accepted".into());
    };
    ensure_runtime_error(&error, "payload type does not match")?;
    let Err(error) = first.host_payload::<SharedState>(&plain) else {
        return Err("plain object was accepted as a typed host object".into());
    };
    ensure_runtime_error(&error, "value is not a typed host object")?;
    let Err(error) = first.host_payload::<SharedState>(&primitive) else {
        return Err("primitive was accepted as a typed host object".into());
    };
    ensure_runtime_error(&error, "value is not a typed host object")?;
    let Err(error) = second.clone_host_object(&host) else {
        return Err("foreign host object was cloned by another VM".into());
    };
    ensure_runtime_error(&error, "retained value belongs to another VM")?;

    host.release()?;
    plain.release()?;
    primitive.release()?;
    foreign_prototype.release()?;
    first.collect_garbage()?;
    ensure_host_storage(&first, 0, 0, 0)
}

#[test]
fn host_limits_and_root_failures_roll_back_transactionally() -> TestResult {
    let payload_drops = Rc::new(Cell::new(0));
    let payload_limits =
        VmStorageLimits::unlimited().with_max_payload_bytes(VmStorageKind::HostPayload, 3);
    let mut payload_limited = vm_with_storage_limits(payload_limits);
    let Err(error) = payload_limited.create_host_object(
        DropProbe {
            drops: Rc::clone(&payload_drops),
        },
        HostObjectOptions::new(4),
    ) else {
        return Err("host payload byte limit unexpectedly accepted an object".into());
    };
    ensure_limit_error(&error, "HostPayload")?;
    ensure_usize(payload_drops.get(), 1, "payload-limit rejection drops")?;
    ensure_host_storage(&payload_limited, 0, 0, 0)?;

    let root_drops = Rc::new(Cell::new(0));
    let root_limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::RetainedHandle, 0);
    let mut root_limited = vm_with_storage_limits(root_limits);
    let Err(error) = root_limited.create_host_object(
        DropProbe {
            drops: Rc::clone(&root_drops),
        },
        HostObjectOptions::new(2),
    ) else {
        return Err("retained-root limit unexpectedly accepted a host object".into());
    };
    ensure_limit_error(&error, "RetainedHandle")?;
    ensure_usize(root_drops.get(), 1, "retained-root rollback drops")?;
    ensure_host_storage(&root_limited, 0, 0, 0)?;

    let shared_drops = Rc::new(Cell::new(0));
    let shared_limits = VmStorageLimits::unlimited()
        .with_max_count(VmStorageKind::HostInstance, 1)
        .with_max_payload_bytes(VmStorageKind::HostPayload, 5);
    let mut shared_limited = vm_with_storage_limits(shared_limits);
    let host = shared_limited.create_host_object(
        DropProbe {
            drops: Rc::clone(&shared_drops),
        },
        HostObjectOptions::new(3),
    )?;
    let Err(error) = shared_limited.clone_host_object(&host) else {
        return Err("host instance limit unexpectedly accepted a shared wrapper".into());
    };
    ensure_limit_error(&error, "HostInstance")?;
    ensure_host_storage(&shared_limited, 1, 1, 3)?;

    shared_limited.update_host_payload_bytes(&host, 5)?;
    ensure_host_storage(&shared_limited, 1, 1, 5)?;
    let Err(error) = shared_limited.update_host_payload_bytes(&host, 6) else {
        return Err("host payload replacement exceeded its byte limit".into());
    };
    ensure_limit_error(&error, "HostPayload")?;
    ensure_host_storage(&shared_limited, 1, 1, 5)?;
    ensure_usize(shared_drops.get(), 0, "live shared payload drops")?;

    host.release()?;
    shared_limited.collect_garbage()?;
    ensure_usize(shared_drops.get(), 1, "collected shared payload drops")?;
    ensure_host_storage(&shared_limited, 0, 0, 0)
}

#[test]
fn teardown_reports_shared_payload_once_and_drops_it_exactly_once() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let drops = Rc::new(Cell::new(0));
    let host = vm.create_host_object(
        DropProbe {
            drops: Rc::clone(&drops),
        },
        HostObjectOptions::new(12),
    )?;
    let cloned = vm.clone_host_object(&host)?;

    let report = vm.finish()?;
    ensure_usize(
        report.storage.count(VmStorageKind::HostInstance),
        2,
        "teardown host wrapper count",
    )?;
    ensure_usize(
        report.storage.count(VmStorageKind::HostPayload),
        1,
        "teardown host payload count",
    )?;
    ensure_usize(
        report.storage.payload_bytes(VmStorageKind::HostPayload),
        12,
        "teardown host payload bytes",
    )?;
    ensure_usize(drops.get(), 1, "teardown payload drops")?;
    drop(host);
    drop(cloned);
    ensure_usize(drops.get(), 1, "post-teardown payload drops")
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

fn ensure_host_storage(
    vm: &Vm,
    instances: usize,
    payloads: usize,
    payload_bytes: usize,
) -> TestResult {
    let snapshot = vm.storage_snapshot()?;
    ensure_usize(
        snapshot.count(VmStorageKind::HostInstance),
        instances,
        "host instance storage",
    )?;
    ensure_usize(
        snapshot.count(VmStorageKind::HostPayload),
        payloads,
        "host payload storage",
    )?;
    ensure_usize(
        snapshot.payload_bytes(VmStorageKind::HostPayload),
        payload_bytes,
        "host payload byte storage",
    )
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
