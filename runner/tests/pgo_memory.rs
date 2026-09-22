#[path = "support/pgo_memory_fixture.rs"]
mod fixture;
#[path = "../src/pgo/memory.rs"]
mod memory;

use anyhow::{Context as _, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Files {
    root: PathBuf,
    binary: PathBuf,
    report: PathBuf,
    base: Value,
}

impl Files {
    fn new() -> Result<Self> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root =
            std::env::temp_dir().join(format!("velum-pgo-memory-{}-{nonce}", std::process::id()));
        fs::create_dir(&root)?;
        let binary = root.join("ordinary");
        fs::write(&binary, "synthetic saved executable")?;
        let base = fixture::report(&binary, &json!({}), &json!({}))?;
        Ok(Self {
            report: root.join("memory.json"),
            root,
            binary,
            base,
        })
    }
    fn verify(&self, value: &Value) -> Result<memory::MemoryEvidence> {
        fs::write(&self.report, serde_json::to_vec(value)?)?;
        memory::verify_memory(&self.report, &self.binary)
    }
}

fn with_files(test: impl FnOnce(&Files) -> Result<()>) -> Result<()> {
    let files = Files::new()?;
    let result = test(&files);
    let cleanup = fs::remove_dir_all(&files.root);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error).context("cleaning memory fixture"),
        (Err(error), Err(cleanup)) => bail!("{error:#}; cleanup failed: {cleanup}"),
    }
}

fn set(value: &mut Value, pointer: &str, replacement: Value) -> Result<()> {
    *value
        .pointer_mut(pointer)
        .with_context(|| format!("fixture path missing: {pointer}"))? = replacement;
    Ok(())
}

#[test]
fn complete_memory_reports_preserve_exact_per_vm_category_evidence() -> Result<()> {
    with_files(|files| {
        let evidence = files.verify(&files.base)?;
        ensure!(evidence.worker_count == 36 && evidence.velum_phase_count == 162);
        ensure!(evidence.velum_vm_snapshots == 978);
        ensure!(evidence.velum_category_records == 978 * velum::VmStorageKind::all().len());
        let restored = serde_json::from_slice(&serde_json::to_vec(&evidence)?)?;
        memory::ensure_equivalent(&evidence, &restored)
    })
}

#[test]
fn memory_rejects_missing_workers_phases_configuration_and_checksums() -> Result<()> {
    with_files(|files| {
        for (pointer, replacement) in [
            ("/runs", json!([])),
            ("/runs/0/repetition", json!(1)),
            ("/runs/0/outcome", json!("skipped")),
            ("/runs/0/elapsed_ns", json!(0)),
            ("/runs/0/phases", json!([])),
            ("/runs/0/phases/0/phase", json!({"name":"empty_vms"})),
            ("/runs/12/phases/6/phase/round", json!(2)),
            ("/runs/0/phases/3/diagnostics/checksum", json!(u64::MAX)),
            ("/runs/0/phases/1/diagnostics/checksum", json!(42)),
            ("/configuration/scenarios/0/vm_count", json!(2)),
            ("/configuration/child_timeout_ms", json!(1)),
            ("/executable_digest", json!("fnv1a64-0000000000000000")),
            ("/campaign_complete", json!(false)),
        ] {
            let mut report = files.base.clone();
            set(&mut report, pointer, replacement)?;
            ensure!(
                files.verify(&report).is_err(),
                "accepted malformed memory field {pointer}"
            );
        }
        let mut report = files.base.clone();
        let worker = report
            .pointer_mut("/runs/0")
            .and_then(Value::as_object_mut)
            .context("missing worker")?;
        ensure!(worker.remove("phases").is_some());
        ensure!(
            files.verify(&report).is_err(),
            "accepted absent phase evidence"
        );
        let mut report = files.base.clone();
        let phase = report
            .pointer_mut("/runs/0/phases/0/diagnostics")
            .and_then(Value::as_object_mut)
            .context("missing diagnostics")?;
        ensure!(phase.remove("checksum").is_some());
        ensure!(
            files.verify(&report).is_err(),
            "accepted absent nullable checksum field"
        );
        Ok(())
    })
}

