use rs_quickjs::{
    Engine, EngineConfig, Error, RuntimeLimits, Value, Vm, VmConfig, VmStorageKind, VmStorageLimits,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn enforces_atom_and_heap_string_limits_before_interning() -> TestResult {
    let atom_limits = VmStorageLimits::unlimited()
        .with_max_count(VmStorageKind::Atom, 1)
        .with_max_payload_bytes(VmStorageKind::Atom, "camera".len());
    let mut vm = vm_with_storage_limits(atom_limits);
    vm.eval("let camera = 42; camera;")?;
    let before = vm.storage_snapshot()?;
    let error = expect_eval_error(&mut vm, "let lens = 24;")?;
    ensure_limit(&error, "Atom")?;
    ensure_snapshot(&vm, &before, "atom limit failure")?;

    let string_limits = VmStorageLimits::unlimited()
        .with_max_count(VmStorageKind::HeapString, 1)
        .with_max_payload_bytes(
            VmStorageKind::HeapString,
            "camera"
                .encode_utf16()
                .count()
                .saturating_mul(std::mem::size_of::<u16>())
                .saturating_add("camera".len()),
        );
    let mut vm = vm_with_storage_limits(string_limits);
    vm.eval(r#""camera";"#)?;
    let before = vm.storage_snapshot()?;
    let error = expect_eval_error(&mut vm, r#""lens";"#)?;
    ensure_limit(&error, "HeapString")?;
    ensure_snapshot(&vm, &before, "heap string limit failure")
}

#[test]
fn enforces_symbol_host_and_output_limits_transactionally() -> TestResult {
    let symbol_limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::Symbol, 0);
    let mut vm = vm_with_storage_limits(symbol_limits);
    let error = expect_eval_error(&mut vm, "Symbol('camera');")?;
    ensure_limit(&error, "Symbol")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::Symbol),
        0,
        "symbol count after rejection",
    )?;

    let host_limits = VmStorageLimits::unlimited()
        .with_max_count(VmStorageKind::HostCallback, 1)
        .with_max_payload_bytes(VmStorageKind::HostCallback, "cam".len());
    let mut vm = vm_with_storage_limits(host_limits);
    vm.register_host_function_typed("cam", |_call| Ok(42_f64))?;
    let before = vm.storage_snapshot()?;
    let Err(error) = vm.register_host_function_typed("lens", |_call| Ok(24_f64)) else {
        return Err("expected host callback limit to fail".into());
    };
    ensure_limit(&error, "HostCallback")?;
    ensure_snapshot(&vm, &before, "host callback limit failure")?;

    let output_limits = VmStorageLimits::unlimited()
        .with_max_count(VmStorageKind::OutputEntry, 1)
        .with_max_payload_bytes(VmStorageKind::OutputEntry, "cam".len());
    let mut vm = vm_with_storage_limits(output_limits);
    vm.eval("print('cam');")?;
    let before = vm.storage_snapshot()?;
    let error = expect_eval_error(&mut vm, "print('x');")?;
    ensure_limit(&error, "OutputEntry")?;
    let after = vm.storage_snapshot()?;
    ensure_kind_unchanged(
        &after,
        &before,
        VmStorageKind::OutputEntry,
        "output limit failure",
    )?;
    let output = vm.take_output();
    ensure_usize(output.len(), 1, "released output entries")?;
    vm.eval("print('new');")?;
    Ok(())
}

