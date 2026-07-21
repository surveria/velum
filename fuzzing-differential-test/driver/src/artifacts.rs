use std::{
    env,
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, ensure};

use crate::compare::{CaseClassification, CaseRecord, CompareConfig, compare_script, sha256_hex};
use crate::node_worker::NodeWorker;
use crate::time::parse_duration;

#[derive(Debug, Clone)]
pub struct TargetConfig {
    pub artifact_dir: PathBuf,
    pub node_binary: String,
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
        let mut engine_timeout =
            env::var("VELUM_DIFF_ENGINE_TIMEOUT").unwrap_or_else(|_| "4s".to_owned());
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
                "--engine-timeout" => engine_timeout = next_value(&mut args, "--engine-timeout")?,
                "--slow-ratio" => slow_ratio = next_value(&mut args, "--slow-ratio")?,
                "--slow-min" => slow_min = next_value(&mut args, "--slow-min")?,
                "--save-all" => save_all = true,
                _ => anyhow::bail!("unexpected differential target argument '{argument}'"),
            }
        }

        let artifact_dir = artifact_dir.context("differential artifact directory is required")?;
        let engine_timeout = parse_duration(&engine_timeout)?;
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
            compare: CompareConfig {
                engine_timeout,
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
        let node_worker = NodeWorker::start(
            config.node_binary.clone(),
            config.artifact_dir.join("node-stderr"),
        )?;
        Ok(Self {
            config,
            worker_pid,
            sequence: 0,
            cases,
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
        let compared = compare_script(source, &mut self.node_worker, self.config.compare)?;
        let saved_script = self.save_script_if_needed(&case_id, compared.classification, source)?;
        let velum_status = compared.velum.status;
        let record = CaseRecord {
            case_id,
            worker_pid: self.worker_pid,
            sequence: self.sequence,
            script_sha256,
            script_bytes: u64::try_from(script.len()).unwrap_or(u64::MAX),
            classification: compared.classification,
            saved_script: saved_script.map(|path| path.display().to_string()),
            ratio_velum_to_v8: compared.ratio,
            velum: compared.velum,
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
            crate::compare::OutcomeStatus::Ok => 0,
            crate::compare::OutcomeStatus::JsError
            | crate::compare::OutcomeStatus::Timeout
            | crate::compare::OutcomeStatus::Crash => 1,
        })
    }

    fn save_script_if_needed(
        &self,
        case_id: &str,
        classification: CaseClassification,
        source: &str,
    ) -> anyhow::Result<Option<PathBuf>> {
        let directory = match classification {
            CaseClassification::Match if self.config.save_all => {
                self.config.artifact_dir.join("all")
            }
            CaseClassification::Match => return Ok(None),
            CaseClassification::Mismatch => self.config.artifact_dir.join("findings/mismatches"),
            CaseClassification::Slow => self.config.artifact_dir.join("findings/slow"),
            CaseClassification::V8Timeout => self.config.artifact_dir.join("findings/v8-timeouts"),
            CaseClassification::V8Crash => self.config.artifact_dir.join("findings/v8-crashes"),
        };
        fs::create_dir_all(&directory)
            .with_context(|| format!("failed to create '{}'", directory.display()))?;
        let path = directory.join(format!("{case_id}.js"));
        fs::write(&path, source)
            .with_context(|| format!("failed to save script '{}'", path.display()))?;
        Ok(Some(path))
    }
}

fn create_layout(root: &Path) -> anyhow::Result<()> {
    for path in [
        root,
        &root.join("cases"),
        &root.join("findings"),
        &root.join("findings/mismatches"),
        &root.join("findings/slow"),
        &root.join("findings/v8-timeouts"),
        &root.join("findings/v8-crashes"),
        &root.join("node-stderr"),
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
