use std::path::Path;

use anyhow::bail;

use crate::{
    host_benchmark_lock, jetstream, report_metadata,
    report_schema::{EnvironmentInfo, ReportDocument, RunConfiguration},
};

pub fn run(report_path: &Path) -> anyhow::Result<()> {
    let report = host_benchmark_lock::with_exclusive("JetStream benchmark suite", jetstream::run)?;
    let reference_missing = report.reference_missing;
    let metadata = report_metadata::RunMetadata::from_env();
    let report = ReportDocument::from_jetstream_run(
        &report,
        metadata,
        EnvironmentInfo::capture(),
        RunConfiguration::capture_jetstream()?,
    )?;
    crate::write_report(report_path, &report)?;
    if reference_missing > 0 {
        bail!(
            "JetStream report contains {reference_missing} missing or stale QuickJS baseline entry/entries; refresh the content-addressed baseline explicitly"
        )
    }
    Ok(())
}
