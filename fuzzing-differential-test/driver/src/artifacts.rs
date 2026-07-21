use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, ensure};

use crate::compare::{
    CaseClassification, CaseFinding, CaseRecord, CompareConfig, OutcomeStatus, compare_script,
    sha256_hex,
};
use crate::engine262_worker::Engine262Worker;
use crate::node_worker::NodeWorker;
use crate::time::parse_duration;

#[derive(Debug, Clone)]
pub struct TargetConfig {
    pub artifact_dir: PathBuf,
    pub node_binary: String,
    pub engine262_package_dir: PathBuf,
    pub compare: CompareConfig,
    pub save_all: bool,
}

impl TargetConfig {
    /// Loads target configuration from launcher-provided arguments or variables.
    ///
    /// # Errors
    ///
    /// Returns an error when a required value is missing or invalid.
    pub fn from_args_or_env(args: impl IntoIterator<Item = String>) -> anyhow::Result<Self> {
        let mut artifact_dir = env::var("VELUM_DIFF_ARTIFACT_DIR").ok().map(PathBuf::from);
        let mut node_binary =
            env::var("VELUM_DIFF_NODE_BINARY").unwrap_or_else(|_| "node".to_owned());
        let mut engine262_package_dir = env::var("VELUM_DIFF_ENGINE262_PACKAGE_DIR")
            .ok()
            .map(PathBuf::from);
        let mut engine262_timeout = env::var("VELUM_DIFF_ENGINE262_TIMEOUT")
            .or_else(|_| env::var("VELUM_DIFF_ENGINE_TIMEOUT"))
            .unwrap_or_else(|_| "30s".to_owned());
        let mut v8_timeout = env::var("VELUM_DIFF_V8_TIMEOUT")
            .or_else(|_| env::var("VELUM_DIFF_ENGINE_TIMEOUT"))
            .unwrap_or_else(|_| "4s".to_owned());
        let mut slow_min = env::var("VELUM_DIFF_SLOW_MIN").unwrap_or_else(|_| "5ms".to_owned());
        let mut slow_ratio = env::var("VELUM_DIFF_SLOW_RATIO").unwrap_or_else(|_| "10".to_owned());
        let mut save_all = env::var("VELUM_DIFF_SAVE_ALL").is_ok_and(|value| value == "1");

        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--artifact-dir" => {
                    artifact_dir = Some(PathBuf::from(next_value(&mut args, "--artifact-dir")?));
                }
                "--node" => node_binary = next_value(&mut args, "--node")?,
                "--engine262-package-dir" => {
                    engine262_package_dir = Some(PathBuf::from(next_value(
                        &mut args,
                        "--engine262-package-dir",
                    )?));
                }
                "--engine-timeout" => {
                    let value = next_value(&mut args, "--engine-timeout")?;
                    engine262_timeout.clone_from(&value);
                    v8_timeout = value;
                }
                "--engine262-timeout" => {
                    engine262_timeout = next_value(&mut args, "--engine262-timeout")?;
                }
                "--v8-timeout" => v8_timeout = next_value(&mut args, "--v8-timeout")?,
                "--slow-ratio" => slow_ratio = next_value(&mut args, "--slow-ratio")?,
                "--slow-min" => slow_min = next_value(&mut args, "--slow-min")?,
                "--save-all" => save_all = true,
                _ => anyhow::bail!("unexpected differential target argument '{argument}'"),
            }
        }

        let artifact_dir = artifact_dir.context("differential artifact directory is required")?;
        let engine262_package_dir =
            engine262_package_dir.context("Engine262 package directory is required")?;
        let engine262_timeout = parse_duration(&engine262_timeout)?;
        let v8_timeout = parse_duration(&v8_timeout)?;
        let slow_min = parse_duration(&slow_min)?;
        let slow_ratio = slow_ratio
            .parse::<f64>()
            .context("slow ratio must be a number")?;
        ensure!(
            slow_ratio.is_finite() && slow_ratio > 0.0,
            "slow ratio must be a positive finite number"
        );
        Ok(Self {
            artifact_dir,
            node_binary,
            engine262_package_dir,
            compare: CompareConfig {
                engine262_timeout,
                v8_timeout,
                slow_ratio,
                slow_min,
            },
            save_all,
        })
    }
}

