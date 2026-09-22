use std::{
    fs::{self, File},
    io::{Read as _, Write as _},
    path::Path,
};

use anyhow::{Context as _, bail};
use serde::Serialize;
use tabled::{Table, Tabled};

use crate::{report_metadata::RunMetadata, report_schema::EnvironmentInfo};

use super::{
    config::{self, Config},
    engines,
    model::{EngineKind, Outcome, Phase, RunMeasurement},
};

const SCHEMA_VERSION: u32 = 1;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const LIMITATIONS: &[&str] = &[
    "empty_vms includes initialized built-ins but no workload script; vms_ready additionally includes compilation and installation of the workload functions.",
    "RSS/PSS describe the isolated worker process, not a VM allocator. All measurements include worker, protocol and runtime overhead.",
    "Physical snapshots are taken while the child waits for acknowledgement, before collecting that phase's logical counters. Earlier counter serialization can retain allocator capacity.",
    "Peak RSS is the kernel-reported process-lifetime VmHWM, including process startup; there is no resettable phase peak and no claimed peak PSS.",
    "Velum records/payload bytes exclude fixed layouts, spare capacity, allocator headers, compiled artifacts and opaque host captures. They are not divided by QuickJS allocator statistics.",
    "Forced-GC duration is an explicit host call over all live VMs, not automatic-GC pause telemetry. Total Rust allocation counts are not measured.",
    "Residual RSS after teardown may reflect allocator retention or mapped pages; it is not by itself evidence of leaked live objects.",
    "Each QuickJS VM has its own Runtime and full Context. Velum VMs are independent but share one immutable compiled setup script within a worker.",
    "The vms_dropped phase drops Velum VMs or QuickJS contexts; owners_dropped additionally drops the compiled Velum script/runtime or the independent QuickJS runtimes.",
    "The host lock serializes participating benchmark runners, not arbitrary external activity. Results are specific to the recorded host, build and configuration.",
    "Missing operating-system metrics are explicitly unavailable. A passed row verifies execution/protocol/checksums and does not imply every physical metric was available.",
    "Async host payloads, allocator attribution, automatic-GC telemetry and additional device classes remain separate follow-up work.",
];

#[derive(Serialize)]
struct SourceEvidence {
    scenario_id: String,
    source_digest: String,
    source: String,
}

#[derive(Serialize)]
pub struct MemoryReport {
    schema_version: u32,
    artifact_kind: &'static str,
    metadata: RunMetadata,
    environment: EnvironmentInfo,
    executable_path: String,
    executable_digest: String,
    harness_source_digests: Vec<(&'static str, String)>,
    reference_identity: &'static str,
    velum_runtime_limits: String,
    quickjs_memory_limit_bytes_per_runtime: usize,
    configuration: Config,
    workload_sources: Vec<SourceEvidence>,
    limitations: &'static [&'static str],
    expected_worker_count: usize,
    campaign_complete: bool,
    elapsed_ns: u64,
    pub runs: Vec<RunMeasurement>,
}

