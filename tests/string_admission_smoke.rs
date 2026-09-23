use std::fmt::Write;

use velum::{
    Engine, EngineConfig, Error, JsString, OptimizationMode, RuntimeLimits, Value, Vm, VmConfig,
    VmGcKind, VmStorageKind, VmStorageLimits, VmStorageSnapshot,
};

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];
const MAX_LITERAL_CACHE_LIMIT: usize = 16;

fn vm_with_limits(storage: VmStorageLimits, mode: OptimizationMode) -> Vm {
    Engine::with_config(EngineConfig::with_default_vm_config(
        VmConfig::with_limits(RuntimeLimits {
            storage,
            ..RuntimeLimits::default()
        })
        .with_optimization_mode(mode),
    ))
    .create_vm()
}

fn ensure_metadata(units: &[u16]) -> TestResult {
    let text = JsString::from_utf16(units.to_vec());
    let exact = String::from_utf16(units).ok();
    let rendered = String::from_utf16_lossy(units);
    if text.as_utf16() != units
        || text.is_well_formed() != exact.is_some()
        || text.as_utf8() != exact.as_deref()
        || text.as_str() != rendered
        || text.identity().is_some()
    {
        return Err(format!("portable UTF-16 metadata mismatch for {units:?}").into());
    }
    Ok(())
}

fn string(value: Value) -> TestResult<JsString> {
    if let Value::String(text) = value {
        return Ok(text);
    }
    Err(format!("expected a string, received {value:?}").into())
}

fn literal(units: &[u16]) -> TestResult<String> {
    let mut source = String::from("'");
    for unit in units {
        write!(source, "\\u{unit:04x}")?;
    }
    source.push('\'');
    Ok(source)
}

fn payload_bytes(units: &[u16]) -> TestResult<usize> {
    units
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .and_then(|bytes| bytes.checked_add(String::from_utf16_lossy(units).len()))
        .ok_or_else(|| "fixture payload length overflowed".into())
}

fn ensure_admission_unchanged(
    actual: &VmStorageSnapshot,
    before: &VmStorageSnapshot,
) -> TestResult {
    for kind in [VmStorageKind::HeapString, VmStorageKind::CacheEntry] {
        if actual.count(kind) != before.count(kind)
            || actual.payload_bytes(kind) != before.payload_bytes(kind)
        {
            return Err(format!("failed or repeated admission changed {kind:?} accounting").into());
        }
    }
    Ok(())
}

fn ensure_limit(error: &Error, category: &str) -> TestResult {
    if matches!(error, Error::ResourceLimit { .. }) && error.to_string().contains(category) {
        return Ok(());
    }
    Err(format!("expected {category} resource limit, received {error}").into())
}

fn minimum_literal_cache_limit(mode: OptimizationMode) -> TestResult<usize> {
    // Calibrate the fixture's transient script caches as well as its retained
    // string entry, without depending on the layout of compiled cache tables.
    for maximum in 0..=MAX_LITERAL_CACHE_LIMIT {
        let limits =
            VmStorageLimits::unlimited().with_max_count(VmStorageKind::CacheEntry, maximum);
        let mut probe = vm_with_limits(limits, mode);
        match probe.eval("'resident'") {
            Ok(_) => return Ok(maximum),
            Err(error) => ensure_limit(&error, "CacheEntry")?,
        }
    }
    Err("literal fixture did not fit its bounded cache calibration".into())
}

#[test]
fn ascii_metadata_handles_nul_del_and_vector_boundaries() -> TestResult {
    let alphabet = (0_u16..=0x7F).collect::<Vec<_>>();
    for length in [0, 1, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 1025, 65_536] {
        let units = alphabet
            .iter()
            .copied()
            .cycle()
            .take(length)
            .collect::<Vec<_>>();
        ensure_metadata(&units)?;
    }
    Ok(())
}

#[test]
fn every_single_code_unit_keeps_exact_rendering_metadata() -> TestResult {
    for unit in 0..=u16::MAX {
        ensure_metadata(&[unit])?;
    }
    Ok(())
}