#[test]
fn enforces_object_regexp_and_buffer_limits_before_arena_growth() -> TestResult {
    let object_limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::Object, 0);
    let mut vm = vm_with_storage_limits(object_limits);
    let error = expect_eval_error(&mut vm, "({ lens: 42 });")?;
    ensure_limit(&error, "Object")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::Object),
        0,
        "object count after rejection",
    )?;

    let mut regexp_probe = Engine::new().create_vm();
    regexp_probe.eval("/cam/;")?;
    let regexp_payload_limit = regexp_probe
        .storage_snapshot()?
        .payload_bytes(VmStorageKind::Object);
    let regexp_limits = VmStorageLimits::unlimited()
        .with_max_payload_bytes(VmStorageKind::Object, regexp_payload_limit);
    let mut vm = vm_with_storage_limits(regexp_limits);
    vm.eval("/cam/;")?;
    let before = vm.storage_snapshot()?;
    let error = expect_eval_error(&mut vm, "/x/;")?;
    ensure_limit(&error, "Object")?;
    ensure_snapshot(&vm, &before, "RegExp payload limit failure")?;

    let buffer_limits = VmStorageLimits::unlimited()
        .with_max_count(VmStorageKind::ByteBuffer, 1)
        .with_max_payload_bytes(VmStorageKind::ByteBuffer, 4);
    let mut vm = vm_with_storage_limits(buffer_limits);
    vm.eval("new ArrayBuffer(4);")?;
    let before = vm.storage_snapshot()?;
    let error = expect_eval_error(&mut vm, "new ArrayBuffer(1);")?;
    ensure_limit(&error, "ByteBuffer")?;
    ensure_snapshot(&vm, &before, "byte buffer limit failure")
}

#[test]
fn accounts_regexp_compile_replacement_transactionally() -> TestResult {
    const LARGE_PATTERN: &str = "(?:camera|lens|sensor|body|frame|focus|aperture|shutter){4,12}";

    let mut small_probe = Engine::new().create_vm();
    small_probe.eval("/x/;")?;
    let small_payload = small_probe
        .storage_snapshot()?
        .payload_bytes(VmStorageKind::Object);

    let mut large_probe = Engine::new().create_vm();
    large_probe.eval(&format!("/{LARGE_PATTERN}/;"))?;
    let large_payload = large_probe
        .storage_snapshot()?
        .payload_bytes(VmStorageKind::Object);
    if large_payload <= small_payload {
        return Err("expected the large compiled RegExp to retain more payload".into());
    }

    let limits =
        VmStorageLimits::unlimited().with_max_payload_bytes(VmStorageKind::Object, small_payload);
    let mut constrained = vm_with_storage_limits(limits);
    constrained.eval("var pattern = /x/;")?;
    let before = constrained.storage_snapshot()?;
    let error = expect_eval_error(
        &mut constrained,
        &format!("pattern.compile('{LARGE_PATTERN}');"),
    )?;
    ensure_limit(&error, "Object")?;
    let after = constrained.storage_snapshot()?;
    ensure_kind_unchanged(
        &after,
        &before,
        VmStorageKind::Object,
        "RegExp replacement limit failure",
    )?;
    let preserved = constrained.eval("pattern.test('x') && !pattern.test('camera')")?;
    if preserved != Value::Bool(true) {
        return Err(
            format!("expected rejected replacement to preserve /x/, got {preserved:?}").into(),
        );
    }

    let limits =
        VmStorageLimits::unlimited().with_max_payload_bytes(VmStorageKind::Object, large_payload);
    let mut shrinking = vm_with_storage_limits(limits);
    shrinking.eval(&format!("var pattern = /{LARGE_PATTERN}/;"))?;
    let before = shrinking
        .storage_snapshot()?
        .payload_bytes(VmStorageKind::Object);
    shrinking.eval("pattern.compile('x');")?;
    let after = shrinking
        .storage_snapshot()?
        .payload_bytes(VmStorageKind::Object);
    if after >= before {
        return Err(format!(
            "expected RegExp replacement to release object payload: {before} -> {after}"
        )
        .into());
    }
    Ok(())
}

