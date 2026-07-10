use rs_quickjs::{Engine, Error, VmStorageKind, VmStorageSnapshot};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn fresh_vm_has_a_complete_empty_storage_map() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();
    let snapshot = vm.storage_snapshot()?;

    ensure_usize(VmStorageKind::all().len(), 26, "storage kind count")?;
    ensure(snapshot.is_empty(), "fresh VM storage should be empty")?;
    ensure_usize(snapshot.total(), 0, "fresh storage total")?;
    ensure_snapshot_sum(snapshot)
}

#[test]
fn counts_every_materialized_vm_owner_category() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.register_host_function_typed("hostCamera", |_call| Ok("camera"))?;
    vm.eval(
        r#"
        var cameraObject = { lens: 42 };
        var cameraFunction = function camera(value) { return value; };
        var generatedCamera = new Function("return 42;");
        var boundCamera = cameraFunction.bind(cameraObject, 1);
        var cameraSymbol = Symbol("camera");
        var cameraBuffer = new ArrayBuffer(8);
        var cameraMap = new Map([[cameraObject, 42]]);
        var cameraSet = new Set([cameraObject]);
        var cameraIterator = cameraMap.keys();
        var pendingCamera = new Promise(function cameraExecutor() {});
        pendingCamera.then(function cameraReaction() { return 42; });
        print(hostCamera());
        "#,
    )?;
    let retained = vm
        .get_global_retained("cameraObject")?
        .ok_or("cameraObject global was not retained")?;

    let snapshot = vm.storage_snapshot()?;
    for (kind, label) in [
        (VmStorageKind::Atom, "atoms"),
        (VmStorageKind::HeapString, "heap strings"),
        (VmStorageKind::Symbol, "symbols"),
        (VmStorageKind::Binding, "bindings"),
        (VmStorageKind::JavaScriptFunction, "JavaScript functions"),
        (VmStorageKind::NativeFunction, "native functions"),
        (VmStorageKind::BoundFunction, "bound functions"),
        (VmStorageKind::HostCallback, "host callbacks"),
        (VmStorageKind::Object, "objects"),
        (VmStorageKind::ObjectProperty, "object properties"),
        (VmStorageKind::ByteBuffer, "byte buffers"),
        (VmStorageKind::Collection, "collections"),
        (VmStorageKind::CollectionEntry, "collection entries"),
        (VmStorageKind::CollectionIterator, "collection iterators"),
        (VmStorageKind::IteratorItem, "iterator items"),
        (VmStorageKind::Promise, "promises"),
        (VmStorageKind::PromiseReaction, "Promise reactions"),
        (VmStorageKind::RetainedHandle, "retained handles"),
        (VmStorageKind::OutputEntry, "output entries"),
        (VmStorageKind::CacheEntry, "cache entries"),
        (VmStorageKind::Association, "associations"),
        (VmStorageKind::SourceRecord, "source records"),
    ] {
        ensure_positive(snapshot.count(kind), label)?;
    }
    ensure_usize(
        snapshot.count(VmStorageKind::TransientRoot),
        0,
        "settled transient roots",
    )?;
    ensure_usize(
        snapshot.count(VmStorageKind::ExecutionFrame),
        0,
        "settled execution frames",
    )?;
    ensure_usize(snapshot.count(VmStorageKind::Module), 0, "module records")?;
    ensure_usize(
        snapshot.count(VmStorageKind::PromiseJob),
        0,
        "settled Promise jobs",
    )?;
    ensure_snapshot_sum(snapshot)?;

    retained.release()?;
    ensure_usize(
        vm.storage_snapshot()?.count(VmStorageKind::RetainedHandle),
        0,
        "released retained handles",
    )
}

#[test]
fn finish_reconciles_the_exact_pre_drop_owner_snapshot() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval("var camera = { lens: 42 };")?;
    let retained = vm
        .get_global_retained("camera")?
        .ok_or("camera global was not retained")?;

    let before = vm.storage_snapshot()?;
    let preview = vm.teardown_report()?;
    ensure_snapshot(before, preview.storage, "teardown preview")?;
    let finished = vm.finish()?;
    ensure_snapshot(before, finished.storage, "finished storage")?;
    ensure_snapshot_sum(finished.storage)?;

    let Err(error) = retained.release() else {
        return Err("retained handle survived VM teardown".into());
    };
    ensure_runtime_error(&error, "retained value owner has been torn down")
}

fn ensure_snapshot_sum(snapshot: VmStorageSnapshot) -> TestResult {
    let total = VmStorageKind::all().iter().try_fold(0_usize, |sum, kind| {
        sum.checked_add(snapshot.count(*kind))
            .ok_or_else(|| Box::<dyn std::error::Error>::from("storage category sum overflowed"))
    })?;
    ensure_usize(total, snapshot.total(), "storage category sum")
}

fn ensure_snapshot(
    actual: VmStorageSnapshot,
    expected: VmStorageSnapshot,
    label: &str,
) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {label} {expected:?}, got {actual:?}").into())
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        return Ok(());
    }
    Err(message.into())
}

fn ensure_positive(actual: usize, label: &str) -> TestResult {
    if actual > 0 {
        return Ok(());
    }
    Err(format!("expected positive {label}, got {actual}").into())
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
