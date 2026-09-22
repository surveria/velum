use std::rc::Rc;

use parking_lot::Mutex;
use velum::{
    Engine, EngineConfig, Error, JsString, RuntimeLimits, Value, VmConfig, VmGcKind, VmStorageKind,
    VmStorageLimits, VmStorageSnapshot,
};

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

struct StringCase {
    source: &'static str,
    units: &'static [u16],
    rendered: &'static str,
    well_formed: bool,
}

const STRING_CASES: [StringCase; 4] = [
    StringCase {
        source: r"var text = 'A'; text += 'B'; text += 'C'; text;",
        units: &[0x0041, 0x0042, 0x0043],
        rendered: "ABC",
        well_formed: true,
    },
    StringCase {
        source: r"var text = '\u00E9'; text += '\u03BB'; text;",
        units: &[0x00E9, 0x03BB],
        rendered: "éλ",
        well_formed: true,
    },
    StringCase {
        source: r"var text = '\uD83D'; text += '\uDE00'; text;",
        units: &[0xD83D, 0xDE00],
        rendered: "😀",
        well_formed: true,
    },
    StringCase {
        source: r"var text = '\uD83D'; text += '\uDE00'; text += '\uD800'; text += 'A'; text;",
        units: &[0xD83D, 0xDE00, 0xD800, 0x0041],
        rendered: "😀\u{FFFD}A",
        well_formed: false,
    },
];

#[test]
fn assignment_and_collection_preserve_exact_utf16_before_rendering() -> TestResult {
    let engine = Engine::new();
    for case in STRING_CASES {
        let mut vm = engine.create_vm();
        let Value::String(text) = vm.eval(case.source)? else {
            return Err(format!("expected a string for {}", case.source).into());
        };
        if text.as_utf16() != case.units || text.is_well_formed() != case.well_formed {
            return Err(format!("UTF-16 metadata differs for {}", case.source).into());
        }
        vm.collect_garbage()?;
        let before = vm.storage_snapshot()?;
        if text.as_str() != case.rendered {
            return Err(format!("unexpected rendering for {}", case.source).into());
        }
        let expected_utf8 = case.well_formed.then_some(case.rendered);
        if text.as_utf8() != expected_utf8 || text.as_utf16() != case.units {
            return Err(format!("rendering changed exact UTF-16 for {}", case.source).into());
        }
        let after = vm.storage_snapshot()?;
        if before != after {
            return Err(format!("rendering changed storage accounting for {}", case.source).into());
        }
    }
    Ok(())
}

#[test]
fn rejects_foreign_utf16_strings_before_resolving_colliding_slots() -> TestResult {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    let mut second = engine.create_vm();
    let Value::String(foreign) = first.eval(r"'\uD800'")? else {
        return Err("expected a foreign UTF-16 string".into());
    };
    let Value::String(local) = second.eval(r"'\uD801'")? else {
        return Err("expected a local UTF-16 string".into());
    };
    if foreign.id() != local.id() || foreign.identity() == local.identity() {
        return Err("foreign string fixture must have a colliding id and distinct owner".into());
    }
    second.register_host_function("foreignString", move |_call| {
        Ok(Value::String(foreign.clone()))
    })?;
    let before = second.storage_snapshot()?;
    let Err(error) = second.eval("foreignString()") else {
        return Err("foreign string validation unexpectedly succeeded".into());
    };
    ensure_runtime_error(&error, "value belongs to another VM")?;
    ensure_string_storage(&second.storage_snapshot()?, &before)
}

#[test]
fn rejects_a_collected_string_id_without_materializing_its_retained_payload() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let returned = Rc::new(Mutex::new(Value::Undefined));
    let captured = Rc::clone(&returned);
    vm.register_host_function("collectedString", move |_call| Ok(captured.lock().clone()))?;
    *returned.lock() = vm.eval(r"'unrooted\uD800'")?;
    let report = vm.collect_garbage()?;
    if report.reclaimed(VmGcKind::HeapString) == 0 {
        return Err("fixture did not reclaim the unretained string id".into());
    }
    let before = vm.storage_snapshot()?;
    let Err(error) = vm.eval("collectedString()") else {
        return Err("collected string validation unexpectedly succeeded".into());
    };
    ensure_runtime_error(&error, "string id is not defined")?;
    ensure_string_storage(&vm.storage_snapshot()?, &before)
}