pub struct ArtifactRecorder {
    config: TargetConfig,
    worker_pid: u32,
    sequence: u64,
    cases: File,
    engine262_worker: Engine262Worker,
    node_worker: NodeWorker,
}

impl ArtifactRecorder {
    /// Creates the recorder and all shared artifact subdirectories.
    ///
    /// # Errors
    ///
    /// Returns an error when directories or JSONL files cannot be created.
    pub fn new(config: TargetConfig) -> anyhow::Result<Self> {
        create_layout(&config.artifact_dir)?;
        let worker_pid = std::process::id();
        let cases_path = config
            .artifact_dir
            .join("cases")
            .join(format!("cases-{worker_pid}.jsonl"));
        let cases = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&cases_path)
            .with_context(|| format!("failed to open cases log '{}'", cases_path.display()))?;
        let engine262_worker = Engine262Worker::start(
            config.node_binary.clone(),
            config.engine262_package_dir.clone(),
            config.artifact_dir.join("engine262-stderr"),
        )?;
        let node_worker = NodeWorker::start(
            config.node_binary.clone(),
            config.artifact_dir.join("v8-stderr"),
        )?;
        Ok(Self {
            config,
            worker_pid,
            sequence: 0,
            cases,
            engine262_worker,
            node_worker,
        })
    }

    /// Compares and records one generated JavaScript program.
    ///
    /// # Errors
    ///
    /// Returns an error when comparison or artifact writes fail.
    pub fn record(&mut self, script: &[u8]) -> anyhow::Result<u8> {
        let source = std::str::from_utf8(script).context("generated script is not UTF-8")?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .context("case sequence overflow")?;
        let script_sha256 = sha256_hex(script);
        let case_id = format!(
            "pid{}_seq{:010}_{}",
            self.worker_pid,
            self.sequence,
            short_hash(&script_sha256)
        );
        let pending_script = self.save_pending_script(&case_id, source)?;
        let compared = compare_script(
            source,
            &mut self.engine262_worker,
            &mut self.node_worker,
            self.config.compare,
        )?;
        Self::remove_pending_script(&pending_script)?;
        let saved_scripts = self.save_scripts_if_needed(&case_id, &compared.findings, source)?;
        let saved_script = saved_scripts.first().cloned();
        let velum_status = compared.velum.status;
        let record = CaseRecord {
            case_id,
            worker_pid: self.worker_pid,
            sequence: self.sequence,
            script_sha256,
            script_bytes: u64::try_from(script.len()).unwrap_or(u64::MAX),
            classification: compared.classification,
            findings: compared.findings,
            saved_script: saved_script.as_ref().map(|path| path.display().to_string()),
            saved_scripts: saved_scripts
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            ratio_velum_to_v8: compared.ratio,
            velum: compared.velum,
            engine262: compared.engine262,
            v8: compared.v8,
        };
        serde_json::to_writer(&mut self.cases, &record)
            .context("failed to write differential case record")?;
        self.cases
            .write_all(b"\n")
            .context("failed to terminate differential case record")?;
        self.cases
            .flush()
            .context("failed to flush differential case record")?;
        Ok(match velum_status {
            OutcomeStatus::Ok => 0,
            OutcomeStatus::JsError | OutcomeStatus::Timeout | OutcomeStatus::Crash => 1,
        })
    }

    fn save_pending_script(&self, case_id: &str, source: &str) -> anyhow::Result<PathBuf> {
        let directory = self.config.artifact_dir.join("pending");
        fs::create_dir_all(&directory)
            .with_context(|| format!("failed to create '{}'", directory.display()))?;
        let path = directory.join(format!("{case_id}.js"));
        fs::write(&path, source)
            .with_context(|| format!("failed to save pending script '{}'", path.display()))?;
        Ok(path)
    }

    fn remove_pending_script(path: &Path) -> anyhow::Result<()> {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error)
                .with_context(|| format!("failed to remove pending script '{}'", path.display())),
        }
    }

    fn save_scripts_if_needed(
        &self,
        case_id: &str,
        findings: &[CaseFinding],
        source: &str,
    ) -> anyhow::Result<Vec<PathBuf>> {
        let mut directories = findings
            .iter()
            .map(|finding| finding_directory(&self.config.artifact_dir, *finding))
            .collect::<Vec<_>>();
        if findings.is_empty() && self.config.save_all {
            directories.push(self.config.artifact_dir.join("all"));
        }
        directories.sort();
        directories.dedup();

        let mut paths = Vec::new();
        for directory in directories {
            fs::create_dir_all(&directory)
                .with_context(|| format!("failed to create '{}'", directory.display()))?;
            let path = directory.join(format!("{case_id}.js"));
            fs::write(&path, source)
                .with_context(|| format!("failed to save script '{}'", path.display()))?;
            paths.push(path);
        }
        Ok(paths)
    }
}