impl MemoryReport {
    pub fn new(config: Config, executable: &Path) -> anyhow::Result<Self> {
        let expected_worker_count = config
            .scenarios
            .len()
            .checked_mul(config.repetitions)
            .and_then(|count| count.checked_mul(2))
            .context("memory worker count overflowed")?;
        let workload_sources = config
            .scenarios
            .iter()
            .map(|scenario| {
                let source = config::source(scenario);
                SourceEvidence {
                    scenario_id: scenario.id.clone(),
                    source_digest: crate::quickjs_baseline::stable_digest(source.as_bytes()),
                    source,
                }
            })
            .collect();
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            artifact_kind: "local_process_isolated_memory_campaign",
            metadata: RunMetadata::from_env(),
            environment: EnvironmentInfo::capture(),
            executable_path: executable.display().to_string(),
            executable_digest: executable_digest(executable)?,
            harness_source_digests: harness_digests(),
            reference_identity: crate::bench_engines::REFERENCE_ENGINE_ID,
            velum_runtime_limits: format!("{:?}", engines::limits()),
            quickjs_memory_limit_bytes_per_runtime: engines::QUICKJS_MEMORY_LIMIT_BYTES,
            configuration: config,
            workload_sources,
            limitations: LIMITATIONS,
            expected_worker_count,
            campaign_complete: false,
            elapsed_ns: 0,
            runs: Vec::new(),
        })
    }

    pub fn set_elapsed(&mut self, elapsed: std::time::Duration) -> anyhow::Result<()> {
        self.elapsed_ns =
            u64::try_from(elapsed.as_nanos()).context("memory campaign duration overflowed")?;
        self.campaign_complete = self.runs.len() == self.expected_worker_count;
        Ok(())
    }

    pub fn write(&self, path: &Path) -> anyhow::Result<()> {
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            bail!("memory report path must end in .md");
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create memory report directory {}",
                    parent.display()
                )
            })?;
        }
        let json = path.with_extension("json");
        let yaml = path.with_extension("yaml");
        write_atomic(&json, |file| {
            serde_json::to_writer_pretty(file, self).context("failed to serialize raw memory JSON")
        })?;
        write_atomic(&yaml, |file| {
            serde_yaml_ng::to_writer(file, self).context("failed to serialize raw memory YAML")
        })?;
        let markdown = self.markdown(&json, &yaml);
        write_atomic(path, |file| {
            file.write_all(markdown.as_bytes())
                .context("failed to write memory Markdown")
        })?;
        Ok(())
    }

    pub fn print_summary(&self, path: &Path) {
        println!("{}", Table::new(self.summary_rows()));
        println!("memory report: {}", path.display());
        println!(
            "raw memory artifacts: {}, {}",
            path.with_extension("json").display(),
            path.with_extension("yaml").display()
        );
    }

    fn markdown(&self, json: &Path, yaml: &Path) -> String {
        let passed = self
            .runs
            .iter()
            .filter(|run| run.outcome == Outcome::Passed)
            .count();
        let failed = self
            .runs
            .iter()
            .filter(|run| run.outcome == Outcome::Failed)
            .count();
        let skipped = self
            .runs
            .iter()
            .filter(|run| run.outcome == Outcome::Skipped)
            .count();
        let mut lines = vec![
            "# Process-Isolated Memory Benchmark".to_owned(),
            String::new(),
            format!("- Completed workers: {passed} passed, {failed} failed, {skipped} skipped."),
            format!("- Campaign complete: {}; expected workers: {}.", self.campaign_complete, self.expected_worker_count),
            format!("- Campaign elapsed: {} ns.", self.elapsed_ns),
            format!("- Raw phase snapshots and exact workload sources: `{}` and `{}`.", json.display(), yaml.display()),
            format!("- Executable content digest (non-cryptographic FNV-1a): `{}`.", self.executable_digest),
            "- Cells are the lower median [minimum..maximum] in KiB across successful fresh workers; n denotes available samples. Unavailable samples are never zero-filled.".to_owned(),
            String::new(),
            format!("```text\n{}\n```", Table::new(self.summary_rows())),
            String::new(),
        ];
        lines.extend(crate::report_metadata::render_section(&self.metadata));
        lines.extend(["## Failed Workers".to_owned(), String::new()]);
        for run in self
            .runs
            .iter()
            .filter(|run| run.outcome == Outcome::Failed)
            .take(10)
        {
            let detail: String = run.detail.chars().take(512).collect();
            lines.push(format!(
                "- {} / {} / repetition {}: {}",
                run.scenario_id,
                run.engine.label(),
                run.repetition,
                detail.replace('\n', " ")
            ));
        }
        if failed == 0 {
            lines.push("None.".to_owned());
        }
        lines.extend([
            String::new(),
            "## Measurement Boundaries".to_owned(),
            String::new(),
        ]);
        lines.extend(LIMITATIONS.iter().map(|note| format!("- {note}")));
        lines.push(String::new());
        lines.join("\n")
    }

    fn summary_rows(&self) -> Vec<SummaryRow> {
        let mut rows = Vec::new();
        for scenario in &self.configuration.scenarios {
            for engine in [EngineKind::Velum, EngineKind::Quickjs] {
                let runs: Vec<_> = self
                    .runs
                    .iter()
                    .filter(|run| run.scenario_id == scenario.id && run.engine == engine)
                    .collect();
                let passed = runs
                    .iter()
                    .filter(|run| run.outcome == Outcome::Passed)
                    .count();
                let failed = runs
                    .iter()
                    .filter(|run| run.outcome == Outcome::Failed)
                    .count();
                let skipped = runs
                    .iter()
                    .filter(|run| run.outcome == Outcome::Skipped)
                    .count();
                rows.push(SummaryRow {
                    scenario: scenario.id.clone(),
                    engine: engine.label(),
                    status: format!("{passed} pass / {failed} fail / {skipped} skip"),
                    live_rss_kib: metric_summary(&runs, PhaseChoice::Live, MetricChoice::Rss),
                    after_gc_rss_kib: metric_summary(
                        &runs,
                        PhaseChoice::AfterGc,
                        MetricChoice::Rss,
                    ),
                    after_gc_pss_kib: metric_summary(
                        &runs,
                        PhaseChoice::AfterGc,
                        MetricChoice::Pss,
                    ),
                    after_drop_rss_kib: metric_summary(
                        &runs,
                        PhaseChoice::Dropped,
                        MetricChoice::Rss,
                    ),
                    lifetime_peak_rss_kib: metric_summary(
                        &runs,
                        PhaseChoice::Dropped,
                        MetricChoice::PeakRss,
                    ),
                });
            }
        }
        rows
    }
}

