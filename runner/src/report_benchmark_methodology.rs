use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::{
    benchmark_protocol::{
        BenchmarkChecksum as SourceChecksum, BenchmarkMethodology as SourceMethodology,
        BenchmarkMode as SourceMode, BenchmarkReferenceSource as SourceReference,
        LifecyclePhase as SourcePhase, ReportedLifecycle,
    },
    report_schema_support::duration_ns,
};

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkMethodology {
    pub(crate) mode: Option<BenchmarkMode>,
    pub(crate) lifecycle: Option<BenchmarkLifecycle>,
    pub(crate) checksum: Option<BenchmarkChecksum>,
    pub(crate) reference_source: Option<ReferenceSource>,
}

impl From<SourceMethodology> for BenchmarkMethodology {
    fn from(value: SourceMethodology) -> Self {
        Self {
            mode: value.mode.map(BenchmarkMode::from),
            lifecycle: value.lifecycle.map(BenchmarkLifecycle::from),
            checksum: value.checksum.map(BenchmarkChecksum::from),
            reference_source: value.reference_source.map(ReferenceSource::from),
        }
    }
}

impl BenchmarkMethodology {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.mode.is_none()
            && (self.lifecycle.is_some()
                || self.checksum.is_some()
                || self.reference_source.is_some())
        {
            bail!("benchmark methodology values require a benchmark mode");
        }
        if self.checksum.is_some() && self.mode != Some(BenchmarkMode::PreparedExecution) {
            bail!("benchmark checksum requires prepared execution mode");
        }
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkMode {
    ColdEval,
    PreparedExecution,
}

impl BenchmarkMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ColdEval => "cold_eval",
            Self::PreparedExecution => "prepared_execution",
        }
    }
}

impl From<SourceMode> for BenchmarkMode {
    fn from(value: SourceMode) -> Self {
        match value {
            SourceMode::ColdEval => Self::ColdEval,
            SourceMode::PreparedExecution => Self::PreparedExecution,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkLifecycle {
    pub(crate) load: LifecyclePhase,
    pub(crate) compile: LifecyclePhase,
    pub(crate) setup: LifecyclePhase,
    pub(crate) warmup: LifecyclePhase,
    pub(crate) run: LifecyclePhase,
    pub(crate) verify: LifecyclePhase,
    pub(crate) teardown: LifecyclePhase,
}

impl From<ReportedLifecycle> for BenchmarkLifecycle {
    fn from(value: ReportedLifecycle) -> Self {
        Self {
            load: value.load.into(),
            compile: value.compile.into(),
            setup: value.setup.into(),
            warmup: value.warmup.into(),
            run: value.run.into(),
            verify: value.verify.into(),
            teardown: value.teardown.into(),
        }
    }
}

impl BenchmarkLifecycle {
    fn validate(&self) -> anyhow::Result<()> {
        for phase in [
            self.load,
            self.compile,
            self.setup,
            self.warmup,
            self.run,
            self.verify,
            self.teardown,
        ] {
            let has_duration = phase.duration_ns.is_some();
            if has_duration != (phase.kind == LifecyclePhaseKind::Duration) {
                bail!("benchmark lifecycle phase has inconsistent duration metadata");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct LifecyclePhase {
    pub(crate) kind: LifecyclePhaseKind,
    pub(crate) duration_ns: Option<u64>,
}

impl From<SourcePhase> for LifecyclePhase {
    fn from(value: SourcePhase) -> Self {
        match value {
            SourcePhase::Duration(value) => Self {
                kind: LifecyclePhaseKind::Duration,
                duration_ns: Some(duration_ns(value)),
            },
            SourcePhase::Measured => phase(LifecyclePhaseKind::Measured),
            SourcePhase::PerOperation => phase(LifecyclePhaseKind::PerOperation),
            SourcePhase::NotMeasured => phase(LifecyclePhaseKind::NotMeasured),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecyclePhaseKind {
    Duration,
    Measured,
    PerOperation,
    NotMeasured,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BenchmarkChecksum {
    Undefined,
    Null,
    Boolean { value: bool },
    Number { bits: u64 },
    String { value: String },
}

impl BenchmarkChecksum {
    pub fn label(&self) -> String {
        match self {
            Self::Undefined => "undefined".to_owned(),
            Self::Null => "null".to_owned(),
            Self::Boolean { value } => value.to_string(),
            Self::Number { bits } => f64::from_bits(*bits).to_string(),
            Self::String { value } => format!("{value:?}"),
        }
    }
}

impl From<SourceChecksum> for BenchmarkChecksum {
    fn from(value: SourceChecksum) -> Self {
        match value {
            SourceChecksum::Undefined => Self::Undefined,
            SourceChecksum::Null => Self::Null,
            SourceChecksum::Boolean(value) => Self::Boolean { value },
            SourceChecksum::Number(bits) => Self::Number { bits },
            SourceChecksum::String(value) => Self::String { value },
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceSource {
    QuickjsBaseline,
    QuickjsLive,
    QuickjsLiveFailed,
    NotConfigured,
}

impl ReferenceSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::QuickjsBaseline => "quickjs_baseline",
            Self::QuickjsLive => "quickjs_live",
            Self::QuickjsLiveFailed => "quickjs_live_failed",
            Self::NotConfigured => "🟡 not configured",
        }
    }
}

impl From<SourceReference> for ReferenceSource {
    fn from(value: SourceReference) -> Self {
        match value {
            SourceReference::QuickjsBaseline => Self::QuickjsBaseline,
            SourceReference::QuickjsLive => Self::QuickjsLive,
            SourceReference::QuickjsLiveFailed => Self::QuickjsLiveFailed,
            SourceReference::NotConfigured => Self::NotConfigured,
        }
    }
}

const fn phase(kind: LifecyclePhaseKind) -> LifecyclePhase {
    LifecyclePhase {
        kind,
        duration_ns: None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{BenchmarkChecksum, BenchmarkMethodology, BenchmarkMode, LifecyclePhaseKind};
    use crate::benchmark_protocol::{
        BenchmarkChecksum as SourceChecksum, BenchmarkLifecycle,
        BenchmarkMethodology as SourceMethodology, BenchmarkMode as SourceMode,
        BenchmarkReferenceSource, ReportedLifecycle,
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn converts_typed_prepared_methodology_without_display_round_trip() -> TestResult {
        let source = SourceMethodology {
            mode: Some(SourceMode::PreparedExecution),
            lifecycle: Some(ReportedLifecycle::prepared(BenchmarkLifecycle {
                load: Duration::from_nanos(1_239),
                compile: Some(Duration::from_nanos(2_501)),
                setup: None,
                warmup: Duration::from_millis(150),
                timed_run: Duration::from_millis(500),
                verify: Some(Duration::from_nanos(4_001)),
                teardown: None,
            })),
            checksum: Some(SourceChecksum::number(-0.0)),
            reference_source: Some(BenchmarkReferenceSource::QuickjsBaseline),
        };
        let methodology = BenchmarkMethodology::from(source);
        let Some(lifecycle) = methodology.lifecycle else {
            return Err("expected lifecycle".into());
        };
        if methodology.mode != Some(BenchmarkMode::PreparedExecution)
            || lifecycle.load.duration_ns != Some(1_239)
            || lifecycle.setup.kind != LifecyclePhaseKind::NotMeasured
            || methodology.checksum
                != Some(BenchmarkChecksum::Number {
                    bits: (-0.0_f64).to_bits(),
                })
        {
            return Err("typed methodology conversion changed exact values".into());
        }
        Ok(())
    }
}