#[test]
fn non_ascii_at_each_boundary_uses_lossless_utf16_fallback() -> TestResult {
    for position in [0, 1, 7, 8, 15, 16, 31, 32, 63, 64, 128] {
        for unit in [0x80, 0x7FF, 0x800, 0xD800, 0xDC00, 0xFFFF] {
            let mut units = vec![0x41; 129];
            let slot = units
                .get_mut(position)
                .ok_or("fixture position is missing")?;
            *slot = unit;
            ensure_metadata(&units)?;
        }
        let mut pair = vec![0x41; position];
        pair.extend([0xD83D, 0xDE00, 0xD800, 0x41, 0xDC00]);
        ensure_metadata(&pair)?;
    }
    Ok(())
}

#[test]
fn ascii_and_unicode_admission_charge_exact_reserved_rendering_bytes() -> TestResult {
    for units in [
        vec![],
        vec![0, 0x41, 0x7F],
        vec![0x41; 4097],
        vec![0x41, 0xE9, 0x3BB],
        vec![0xD83D, 0xDE00, 0xD800, 0x41, 0xDC00],
    ] {
        for mode in MODES {
            let mut vm = vm_with_limits(VmStorageLimits::unlimited(), mode);
            let script = vm.compile(&literal(&units)?)?;
            let first = string(vm.eval_compiled(&script)?)?;
            let before = vm.storage_snapshot()?;
            if first.as_utf16() != units
                || before.count(VmStorageKind::HeapString) != 1
                || before.payload_bytes(VmStorageKind::HeapString) != payload_bytes(&units)?
            {
                return Err("admission changed exact UTF-16 or reserved byte totals".into());
            }
            let second = string(vm.eval_compiled(&script)?)?;
            if first.id() != second.id() || first.identity() != second.identity() {
                return Err("duplicate admission changed string identity".into());
            }
            ensure_admission_unchanged(&vm.storage_snapshot()?, &before)?;
            if first.as_str() != String::from_utf16_lossy(&units) {
                return Err("lazy rendering differs from the UTF-16 input".into());
            }
            ensure_admission_unchanged(&vm.storage_snapshot()?, &before)?;
        }
    }
    Ok(())
}

#[test]
fn literal_portable_and_builtin_utf16_admission_share_identity() -> TestResult {
    for mode in MODES {
        let mut vm = vm_with_limits(VmStorageLimits::unlimited(), mode);
        let portable = JsString::from_utf16(vec![0x41, 0xD800, 0xDC00, 0xDC00]);
        vm.register_host_function("portable", move |_call| Ok(Value::String(portable.clone())))?;
        let literal = string(vm.eval(r"'A\uD800\uDC00\uDC00'")?)?;
        let host = string(vm.eval("portable()")?)?;
        let builtin = string(vm.eval("String.fromCharCode(65, 55296, 56320, 56320)")?)?;
        for text in [host, builtin] {
            if text.id() != literal.id()
                || text.identity() != literal.identity()
                || text.as_utf16() != literal.as_utf16()
            {
                return Err("UTF-16 admission surfaces failed to share an interned entry".into());
            }
        }
    }
    Ok(())
}

#[test]
fn utf8_and_utf16_admission_share_identity() -> TestResult {
    for mode in MODES {
        let mut vm = vm_with_limits(VmStorageLimits::unlimited(), mode);
        let runtime = string(vm.eval("typeof absent")?)?;
        let literal = string(vm.eval("'undefined'")?)?;
        if runtime.id() != literal.id() || runtime.identity() != literal.identity() {
            return Err(
                "UTF-8 runtime and UTF-16 literal admission produced distinct entries".into(),
            );
        }
    }
    Ok(())
}

#[test]
fn exhausted_cache_accepts_hits_and_rejects_misses_without_mutating_strings() -> TestResult {
    for mode in MODES {
        let maximum = minimum_literal_cache_limit(mode)?;
        let limits =
            VmStorageLimits::unlimited().with_max_count(VmStorageKind::CacheEntry, maximum);
        let mut vm = vm_with_limits(limits, mode);
        vm.eval("'resident'")?;
        let before = vm.storage_snapshot()?;
        for _ in 0..3 {
            vm.eval("'resident'")?;
            ensure_admission_unchanged(&vm.storage_snapshot()?, &before)?;
            let Err(error) = vm.eval("'new-string'") else {
                return Err("new string bypassed the exhausted cache-entry limit".into());
            };
            ensure_limit(&error, "CacheEntry")?;
            ensure_admission_unchanged(&vm.storage_snapshot()?, &before)?;
        }
    }
    Ok(())
}