#[test]
fn enforces_retained_source_limits_and_keeps_vm_policies_isolated() -> TestResult {
    const GENERATED_SOURCE: &str = "function anonymous() {\nreturn 1;\n}";
    let source_limits = VmStorageLimits::unlimited()
        .with_max_count(VmStorageKind::SourceRecord, 1)
        .with_max_payload_bytes(VmStorageKind::SourceRecord, GENERATED_SOURCE.len());
    let mut constrained = vm_with_storage_limits(source_limits);
    constrained.eval("new Function('return 1;');")?;
    let before = constrained.storage_snapshot()?;
    let error = expect_eval_error(&mut constrained, "new Function('return 2;');")?;
    ensure_limit(&error, "SourceRecord")?;
    ensure_usize(
        constrained
            .storage_snapshot()?
            .count(VmStorageKind::SourceRecord),
        before.count(VmStorageKind::SourceRecord),
        "source records after rejection",
    )?;
    ensure_usize(
        constrained
            .storage_snapshot()?
            .payload_bytes(VmStorageKind::SourceRecord),
        before.payload_bytes(VmStorageKind::SourceRecord),
        "source bytes after rejection",
    )?;

    let engine = Engine::new();
    let mut independent = engine.create_vm();
    independent.eval("var camera = {}; new ArrayBuffer(8); /camera/g;")?;
    Ok(())
}

#[test]
fn enforces_binding_limits_and_releases_finished_scopes() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::Binding, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(&mut vm, "let camera = 1;")?;
    ensure_limit(&error, "Binding")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::Binding),
        0,
        "binding count after rejection",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::Binding, 1);
    let mut vm = vm_with_storage_limits(limits);
    vm.eval("{ let camera = 1; } { let lens = 2; }")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::Binding),
        0,
        "binding count after lexical scope release",
    )
}

#[test]
fn enforces_javascript_native_and_bound_function_limits() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::JavaScriptFunction, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(&mut vm, "function camera() {}")?;
    ensure_limit(&error, "JavaScriptFunction")?;
    ensure_usize(
        vm.storage_snapshot()?
            .count(VmStorageKind::JavaScriptFunction),
        0,
        "JavaScript function count after rejection",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::NativeFunction, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(&mut vm, "Math.abs(1);")?;
    ensure_limit(&error, "NativeFunction")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::NativeFunction),
        0,
        "native function count after rejection",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::BoundFunction, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(&mut vm, "(() => 1).bind(null);")?;
    ensure_limit(&error, "BoundFunction")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::BoundFunction),
        0,
        "bound function count after rejection",
    )
}

#[test]
fn enforces_object_property_limits_and_reuses_released_capacity() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::ObjectProperty, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(&mut vm, "({ camera: 1 });")?;
    ensure_limit(&error, "ObjectProperty")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::ObjectProperty),
        0,
        "object property count after rejection",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::ObjectProperty, 2);
    let mut vm = vm_with_storage_limits(limits);
    vm.eval("{ let camera = { lens: 1 }; delete camera.lens; camera.body = 2; }")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::ObjectProperty),
        2,
        "object property count after delete and reuse",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::ObjectProperty, 4);
    let mut vm = vm_with_storage_limits(limits);
    vm.eval("{ let camera = () => 1; delete camera.name; camera.lens = 2; }")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::ObjectProperty),
        4,
        "function property count after delete and reuse",
    )
}

#[test]
fn enforces_cache_entry_limits_before_cache_materialization() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::CacheEntry, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(&mut vm, "var camera = 1;")?;
    ensure_limit(&error, "CacheEntry")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::CacheEntry),
        0,
        "cache entry count after rejection",
    )
}

#[test]
fn enforces_collection_owner_limits_and_reuses_deleted_entries() -> TestResult {
    for (kind, source) in [
        (VmStorageKind::Collection, "new Map();"),
        (
            VmStorageKind::CollectionEntry,
            "var camera = new Map(); camera.set('lens', 1);",
        ),
        (
            VmStorageKind::CollectionIterator,
            "new Map([['camera', 1]]).entries();",
        ),
        (
            VmStorageKind::IteratorItem,
            "new Map([['camera', 1]]).entries();",
        ),
    ] {
        let limits = VmStorageLimits::unlimited().with_max_count(kind, 0);
        let mut vm = vm_with_storage_limits(limits);
        let error = expect_eval_error(&mut vm, source)?;
        ensure_limit(&error, storage_kind_name(kind))?;
        ensure_usize(
            vm.storage_snapshot()?.count(kind),
            0,
            "collection owner count after rejection",
        )?;
    }

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::CollectionEntry, 1);
    let mut vm = vm_with_storage_limits(limits);
    vm.eval(
        "var camera = new Map(); camera.set('lens', 1); camera.delete('lens'); camera.set('body', 2);",
    )?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::CollectionEntry),
        1,
        "collection entries after delete and reuse",
    )
}

