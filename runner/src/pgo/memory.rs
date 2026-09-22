//! Frozen PGO memory evidence: logical equivalence is separate from residency.

use std::{
    collections::BTreeSet,
    fs::File,
    io::Read as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

const MAX_REPORT_BYTES: u64 = 32 * 1024 * 1024;
const REPETITIONS: u32 = 3;
const WORKERS: usize = 36;
const SCENARIOS: [(&str, &str, u32, u32, u32, u32); 6] = [
    ("hello-world", "hello_world", 1, 0, 0, 1),
    ("retained-graph", "retained_graph", 1, 1024, 256, 1),
    ("cyclic-churn", "cyclic_churn", 1, 1024, 256, 3),
    ("independent-vms-1", "independent_vms", 1, 64, 256, 1),
    ("independent-vms-10", "independent_vms", 10, 64, 256, 1),
    ("independent-vms-50", "independent_vms", 50, 64, 256, 1),
];

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryEvidence {
    pub worker_count: usize,
    pub velum_phase_count: usize,
    pub velum_vm_snapshots: usize,
    pub velum_category_records: usize,
    storage_categories: Vec<String>,
    logical: Vec<LogicalPhase>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LogicalPhase {
    scenario: String,
    repetition: u32,
    phase: Phase,
    checksum: Option<u64>,
    reclaimed_records: Option<u64>,
    vms: Vec<LogicalVm>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LogicalVm {
    vm_index: u32,
    records: u64,
    payload_bytes: u64,
    runtime_steps: u64,
    // Entries follow storage_categories, and contain records then payload bytes.
    categories: Vec<[u64; 2]>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "name", rename_all = "snake_case", deny_unknown_fields)]
enum Phase {
    ProcessBaseline,
    EmptyVms,
    VmsReady,
    Live { round: u32 },
    RootsReleased { round: u32 },
    AfterGc { round: u32 },
    VmsDropped,
    OwnersDropped,
}

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
enum Engine {
    Velum,
    Quickjs,
}

#[derive(Deserialize)]
struct Report {
    schema_version: u32,
    artifact_kind: String,
    executable_path: PathBuf,
    executable_digest: String,
    configuration: Configuration,
    expected_worker_count: usize,
    campaign_complete: bool,
    runs: Vec<Worker>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    repetitions: u32,
    child_timeout_ms: u64,
    quickjs_compiled: bool,
    scenarios: Vec<Scenario>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    id: String,
    kind: String,
    vm_count: u32,
    nodes_per_vm: u32,
    bytes_per_node: u32,
    rounds: u32,
}

#[derive(Deserialize)]
struct Worker {
    scenario_id: String,
    engine: Engine,
    repetition: u32,
    outcome: String,
    elapsed_ns: u64,
    phases: Vec<Measurement>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Measurement {
    phase: Phase,
    process: Process,
    diagnostics: Diagnostics,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Process {
    pid: u32,
    rss_bytes: Metric,
    pss_bytes: Metric,
    peak_rss_bytes: Metric,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum Metric {
    Available {
        #[serde(rename = "bytes")]
        _bytes: u64,
    },
    Unavailable {
        reason: String,
    },
}

impl Metric {
    fn validate(&self) -> Result<()> {
        match self {
            // Zero is an explicitly measured value, never a missing-data sentinel.
            Self::Available { .. } => Ok(()),
            Self::Unavailable { reason } => {
                ensure!(
                    !reason.trim().is_empty(),
                    "unavailable memory metric needs a reason"
                );
                Ok(())
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Diagnostics {
    vms: Vec<Counters>,
    #[serde(deserialize_with = "Option::deserialize")]
    checksum: Option<u64>,
    #[serde(deserialize_with = "Option::deserialize")]
    forced_gc_duration_ns: Option<u64>,
    #[serde(deserialize_with = "Option::deserialize")]
    velum_reclaimed_records: Option<u64>,
}

#[derive(Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case", deny_unknown_fields)]
enum Counters {
    Velum {
        vm_index: u32,
        logical_records: u64,
        logical_payload_bytes: u64,
        runtime_steps: u64,
        categories: Vec<StorageCategory>,
    },
    Quickjs {
        vm_index: u32,
        #[serde(rename = "allocator_bytes")]
        _allocator_bytes: u64,
        #[serde(rename = "memory_used_bytes")]
        _memory_used_bytes: u64,
        #[serde(rename = "allocator_blocks")]
        _allocator_blocks: u64,
        #[serde(rename = "memory_used_blocks")]
        _memory_used_blocks: u64,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageCategory {
    category: String,
    logical_records: u64,
    logical_payload_bytes: u64,
}

pub fn verify_memory(path: &Path, executable: &Path) -> Result<MemoryEvidence> {
    let mut bytes = Vec::new();
    File::open(path)
        .with_context(|| format!("opening memory report {}", path.display()))?
        .take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        u64::try_from(bytes.len())? <= MAX_REPORT_BYTES,
        "memory report exceeds size limit"
    );
    let mut report: Report = serde_json::from_slice(&bytes)
        .with_context(|| format!("decoding typed memory report {}", path.display()))?;
    ensure!(
        report.schema_version == 1
            && report.artifact_kind == "local_process_isolated_memory_campaign",
        "unsupported memory schema or artifact kind"
    );
    ensure!(
        report.campaign_complete
            && report.expected_worker_count == WORKERS
            && report.runs.len() == WORKERS,
        "incomplete memory worker matrix"
    );
    ensure!(
        report.executable_path == executable
            && report.executable_digest == executable_digest(executable)?,
        "memory executable identity mismatch"
    );
    validate_configuration(&report.configuration)?;
    let expected = SCENARIOS
        .iter()
        .flat_map(|(id, ..)| {
            [Engine::Velum, Engine::Quickjs]
                .into_iter()
                .flat_map(move |engine| {
                    (0..REPETITIONS).map(move |repetition| (*id, engine, repetition))
                })
        })
        .collect::<BTreeSet<_>>();
    let actual = report
        .runs
        .iter()
        .map(|row| (row.scenario_id.as_str(), row.engine, row.repetition))
        .collect::<BTreeSet<_>>();
    ensure!(
        actual == expected,
        "missing, duplicate, or unexpected memory worker"
    );
    let categories = velum::VmStorageKind::all()
        .iter()
        .map(|kind| format!("{kind:?}"))
        .collect::<Vec<_>>();
    report.runs.sort_by(|left, right| {
        (&left.scenario_id, left.engine, left.repetition).cmp(&(
            &right.scenario_id,
            right.engine,
            right.repetition,
        ))
    });
    let mut evidence = MemoryEvidence {
        worker_count: WORKERS,
        velum_phase_count: 0,
        velum_vm_snapshots: 0,
        velum_category_records: 0,
        storage_categories: categories,
        logical: Vec::new(),
    };
    for worker in report.runs {
        validate_worker(&worker, &mut evidence).with_context(|| {
            format!(
                "memory scenario {}, repetition {}",
                worker.scenario_id, worker.repetition
            )
        })?;
    }
    evidence.velum_phase_count = evidence.logical.len();
    Ok(evidence)
}

fn validate_configuration(config: &Configuration) -> Result<()> {
    ensure!(
        config.repetitions == REPETITIONS
            && config.child_timeout_ms == 120_000
            && config.quickjs_compiled,
        "memory sampling configuration changed"
    );
    let actual = config
        .scenarios
        .iter()
        .map(|s| {
            (
                s.id.as_str(),
                s.kind.as_str(),
                s.vm_count,
                s.nodes_per_vm,
                s.bytes_per_node,
                s.rounds,
            )
        })
        .collect::<BTreeSet<_>>();
    ensure!(
        config.scenarios.len() == SCENARIOS.len() && actual == BTreeSet::from(SCENARIOS),
        "memory scenario parameters changed"
    );
    Ok(())
}

fn expected_phases(rounds: u32) -> Vec<Phase> {
    let mut phases = vec![Phase::ProcessBaseline, Phase::EmptyVms, Phase::VmsReady];
    for round in 0..rounds {
        phases.extend([
            Phase::Live { round },
            Phase::RootsReleased { round },
            Phase::AfterGc { round },
        ]);
    }
    phases.extend([Phase::VmsDropped, Phase::OwnersDropped]);
    phases
}

fn validate_worker(worker: &Worker, evidence: &mut MemoryEvidence) -> Result<()> {
    ensure!(
        worker.outcome == "passed" && worker.elapsed_ns > 0,
        "failed, skipped, or unmeasured memory worker"
    );
    let (_, _, count, nodes, _, rounds) = SCENARIOS
        .iter()
        .find(|(id, ..)| *id == worker.scenario_id)
        .context("unknown memory scenario")?;
    let phases = expected_phases(*rounds);
    ensure!(
        worker.phases.len() == phases.len(),
        "missing or extra memory phases"
    );
    let pid = worker
        .phases
        .first()
        .context("no memory phases")?
        .process
        .pid;
    ensure!(pid > 0, "invalid memory worker PID");
    for (measurement, expected) in worker.phases.iter().zip(phases) {
        ensure!(
            measurement.phase == expected,
            "missing, duplicate, or reordered memory phase: expected {expected:?}"
        );
        validate_phase(worker, measurement, *count, *nodes, pid, evidence)
            .with_context(|| format!("memory phase {expected:?}"))?;
    }
    Ok(())
}

fn validate_phase(
    worker: &Worker,
    measurement: &Measurement,
    count: u32,
    nodes: u32,
    pid: u32,
    evidence: &mut MemoryEvidence,
) -> Result<()> {
    ensure!(
        measurement.process.pid == pid,
        "memory worker PID changed between phases"
    );
    for metric in [
        &measurement.process.rss_bytes,
        &measurement.process.pss_bytes,
        &measurement.process.peak_rss_bytes,
    ] {
        metric.validate()?;
    }
    let phase = &measurement.phase;
    let diagnostics = &measurement.diagnostics;
    let dropped = matches!(phase, Phase::ProcessBaseline | Phase::OwnersDropped)
        || (worker.engine == Engine::Velum && matches!(phase, Phase::VmsDropped));
    let expected_vms = if dropped { 0 } else { count };
    ensure!(
        diagnostics.vms.len() == usize::try_from(expected_vms)?,
        "wrong memory VM count"
    );
    let expected_checksum = if matches!(phase, Phase::Live { .. }) {
        let one_vm = if worker.scenario_id == "hello-world" {
            42
        } else {
            (0..nodes).map(|index| u64::from(index % 251)).sum::<u64>()
        };
        Some(
            one_vm
                .checked_mul(u64::from(count))
                .context("memory checksum overflow")?,
        )
    } else {
        None
    };
    ensure!(
        diagnostics.checksum == expected_checksum,
        "memory workload checksum mismatch"
    );
    let after_gc = matches!(phase, Phase::AfterGc { .. });
    ensure!(
        diagnostics.forced_gc_duration_ns.is_some() == after_gc
            && diagnostics.velum_reclaimed_records.is_some()
                == (after_gc && worker.engine == Engine::Velum),
        "invalid memory collection metadata"
    );
    let mut vms = Vec::new();
    for (index, counters) in diagnostics.vms.iter().enumerate() {
        if let Some(vm) = validate_counters(
            counters,
            worker.engine,
            u32::try_from(index)?,
            &evidence.storage_categories,
        )? {
            evidence.velum_vm_snapshots = evidence
                .velum_vm_snapshots
                .checked_add(1)
                .context("memory VM count overflow")?;
            evidence.velum_category_records = evidence
                .velum_category_records
                .checked_add(vm.categories.len())
                .context("memory category count overflow")?;
            vms.push(vm);
        }
    }
    if worker.engine == Engine::Velum {
        evidence.logical.push(LogicalPhase {
            scenario: worker.scenario_id.clone(),
            repetition: worker.repetition,
            phase: phase.clone(),
            checksum: diagnostics.checksum,
            reclaimed_records: diagnostics.velum_reclaimed_records,
            vms,
        });
    }
    Ok(())
}

fn validate_counters(
    counters: &Counters,
    engine: Engine,
    index: u32,
    names: &[String],
) -> Result<Option<LogicalVm>> {
    match counters {
        Counters::Velum {
            vm_index,
            logical_records,
            logical_payload_bytes,
            runtime_steps,
            categories,
        } if engine == Engine::Velum && *vm_index == index => {
            ensure!(
                categories.len() == names.len(),
                "incomplete memory category set for VM {index}"
            );
            let mut records = 0_u64;
            let mut payload = 0_u64;
            let mut projection = Vec::new();
            for (category, name) in categories.iter().zip(names) {
                ensure!(
                    category.category == *name,
                    "wrong memory category order at VM {index}: expected {name}"
                );
                records = records
                    .checked_add(category.logical_records)
                    .context("memory category record sum overflow")?;
                payload = payload
                    .checked_add(category.logical_payload_bytes)
                    .context("memory category payload sum overflow")?;
                projection.push([category.logical_records, category.logical_payload_bytes]);
            }
            ensure!(
                records == *logical_records && payload == *logical_payload_bytes,
                "memory logical category totals do not reconcile for VM {index}"
            );
            Ok(Some(LogicalVm {
                vm_index: index,
                records,
                payload_bytes: payload,
                runtime_steps: *runtime_steps,
                categories: projection,
            }))
        }
        Counters::Quickjs { vm_index, .. } if engine == Engine::Quickjs && *vm_index == index => {
            // Typed unsigned fields reject negatives; allocator telemetry is not a Velum logical counter.
            Ok(None)
        }
        _ => anyhow::bail!("wrong memory counter engine or VM index at {index}"),
    }
}

pub fn ensure_equivalent(left: &MemoryEvidence, right: &MemoryEvidence) -> Result<()> {
    ensure!(
        left.storage_categories == right.storage_categories
            && left.worker_count == right.worker_count
            && left.velum_phase_count == right.velum_phase_count
            && left.velum_vm_snapshots == right.velum_vm_snapshots
            && left.velum_category_records == right.velum_category_records,
        "PGO memory logical coverage changed; needs review"
    );
    for (before, after) in left.logical.iter().zip(&right.logical) {
        ensure!(
            before == after,
            "PGO memory logical drift at scenario {}, repetition {}, phase {:?}; needs review",
            before.scenario,
            before.repetition,
            before.phase
        );
    }
    ensure!(
        left.logical.len() == right.logical.len(),
        "PGO memory logical phases changed; needs review"
    );
    Ok(())
}

fn executable_digest(path: &Path) -> Result<String> {
    let mut file = File::open(path).context("opening memory executable")?;
    let mut buffer = [0_u8; 16_384];
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        for byte in buffer
            .get(..count)
            .context("invalid executable read length")?
        {
            hash ^= u64::from(*byte);
            hash = hash.overflowing_mul(0x0000_0100_0000_01b3).0;
        }
    }
    Ok(format!("fnv1a64-{hash:016x}"))
}
