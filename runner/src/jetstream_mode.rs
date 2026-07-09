use std::path::Path;

use anyhow::bail;

use crate::{jetstream, report_metadata};

pub fn run(report_path: &Path) -> anyhow::Result<()> {
    let report = jetstream::run()?;
    let metadata = report_metadata::RunMetadata::from_env();
    jetstream::write_report(report_path, &metadata, &report)?;
    if report.reference_missing > 0 {
        bail!(
            "JetStream report contains {} missing or stale QuickJS baseline entry/entries; refresh the content-addressed baseline explicitly",
            report.reference_missing
        )
    }
    Ok(())
}