#[test]
fn enforces_promise_reaction_and_job_limits() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::Promise, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(&mut vm, "Promise.resolve(1);")?;
    ensure_limit(&error, "Promise")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::Promise),
        0,
        "Promise count after rejection",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::PromiseReaction, 0);
    let mut vm = vm_with_storage_limits(limits);
    vm.eval(
        "var cameraResolve; var camera = new Promise(function(resolve) { cameraResolve = resolve; });",
    )?;
    let error = expect_eval_error(&mut vm, "camera.then(function(value) { return value; });")?;
    ensure_limit(&error, "PromiseReaction")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::PromiseReaction),
        0,
        "Promise reaction count after rejection",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::PromiseJob, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(
        &mut vm,
        "Promise.resolve(1).then(function(value) { return value; });",
    )?;
    ensure_limit(&error, "PromiseJob")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::PromiseJob),
        0,
        "Promise job count after rejection",
    )
}

#[test]
fn releases_settled_reactions_and_drained_jobs() -> TestResult {
    let mut vm = vm_with_storage_limits(VmStorageLimits::unlimited());
    vm.eval(
        "var cameraResolve; var camera = new Promise(function(resolve) { cameraResolve = resolve; }); camera.then(function(value) { return value; });",
    )?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::PromiseReaction),
        1,
        "pending Promise reactions",
    )?;
    vm.eval("cameraResolve(42);")?;
    let snapshot = vm.storage_snapshot()?;
    ensure_usize(
        snapshot.count(VmStorageKind::PromiseReaction),
        0,
        "settled Promise reactions",
    )?;
    ensure_usize(
        snapshot.count(VmStorageKind::PromiseJob),
        0,
        "drained Promise jobs",
    )
}

#[test]
fn enforces_and_releases_retained_transient_and_execution_owners() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::RetainedHandle, 1);
    let mut vm = vm_with_storage_limits(limits);
    let retained = vm.eval_retained("({ camera: 1 });")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::RetainedHandle),
        1,
        "active retained handle",
    )?;
    let Err(error) = vm.eval_retained("({ lens: 2 });") else {
        return Err("expected retained handle limit to fail".into());
    };
    ensure_limit(&error, "RetainedHandle")?;
    drop(retained);
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::RetainedHandle),
        0,
        "dropped retained handle",
    )?;
    let replacement = vm.eval_retained("({ body: 3 });")?;
    replacement.release()?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::TransientRoot, 0);
    let mut vm = vm_with_storage_limits(limits);
    vm.register_host_function_typed("captureCamera", |_call| Ok(1_f64))?;
    let error = expect_eval_error(&mut vm, "captureCamera({ lens: 1 });")?;
    ensure_limit(&error, "TransientRoot")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::TransientRoot),
        0,
        "transient roots after rejection",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::ExecutionFrame, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(
        &mut vm,
        "function camera(value) { let lens = value + 1; return lens; } camera(1);",
    )?;
    ensure_limit(&error, "ExecutionFrame")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::ExecutionFrame),
        0,
        "execution frames after rejection",
    )
}

#[test]
fn enforces_association_limits_and_keeps_module_storage_empty() -> TestResult {
    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::Association, 0);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(&mut vm, "({ camera: 1 });")?;
    ensure_limit(&error, "Association")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::Association),
        0,
        "association count after rejection",
    )?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::Module),
        0,
        "unsupported module storage",
    )
}

#[test]
fn charges_activation_scope_and_bytecode_frames_per_nested_call() -> TestResult {
    const SOURCE: &str = r"
        function inner(value) {
            let lens = value + 1;
            return lens;
        }
        function outer(value) {
            let body = value + 1;
            return inner(body);
        }
        outer(40);
    ";

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::ExecutionFrame, 5);
    let mut vm = vm_with_storage_limits(limits);
    vm.eval(SOURCE)?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::ExecutionFrame),
        0,
        "released nested activation frames",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::ExecutionFrame, 4);
    let mut vm = vm_with_storage_limits(limits);
    let error = expect_eval_error(&mut vm, SOURCE)?;
    ensure_limit(&error, "ExecutionFrame")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::ExecutionFrame),
        0,
        "nested activation frames after rejection",
    )
}

