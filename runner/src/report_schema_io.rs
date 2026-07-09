use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use serde::Serialize;

use crate::report_schema::{ReportDocument, ReportSummary};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct YamlArtifactPaths {
    pub(crate) summary: PathBuf,
    pub(crate) details: PathBuf,
}

pub fn write_yaml_artifacts(
    report_path: &Path,
    report: &ReportDocument,
) -> anyhow::Result<YamlArtifactPaths> {
    report.validate()?;
    let paths = yaml_artifact_paths(report_path);
    let summary = report.summary();
    summary.validate()?;
    write_yaml(&paths.summary, &summary)?;
    write_yaml(&paths.details, report)?;
    Ok(paths)
}

pub fn read_summary(path: &Path) -> anyhow::Result<ReportSummary> {
    let file = File::open(path)
        .with_context(|| format!("failed to open YAML report '{}'", path.display()))?;
    let report: ReportSummary = serde_yaml_ng::from_reader(BufReader::new(file))
        .with_context(|| format!("failed to parse YAML report '{}'", path.display()))?;
    report.validate()?;
    Ok(report)
}

pub fn read_document(path: &Path) -> anyhow::Result<ReportDocument> {
    let file = File::open(path)
        .with_context(|| format!("failed to open YAML report '{}'", path.display()))?;
    let report: ReportDocument = serde_yaml_ng::from_reader(BufReader::new(file))
        .with_context(|| format!("failed to parse YAML report '{}'", path.display()))?;
    report.validate()?;
    Ok(report)
}

fn yaml_artifact_paths(report_path: &Path) -> YamlArtifactPaths {
    let stem = report_path
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("rsqjs-test-report");
    YamlArtifactPaths {
        summary: report_path.with_extension("yaml"),
        details: report_path.with_file_name(format!("{stem}-details.yaml")),
    }
}

fn write_yaml<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create YAML report directory '{}'",
                parent.display()
            )
        })?;
    }
    let file = File::create(path)
        .with_context(|| format!("failed to create YAML report '{}'", path.display()))?;
    serde_yaml_ng::to_writer(BufWriter::new(file), value)
        .with_context(|| format!("failed to write YAML report '{}'", path.display()))
}
