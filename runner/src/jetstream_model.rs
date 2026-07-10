use tabled::Tabled;

use crate::{bench_measure::MeasureStats, timing};

pub const BUDGET_LABEL: &str = "1.00x";

pub const BUDGET_NUMERATOR: u128 = 100;
pub const BUDGET_DENOMINATOR: u128 = 100;
pub const STATUS_WITHIN_BUDGET: &str = "✅ within budget";
pub const STATUS_TRACKED_EXCEPTION: &str = "🟡 tracked exception";
pub const STATUS_FAILED: &str = "❌ failed";
pub const STATUS_SKIPPED: &str = "🟡 skipped";
pub const STATUS_INVALID_BENCHMARK: &str = "❌ invalid benchmark";
pub const LATENCY_WITHIN: &str = "✅ <= 1.00x";
pub const LATENCY_OVER: &str = "🟡 > 1.00x";
pub const LATENCY_NOT_AVAILABLE: &str = "🟡 unavailable";
pub const LATENCY_INVALID: &str = "❌ invalid";
pub const QUALITY_VALID: &str = "✅ valid";
pub const QUALITY_INVALID: &str = "❌ invalid";
pub const NOT_MEASURED: &str = "-";
pub const DETAIL_COMPLETED: &str = "JetStream shell workload completed";
pub const DETAIL_LATENCY_EXCEPTION: &str = "latency budget exception tracked";
pub const DETAIL_QUALITY_GATE: &str = "measurement quality gate failed";
pub const DETAIL_REFERENCE_COMPLETED: &str = "QuickJS reference completed";
pub const REFERENCE_NOT_CONFIGURED: &str = "🟡 not configured";
pub const REFERENCE_NOT_AVAILABLE: &str = "🟡 not available";
pub const REFERENCE_BASELINE_MISSING: &str = "🟡 baseline missing";
pub const REFERENCE_SOURCE_BASELINE: &str = "committed baseline";
pub const REFERENCE_SOURCE_LIVE: &str = "live refresh";
pub const REFERENCE_SOURCE_MISSING: &str = "baseline miss";
pub const REFERENCE_SOURCE_DISABLED: &str = "disabled";
pub const REFERENCE_MEASURE_CACHED: &str = "cached; not run";
pub const SHELL_PRELUDE: &str = r#"
var console = {
    log: function() {},
    warn: function() {},
    error: function() {},
    assert: function(condition, message) {
        if (!condition)
            throw new Error(message || "console.assert failed");
    }
};
var isInBrowser = false;
// Keep JetStream feature detection on the unsupported typed-array path until
// the engine implements the broader ArrayBufferView surface these workloads use.
var ArrayBuffer = undefined;
var Uint8Array = undefined;
"#;
pub const QUICKJS_PERFORMANCE_PRELUDE: &str = r"
// Bare QuickJS has no Web Performance API. This reference-only compatibility
// surface is not used as the benchmark timer; Rust Instant remains canonical.
var performance = {
    now: function() { return Date.now(); },
    mark: function() {},
    measure: function() {}
};
";
pub const SYNC_HARNESS: &str = r#"
var __rsqjsJetStreamBenchmark = new Benchmark();
var __rsqjsJetStreamResult = __rsqjsJetStreamBenchmark.runIteration();
if (__rsqjsJetStreamResult && typeof __rsqjsJetStreamResult.then === "function") {
    throw new Error("async JetStream workloads are not supported by the synchronous harness");
}
"#;

#[derive(Debug)]
pub struct JetStreamReport {
    pub rows: Vec<JetStreamRow>,
    pub measured: usize,
    pub failed: usize,
    pub invalid: usize,
    pub skipped: usize,
    pub over_latency_budget: usize,
    pub reference_missing: usize,
    pub elapsed: std::time::Duration,
}

impl JetStreamReport {
    #[must_use]
    pub const fn not_run() -> Self {
        Self {
            rows: Vec::new(),
            measured: 0,
            failed: 0,
            invalid: 0,
            skipped: 0,
            over_latency_budget: 0,
            reference_missing: 0,
            elapsed: std::time::Duration::ZERO,
        }
    }
}

#[derive(Debug, Tabled)]
pub struct JetStreamRow {
    pub(crate) benchmark: String,
    pub(crate) status: String,
    pub(crate) source: String,
    pub(crate) case_elapsed: String,
    pub(crate) rsqjs_measure: String,
    pub(crate) quickjs_measure: String,
    pub(crate) reference_source: String,
    pub(crate) rsqjs_time: String,
    pub(crate) quickjs_time: String,
    pub(crate) latency_ratio: String,
    pub(crate) latency_budget: String,
    pub(crate) rsqjs_cv: String,
    pub(crate) quickjs_cv: String,
    pub(crate) quality: String,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Copy)]
pub struct JetStreamCase {
    pub id: &'static str,
    pub files: &'static [&'static str],
    pub mode: JetStreamMode,
}

impl JetStreamCase {
    pub const fn timed(id: &'static str, files: &'static [&'static str]) -> Self {
        Self {
            id,
            files,
            mode: JetStreamMode::Timed,
        }
    }

    pub const fn skipped(id: &'static str, reason: &'static str) -> Self {
        Self {
            id,
            files: &[],
            mode: JetStreamMode::Skipped(reason),
        }
    }

    pub fn source_label(self) -> String {
        match self.files {
            [] => NOT_MEASURED.to_owned(),
            [file] => (*file).to_owned(),
            [first, ..] => format!("{} (+{} more)", first, self.files.len().saturating_sub(1)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum JetStreamMode {
    Timed,
    Skipped(&'static str),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct JetStreamCounts {
    pub measured: usize,
    pub failed: usize,
    pub invalid: usize,
    pub skipped: usize,
    pub over_latency_budget: usize,
    pub reference_missing: usize,
}

#[derive(Debug)]
pub struct JetStreamOutcome {
    pub row: JetStreamRow,
    pub counts: JetStreamCounts,
}

#[derive(Debug)]
pub enum ReferenceMeasurement {
    Missing,
    Disabled,
    CachedUnavailable(String),
    Measured(ReferenceSample),
    Failed(timing::Timed<String>),
}

#[derive(Debug, Clone, Copy)]
pub struct ReferenceSample {
    pub stats: MeasureStats,
    pub elapsed: Option<std::time::Duration>,
    pub source: &'static str,
}

impl ReferenceSample {
    pub fn measure_text(self) -> String {
        self.elapsed.map_or_else(
            || REFERENCE_MEASURE_CACHED.to_owned(),
            timing::format_duration,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BudgetCheck {
    pub label: &'static str,
    pub over_budget: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ReferenceFlags {
    pub skipped: bool,
    pub missing: bool,
}