fn finding_directory(root: &Path, finding: CaseFinding) -> PathBuf {
    match finding {
        CaseFinding::CorrectnessMismatch => root.join("findings/correctness-mismatches"),
        CaseFinding::PerformanceSlow => root.join("findings/performance-slow"),
        CaseFinding::VelumTimeout => root.join("findings/velum-timeouts"),
        CaseFinding::VelumCrash => root.join("findings/velum-crashes"),
        CaseFinding::Engine262Timeout => root.join("findings/engine262-timeouts"),
        CaseFinding::Engine262Crash => root.join("findings/engine262-crashes"),
        CaseFinding::V8Timeout => root.join("findings/v8-timeouts"),
        CaseFinding::V8Crash => root.join("findings/v8-crashes"),
    }
}

impl CaseClassification {
    #[must_use]
    pub const fn as_legacy_finding(self) -> Option<CaseFinding> {
        match self {
            Self::Match => None,
            Self::CorrectnessMismatch => Some(CaseFinding::CorrectnessMismatch),
            Self::PerformanceSlow => Some(CaseFinding::PerformanceSlow),
            Self::VelumTimeout => Some(CaseFinding::VelumTimeout),
            Self::VelumCrash => Some(CaseFinding::VelumCrash),
            Self::Engine262Timeout => Some(CaseFinding::Engine262Timeout),
            Self::Engine262Crash => Some(CaseFinding::Engine262Crash),
            Self::V8Timeout => Some(CaseFinding::V8Timeout),
            Self::V8Crash => Some(CaseFinding::V8Crash),
        }
    }
}

#[must_use]
pub fn normalized_findings(record: &CaseRecord) -> Vec<CaseFinding> {
    if !record.findings.is_empty() {
        return record.findings.clone();
    }
    record
        .classification
        .as_legacy_finding()
        .into_iter()
        .collect()
}

fn create_layout(root: &Path) -> anyhow::Result<()> {
    for path in [
        root,
        &root.join("cases"),
        &root.join("findings"),
        &root.join("findings/correctness-mismatches"),
        &root.join("findings/performance-slow"),
        &root.join("findings/velum-timeouts"),
        &root.join("findings/velum-crashes"),
        &root.join("findings/engine262-timeouts"),
        &root.join("findings/engine262-crashes"),
        &root.join("findings/v8-timeouts"),
        &root.join("findings/v8-crashes"),
        &root.join("pending"),
        &root.join("engine262-stderr"),
        &root.join("v8-stderr"),
    ] {
        fs::create_dir_all(path)
            .with_context(|| format!("failed to create artifact directory '{}'", path.display()))?;
    }
    Ok(())
}

fn next_value(args: &mut impl Iterator<Item = String>, option: &str) -> anyhow::Result<String> {
    args.next()
        .with_context(|| format!("missing value after {option}"))
}

fn short_hash(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}