#[test]
fn record_and_payload_rejections_leave_no_reserved_index_entry() -> TestResult {
    for limits in [
        VmStorageLimits::unlimited().with_max_count(VmStorageKind::HeapString, 1),
        VmStorageLimits::unlimited().with_max_payload_bytes(VmStorageKind::HeapString, 3),
    ] {
        for mode in MODES {
            let mut vm = vm_with_limits(limits.clone(), mode);
            vm.eval("'A'")?;
            let before = vm.storage_snapshot()?;
            for source in ["'B'", r"'\uD800'", "'B'", "'A'"] {
                if source == "'A'" {
                    vm.eval(source)?;
                } else {
                    let Err(error) = vm.eval(source) else {
                        return Err("distinct string exceeded its storage limit".into());
                    };
                    ensure_limit(&error, "HeapString")?;
                }
                ensure_admission_unchanged(&vm.storage_snapshot()?, &before)?;
            }
        }
    }
    Ok(())
}

#[test]
fn failed_free_slot_admission_can_recover_and_deduplicate_after_collection() -> TestResult {
    for mode in MODES {
        let limits =
            VmStorageLimits::unlimited().with_max_payload_bytes(VmStorageKind::HeapString, 3);
        let mut vm = vm_with_limits(limits, mode);
        for unit in 0x41..=0x5A {
            let before = vm.storage_snapshot()?;
            let Err(error) = vm.eval("'too-long'") else {
                return Err("oversized string unexpectedly occupied a free slot".into());
            };
            ensure_limit(&error, "HeapString")?;
            ensure_admission_unchanged(&vm.storage_snapshot()?, &before)?;
            let source = literal(&[unit])?;
            let first = string(vm.eval(&source)?)?;
            let second = string(vm.eval(&source)?)?;
            if first.id() != second.id() || first.as_utf16() != [unit] {
                return Err("free-slot reuse broke exact identity or value".into());
            }
            let report = vm.collect_garbage()?;
            if report.reclaimed(VmGcKind::HeapString) != 1 {
                return Err("unrooted interned string was not reclaimed exactly once".into());
            }
            if vm.storage_snapshot()?.count(VmStorageKind::HeapString) != 0 {
                return Err("collection retained an unrooted string record".into());
            }
        }
    }
    Ok(())
}

#[test]
fn shared_portable_payload_keeps_independent_vm_owners() -> TestResult {
    let portable = JsString::from_utf16(vec![0x41; 8193]);
    let mut identities = Vec::new();
    for mode in MODES {
        let mut vm = vm_with_limits(VmStorageLimits::unlimited(), mode);
        let returned = portable.clone();
        vm.register_host_function("text", move |_call| Ok(Value::String(returned.clone())))?;
        let admitted = string(vm.eval("text()")?)?;
        if admitted.as_utf16() != portable.as_utf16() || admitted.identity().is_none() {
            return Err("portable admission lost its payload or VM owner".into());
        }
        identities.push(admitted.identity().cloned());
    }
    if portable.identity().is_some() || identities.first() == identities.last() {
        return Err("shared portable payload leaked one VM's identity to another".into());
    }
    Ok(())
}

#[test]
fn repeated_concatenation_preserves_ascii_and_late_surrogates() -> TestResult {
    for mode in MODES {
        let mut vm = vm_with_limits(VmStorageLimits::unlimited(), mode);
        let actual = string(vm.eval(
            r"var text = ''; for (var i = 0; i < 2048; i++) text += 'A'; text += '\uD800'; text += '\uDC00'; text += '\uD800'; text;",
        )?)?;
        let mut expected = vec![0x41; 2048];
        expected.extend([0xD800, 0xDC00, 0xD800]);
        if actual.as_utf16() != expected
            || actual.is_well_formed()
            || actual.as_str() != String::from_utf16_lossy(&expected)
        {
            return Err("repeated concatenation corrupted the ASCII/UTF-16 boundary".into());
        }
        vm.collect_garbage()?;
        vm.storage_snapshot()?;
    }
    Ok(())
}
