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
    pub(crate) component: PathBuf,
    pub(crate) exhaustive: Option<PathBuf>,
}

pub const MAX_CANONICAL_YAML_LINES: usize = 1_000;

pub fn write_yaml_artifacts(
    report_path: &Path,
    component: &ReportDocument,
    exhaustive: Option<&ReportDocument>,
) -> anyhow::Result<YamlArtifactPaths> {
    component.validate()?;
    let paths = yaml_artifact_paths(report_path);
    let summary = component.summary();
    summary.validate()?;
    write_bounded_yaml(&paths.summary, &summary)?;
    write_bounded_yaml(&paths.component, component)?;
    if let (Some(path), Some(report)) = (&paths.exhaustive, exhaustive) {
        report.validate()?;
        write_yaml(path, report)?;
    }
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
        .unwrap_or("velum-test-report");
    YamlArtifactPaths {
        summary: report_path.with_extension("yaml"),
        component: report_path.with_file_name(format!("{stem}-component.yaml")),
        exhaustive: exhaustive_enabled()
            .then(|| report_path.with_file_name(format!("{stem}-exhaustive.yaml"))),
    }
}

pub fn exhaustive_enabled() -> bool {
    std::env::var("VELUM_REPORT_EXHAUSTIVE").is_ok_and(|value| value.trim() == "1")
}

fn write_bounded_yaml<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let encoded = serde_yaml_ng::to_string(value)
        .with_context(|| format!("failed to serialize YAML report '{}'", path.display()))?;
    let line_count = encoded.lines().count();
    if line_count > MAX_CANONICAL_YAML_LINES {
        anyhow::bail!(
            "ordinary YAML report '{}' has {line_count} lines; maximum is {MAX_CANONICAL_YAML_LINES}",
            path.display()
        );
    }
    write_text(path, &encoded)
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

fn write_text(path: &Path, value: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create YAML report directory '{}'",
                parent.display()
            )
        })?;
    }
    fs::write(path, value)
        .with_context(|| format!("failed to write YAML report '{}'", path.display()))
}