#[test]
fn charges_and_unwinds_structured_control_frames() -> TestResult {
    const LOOP_SOURCE: &str = r"
        var index = 0;
        while (keepRunning(index)) {
            index = index + 1;
        }
        index;
    ";
    const NESTED_SOURCE: &str = r"
        var index = 0;
        while (keepRunning(index)) {
            try {
                index = index + 1;
            } finally {
                index = index;
            }
        }
    ";

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::ExecutionFrame, 2);
    let mut vm = vm_with_storage_limits(limits);
    vm.register_host_function_typed("keepRunning", |call| Ok(call.number(0, "index")? < 1.0))?;
    vm.eval(LOOP_SOURCE)?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::ExecutionFrame),
        0,
        "released structured loop frames",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::ExecutionFrame, 1);
    let mut vm = vm_with_storage_limits(limits);
    vm.register_host_function_typed("keepRunning", |call| Ok(call.number(0, "index")? < 1.0))?;
    let error = expect_eval_error(&mut vm, LOOP_SOURCE)?;
    ensure_limit(&error, "ExecutionFrame")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::ExecutionFrame),
        0,
        "structured loop frames after rejection",
    )?;

    let limits = VmStorageLimits::unlimited().with_max_count(VmStorageKind::ExecutionFrame, 2);
    let mut vm = vm_with_storage_limits(limits);
    vm.register_host_function_typed("keepRunning", |call| Ok(call.number(0, "index")? < 1.0))?;
    let error = expect_eval_error(&mut vm, NESTED_SOURCE)?;
    ensure_limit(&error, "ExecutionFrame")?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::ExecutionFrame),
        0,
        "nested structured frames after rejection",
    )
}

const fn storage_kind_name(kind: VmStorageKind) -> &'static str {
    match kind {
        VmStorageKind::Collection => "Collection",
        VmStorageKind::CollectionEntry => "CollectionEntry",
        VmStorageKind::CollectionIterator => "CollectionIterator",
        VmStorageKind::IteratorItem => "IteratorItem",
        _ => "storage",
    }
}

fn vm_with_storage_limits(storage: VmStorageLimits) -> Vm {
    let limits = RuntimeLimits {
        storage,
        ..RuntimeLimits::default()
    };
    Engine::with_config(EngineConfig::with_default_vm_config(VmConfig::with_limits(
        limits,
    )))
    .create_vm()
}

fn expect_eval_error(vm: &mut Vm, source: &str) -> Result<Error, Box<dyn std::error::Error>> {
    let Err(error) = vm.eval(source) else {
        return Err(format!("expected storage limit to reject {source:?}").into());
    };
    Ok(error)
}

fn ensure_limit(error: &Error, category: &str) -> TestResult {
    if matches!(error, Error::ResourceLimit { .. }) && error.to_string().contains(category) {
        return Ok(());
    }
    Err(format!("expected {category} resource limit, got {error:?}").into())
}

fn ensure_snapshot(vm: &Vm, expected: &rs_quickjs::VmStorageSnapshot, label: &str) -> TestResult {
    let actual = vm.storage_snapshot()?;
    if &actual == expected {
        return Ok(());
    }
    Err(format!("expected {label} {expected:?}, got {actual:?}").into())
}

fn ensure_kind_unchanged(
    actual: &rs_quickjs::VmStorageSnapshot,
    expected: &rs_quickjs::VmStorageSnapshot,
    kind: VmStorageKind,
    label: &str,
) -> TestResult {
    if actual.count(kind) == expected.count(kind)
        && actual.payload_bytes(kind) == expected.payload_bytes(kind)
    {
        return Ok(());
    }
    Err(format!("expected unchanged {label} {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {label} {expected}, got {actual}").into())
}