#[derive(Tabled)]
struct SummaryRow {
    scenario: String,
    engine: &'static str,
    status: String,
    live_rss_kib: String,
    after_gc_rss_kib: String,
    after_gc_pss_kib: String,
    after_drop_rss_kib: String,
    lifetime_peak_rss_kib: String,
}

#[derive(Clone, Copy)]
enum PhaseChoice {
    Live,
    AfterGc,
    Dropped,
}
#[derive(Clone, Copy)]
enum MetricChoice {
    Rss,
    Pss,
    PeakRss,
}

fn metric_summary(
    runs: &[&RunMeasurement],
    phase_choice: PhaseChoice,
    metric: MetricChoice,
) -> String {
    let mut samples = Vec::new();
    let mut eligible = 0_usize;
    for run in runs.iter().filter(|run| run.outcome == Outcome::Passed) {
        eligible = eligible.saturating_add(1);
        let phase = run.phases.iter().rev().find(|phase| match phase_choice {
            PhaseChoice::Live => matches!(phase.phase, Phase::Live { .. }),
            PhaseChoice::AfterGc => matches!(phase.phase, Phase::AfterGc { .. }),
            PhaseChoice::Dropped => phase.phase == Phase::OwnersDropped,
        });
        let value = phase.and_then(|phase| {
            match metric {
                MetricChoice::Rss => &phase.process.rss_bytes,
                MetricChoice::Pss => &phase.process.pss_bytes,
                MetricChoice::PeakRss => &phase.process.peak_rss_bytes,
            }
            .bytes()
        });
        if let Some(bytes) = value {
            samples.push(bytes / 1_024);
        }
    }
    samples.sort_unstable();
    let middle = samples.len().saturating_sub(1) / 2;
    match (samples.first(), samples.get(middle), samples.last()) {
        (Some(min), Some(median), Some(max)) => {
            format!("{median} [{min}..{max}] n={}/{eligible}", samples.len())
        }
        _ => "unavailable".to_owned(),
    }
}

fn write_atomic(
    path: &Path,
    write: impl FnOnce(&mut File) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let file_name = path
        .file_name()
        .context("memory report has no filename")?
        .to_string_lossy();
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()));
    let mut file = File::create(&temporary)
        .with_context(|| format!("failed to create {}", temporary.display()))?;
    write(&mut file)?;
    file.sync_all().context("failed to sync memory report")?;
    fs::rename(&temporary, path)
        .with_context(|| format!("failed to publish memory report {}", path.display()))
}

fn executable_digest(path: &Path) -> anyhow::Result<String> {
    let mut file =
        File::open(path).context("failed to open memory worker executable for identity")?;
    let mut buffer = [0_u8; 16_384];
    let mut hash = FNV_OFFSET;
    loop {
        let count = file
            .read(&mut buffer)
            .context("failed to read memory worker executable")?;
        if count == 0 {
            break;
        }
        for byte in buffer
            .get(..count)
            .context("memory executable read exceeded buffer")?
        {
            hash ^= u64::from(*byte);
            let (product, _overflowed) = hash.overflowing_mul(FNV_PRIME);
            hash = product;
        }
    }
    Ok(format!("fnv1a64-{hash:016x}"))
}

fn harness_digests() -> Vec<(&'static str, String)> {
    [
        (
            "memory_benchmarks.rs",
            include_str!("../memory_benchmarks.rs"),
        ),
        ("config.rs", include_str!("config.rs")),
        ("model.rs", include_str!("model.rs")),
        ("engines.rs", include_str!("engines.rs")),
        ("quickjs.rs", include_str!("quickjs.rs")),
        ("process.rs", include_str!("process.rs")),
        ("proc_metrics.rs", include_str!("proc_metrics.rs")),
        ("protocol.rs", include_str!("protocol.rs")),
        ("worker.rs", include_str!("worker.rs")),
        ("report.rs", include_str!("report.rs")),
    ]
    .into_iter()
    .map(|(name, source)| {
        (
            name,
            crate::quickjs_baseline::stable_digest(source.as_bytes()),
        )
    })
    .collect()
}