#[test]
fn memory_rejects_wrong_vm_category_and_collection_metadata() -> Result<()> {
    with_files(|files| {
        for (pointer, replacement) in [
            ("/runs/0/phases/1/diagnostics/vms", json!([])),
            ("/runs/0/phases/1/diagnostics/vms/0/vm_index", json!(1)),
            (
                "/runs/0/phases/1/diagnostics/vms/0/logical_records",
                json!(true),
            ),
            (
                "/runs/0/phases/1/diagnostics/vms/0/runtime_steps",
                json!(-1),
            ),
            ("/runs/0/phases/1/diagnostics/vms/0/categories", json!([])),
            (
                "/runs/0/phases/1/diagnostics/vms/0/categories/0/category",
                json!("HeapString"),
            ),
            (
                "/runs/0/phases/1/diagnostics/vms/0/categories/0/logical_records",
                json!(2),
            ),
            (
                "/runs/0/phases/1/diagnostics/vms/0/categories/0/logical_payload_bytes",
                json!(1),
            ),
            (
                "/runs/0/phases/1/diagnostics/forced_gc_duration_ns",
                json!(1),
            ),
            (
                "/runs/0/phases/5/diagnostics/velum_reclaimed_records",
                Value::Null,
            ),
            (
                "/runs/0/phases/5/diagnostics/forced_gc_duration_ns",
                Value::Null,
            ),
            (
                "/runs/3/phases/1/diagnostics/vms/0/allocator_bytes",
                json!(-1),
            ),
            (
                "/runs/3/phases/5/diagnostics/velum_reclaimed_records",
                json!(1),
            ),
        ] {
            let mut report = files.base.clone();
            set(&mut report, pointer, replacement)?;
            ensure!(
                files.verify(&report).is_err(),
                "accepted invalid VM evidence {pointer}"
            );
        }
        let mut report = files.base.clone();
        set(
            &mut report,
            "/runs/0/phases/1/diagnostics/vms/0/categories/0/logical_records",
            json!(u64::MAX),
        )?;
        set(
            &mut report,
            "/runs/0/phases/1/diagnostics/vms/0/categories/1/logical_records",
            json!(1),
        )?;
        ensure!(
            files.verify(&report).is_err(),
            "accepted overflowing category sum"
        );
        Ok(())
    })
}

#[test]
fn memory_metrics_require_explicit_availability_but_not_monotonic_hwm() -> Result<()> {
    with_files(|files| {
        for (pointer, replacement) in [
            ("/runs/0/phases/0/process/pid", json!(0)),
            ("/runs/0/phases/1/process/pid", json!(43)),
            (
                "/runs/0/phases/0/process/rss_bytes",
                json!({"status":"available"}),
            ),
            (
                "/runs/0/phases/0/process/pss_bytes",
                json!({"status":"unavailable","reason":""}),
            ),
            (
                "/runs/0/phases/0/process/peak_rss_bytes",
                json!({"status":"available","bytes":1.5}),
            ),
        ] {
            let mut report = files.base.clone();
            set(&mut report, pointer, replacement)?;
            ensure!(
                files.verify(&report).is_err(),
                "accepted malformed OS metric {pointer}"
            );
        }
        let baseline = files.verify(&files.base)?;
        let mut report = files.base.clone();
        set(
            &mut report,
            "/runs/0/phases/0/process/pss_bytes",
            json!({"status":"unavailable","reason":"smaps unavailable"}),
        )?;
        set(
            &mut report,
            "/runs/0/phases/1/process/peak_rss_bytes",
            json!({"status":"available","bytes":4096}),
        )?;
        set(
            &mut report,
            "/runs/0/phases/5/diagnostics/forced_gc_duration_ns",
            json!(999_999),
        )?;
        set(
            &mut report,
            "/runs/3/phases/1/diagnostics/vms/0/allocator_bytes",
            json!(276),
        )?;
        memory::ensure_equivalent(&baseline, &files.verify(&report)?)
    })
}

#[test]
fn logical_drift_is_rejected_even_when_aggregate_totals_are_unchanged() -> Result<()> {
    with_files(|files| {
        let baseline = files.verify(&files.base)?;
        let mut report = files.base.clone();
        set(
            &mut report,
            "/runs/0/phases/1/diagnostics/vms/0/categories/0/logical_records",
            json!(0),
        )?;
        set(
            &mut report,
            "/runs/0/phases/1/diagnostics/vms/0/categories/1/logical_records",
            json!(1),
        )?;
        let error = memory::ensure_equivalent(&baseline, &files.verify(&report)?)
            .err()
            .context("accepted category-only drift")?;
        ensure!(
            error.to_string().contains("needs review") && error.to_string().contains("hello-world")
        );
        for (pointer, replacement) in [
            (
                "/runs/0/phases/1/diagnostics/vms/0/runtime_steps",
                json!(11),
            ),
            (
                "/runs/0/phases/5/diagnostics/velum_reclaimed_records",
                json!(2),
            ),
        ] {
            let mut report = files.base.clone();
            set(&mut report, pointer, replacement)?;
            ensure!(
                memory::ensure_equivalent(&baseline, &files.verify(&report)?).is_err(),
                "accepted logical drift {pointer}"
            );
        }
        let mut report = files.base.clone();
        for (vm, records) in [(0, 0), (1, 2)] {
            set(
                &mut report,
                &format!("/runs/24/phases/1/diagnostics/vms/{vm}/logical_records"),
                json!(records),
            )?;
            set(
                &mut report,
                &format!("/runs/24/phases/1/diagnostics/vms/{vm}/categories/0/logical_records"),
                json!(records),
            )?;
        }
        ensure!(
            memory::ensure_equivalent(&baseline, &files.verify(&report)?).is_err(),
            "accepted per-VM redistribution with unchanged aggregate"
        );
        Ok(())
    })
}
