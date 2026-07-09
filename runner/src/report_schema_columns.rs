use crate::{
    benchmark_protocol::BenchmarkContributionFlag as SourceContributionFlag,
    report_benchmark_methodology::BenchmarkMethodology,
};

use super::BenchmarkContributionFlag;

impl From<SourceContributionFlag> for BenchmarkContributionFlag {
    fn from(value: SourceContributionFlag) -> Self {
        match value {
            SourceContributionFlag::NotCounted => Self::NotCounted,
            SourceContributionFlag::Counted => Self::Counted,
        }
    }
}

pub(super) struct BenchmarkColumns {
    pub(super) id: String,
    pub(super) status: String,
    pub(super) source: String,
    pub(super) iterations: Option<u64>,
    pub(super) case_duration: String,
    pub(super) engine_measurement: String,
    pub(super) engine_median: String,
    pub(super) engine_cv: String,
    pub(super) reference_measurement: String,
    pub(super) reference_median: String,
    pub(super) reference_cv: String,
    pub(super) latency_ratio: String,
    pub(super) memory_ratio: String,
    pub(super) latency_budget: String,
    pub(super) quality: String,
    pub(super) methodology: Option<BenchmarkMethodology>,
    pub(super) detail: String,
}