#[test]
fn lazy_utf16_admission_preserves_record_and_rendering_byte_limits() -> TestResult {
    for (limits, expected_error) in [
        (
            VmStorageLimits::unlimited().with_max_count(VmStorageKind::HeapString, 1),
            "HeapString record count exceeded 1",
        ),
        (
            VmStorageLimits::unlimited().with_max_payload_bytes(VmStorageKind::HeapString, 5),
            "HeapString payload bytes exceeded 5",
        ),
    ] {
        check_admission_limit(limits, expected_error)?;
    }
    Ok(())
}

#[test]
fn portable_string_validation_keeps_the_utf16_length_limit() -> TestResult {
    let limits = RuntimeLimits {
        max_string_len: 32,
        ..RuntimeLimits::default()
    };
    let engine = Engine::with_config(EngineConfig::with_default_vm_config(VmConfig::with_limits(
        limits,
    )));
    let mut vm = engine.create_vm();
    let text = JsString::from_utf16(vec![0xD800; 33]);
    vm.register_host_function("x", move |_call| Ok(Value::String(text.clone())))?;
    let before = vm.storage_snapshot()?;
    let Err(error) = vm.eval("x()") else {
        return Err("oversized portable string exceeded the limit without an error".into());
    };
    ensure_limit_error(&error, "string length 33 exceeded 32")?;
    ensure_string_storage(&vm.storage_snapshot()?, &before)
}

fn check_admission_limit(storage: VmStorageLimits, expected_error: &str) -> TestResult {
    let limits = RuntimeLimits {
        storage,
        ..RuntimeLimits::default()
    };
    let engine = Engine::with_config(EngineConfig::with_default_vm_config(VmConfig::with_limits(
        limits,
    )));
    let mut vm = engine.create_vm();
    let script = vm.compile(r"'\uD800'")?;
    let rejected_script = vm.compile(r"'\uD801'")?;
    let first = vm.eval_compiled(&script)?;
    let before = vm.storage_snapshot()?;
    if before.count(VmStorageKind::HeapString) != 1
        || before.payload_bytes(VmStorageKind::HeapString) != 5
    {
        return Err(
            "a lone surrogate must reserve two UTF-16 bytes and three rendering bytes".into(),
        );
    }
    let second = vm.eval_compiled(&script)?;
    if first != second {
        return Err("repeat admission changed the UTF-16 value".into());
    }
    ensure_string_storage(&vm.storage_snapshot()?, &before)?;
    let Err(error) = vm.eval_compiled(&rejected_script) else {
        return Err("distinct string admission unexpectedly exceeded its limit".into());
    };
    ensure_limit_error(&error, expected_error)?;
    let after = vm.storage_snapshot()?;
    ensure_string_storage(&after, &before)?;
    if after.count(VmStorageKind::CacheEntry) != before.count(VmStorageKind::CacheEntry) {
        return Err("failed string admission leaked a cache reservation".into());
    }
    Ok(())
}

fn ensure_string_storage(actual: &VmStorageSnapshot, expected: &VmStorageSnapshot) -> TestResult {
    if actual.count(VmStorageKind::HeapString) != expected.count(VmStorageKind::HeapString)
        || actual.payload_bytes(VmStorageKind::HeapString)
            != expected.payload_bytes(VmStorageKind::HeapString)
    {
        return Err("string validation changed heap count or reserved payload bytes".into());
    }
    Ok(())
}

fn ensure_runtime_error(error: &Error, expected: &str) -> TestResult {
    if matches!(error, Error::Runtime { .. }) && error.to_string().contains(expected) {
        return Ok(());
    }
    Err(format!("expected runtime error containing {expected:?}, got {error}").into())
}

fn ensure_limit_error(error: &Error, expected: &str) -> TestResult {
    if matches!(error, Error::ResourceLimit { .. }) && error.to_string().contains(expected) {
        return Ok(());
    }
    Err(format!("expected resource limit containing {expected:?}, got {error}").into())
}
