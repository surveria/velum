use std::{
    env, fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, bail, ensure};

use crate::time::{parse_duration, unix_timestamp_millis};

const DEFAULT_ARTIFACT_ROOT: &str = "/home/user/velum-fuzzing-artifacts/differential-v8";
const DEFAULT_CONFIG: &str = "config/default.json";

pub const HELP: &str = r"Usage: velum-diff-fuzz [OPTIONS]

Run a local differential campaign comparing Velum with Engine262 and V8/Node.

Options:
  --config PATH         JSON config file (default: fuzzing-differential-test/config/default.json)
  --duration TIME       Stop after a human duration such as 30s, 10m, or 1h
  --iterations N        Stop after N Fuzzilli iterations
  --jobs N              Run N Fuzzilli workers (default: 1)
  --artifact-root PATH  Shared artifact root (default: /home/user/velum-fuzzing-artifacts/differential-v8)
  --output PATH         Store this session at PATH
  --resume PATH         Resume an existing session
  --replay PATH         Re-run saved .js scripts from a prior session or directory
  --node PATH           Node/V8 executable (default: node)
  --engine-timeout TIME Set both Engine262 and V8 timeout for compatibility
  --engine262-timeout TIME Engine262 timeout such as 30s (default: 30s)
  --v8-timeout TIME     V8 timeout such as 4s (default: 4s)
  --slow-ratio N        Save cases with Velum/V8 ratio >= N (default: 2)
  --slow-min TIME       Minimum Velum time before a slow case is saved (default: 5ms)
  --stop-after-correctness-mismatches N Stop after N Engine262 mismatches (default: 10)
  --save-all            Save every generated JavaScript program
  --skip-build          Reuse existing Fuzzilli and differential target binaries
  -h, --help            Show this help
";

#[derive(Debug)]
pub struct Config {
    pub config_path: PathBuf,
    pub duration: Option<Duration>,
    pub iterations: Option<NonZeroUsize>,
    pub jobs: NonZeroUsize,
    pub artifact_root: PathBuf,
    pub output: Option<PathBuf>,
    pub resume_path: Option<PathBuf>,
    pub replay_path: Option<PathBuf>,
    pub node_binary: String,
    pub engine262_timeout: Duration,
    pub v8_timeout: Duration,
    pub slow_ratio: f64,
    pub slow_min: Duration,
    pub stop_after: StopAfter,
    pub save_all: bool,
    pub skip_build: bool,
    pub help: bool,
}

impl Config {
    /// Returns the session directory selected by output, resume, or timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error when the current timestamp cannot be read.
    pub fn session_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(path) = &self.resume_path {
            return Ok(path.clone());
        }
        if let Some(path) = &self.output {
            return Ok(path.clone());
        }
        let timestamp = unix_timestamp_millis()?;
        Ok(self.artifact_root.join(format!("session-{timestamp}")))
    }

    #[must_use]
    pub const fn resume(&self) -> bool {
        self.resume_path.is_some()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StopAfter {
    pub correctness_mismatches: Option<NonZeroUsize>,
    pub performance_slow: Option<NonZeroUsize>,
    pub v8_timeouts: Option<NonZeroUsize>,
    pub v8_crashes: Option<NonZeroUsize>,
    pub engine262_timeouts: Option<NonZeroUsize>,
    pub engine262_crashes: Option<NonZeroUsize>,
}

impl StopAfter {
    const fn disabled() -> Self {
        Self {
            correctness_mismatches: None,
            performance_slow: None,
            v8_timeouts: None,
            v8_crashes: None,
            engine262_timeouts: None,
            engine262_crashes: None,
        }
    }
}

/// Parses JSON configuration and command-line overrides.
///
/// # Errors
///
/// Returns an error when configuration values are invalid or paths are unsafe.
pub fn parse_arguments(
    args: impl Iterator<Item = String>,
    differential_dir: &Path,
) -> anyhow::Result<Config> {
    let raw_args = args.collect::<Vec<_>>();
    let config_path = find_config_path(&raw_args, differential_dir)?;
    let mut config = Config {
        config_path: config_path.clone(),
        duration: None,
        iterations: None,
        jobs: NonZeroUsize::MIN,
        artifact_root: default_artifact_root(),
        output: None,
        resume_path: None,
        replay_path: None,
        node_binary: "node".to_owned(),
        engine262_timeout: parse_duration("30s")?,
        v8_timeout: parse_duration("4s")?,
        slow_ratio: 2.0,
        slow_min: parse_duration("5ms")?,
        stop_after: StopAfter {
            correctness_mismatches: NonZeroUsize::new(10),
            ..StopAfter::disabled()
        },
        save_all: false,
        skip_build: false,
        help: false,
    };

    apply_config_file(&mut config, &config_path)?;
    apply_cli_arguments(&mut config, raw_args)?;

    ensure!(
        config.slow_ratio.is_finite() && config.slow_ratio > 0.0,
        "--slow-ratio must be a positive finite number"
    );
    ensure_absolute(&config.artifact_root, "artifact root")?;
    if let Some(path) = &config.output {
        ensure_absolute(path, "output path")?;
    }
    if let Some(path) = &config.resume_path {
        ensure_absolute(path, "resume path")?;
    }
    if let Some(path) = &config.replay_path {
        ensure_absolute(path, "replay path")?;
    }
    Ok(config)
}

fn apply_cli_arguments(config: &mut Config, raw_args: Vec<String>) -> anyhow::Result<()> {
    let mut args = raw_args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--config" => {
                let _ignored = next_value(&mut args, "--config")?;
            }
            "--duration" => {
                config.duration = Some(parse_duration(&next_value(&mut args, "--duration")?)?);
            }
            "--iterations" => {
                config.iterations = Some(parse_positive(&next_value(&mut args, "--iterations")?)?);
            }
            "--jobs" => config.jobs = parse_positive(&next_value(&mut args, "--jobs")?)?,
            "--artifact-root" => {
                config.artifact_root = PathBuf::from(next_value(&mut args, "--artifact-root")?);
            }
            "--output" => {
                ensure!(
                    config.resume_path.is_none(),
                    "--output cannot be combined with --resume"
                );
                config.output = Some(PathBuf::from(next_value(&mut args, "--output")?));
            }
            "--resume" => {
                ensure!(
                    config.output.is_none(),
                    "--resume cannot be combined with --output"
                );
                config.resume_path = Some(PathBuf::from(next_value(&mut args, "--resume")?));
            }
            "--replay" => {
                config.replay_path = Some(PathBuf::from(next_value(&mut args, "--replay")?));
            }
            "--node" => config.node_binary = next_value(&mut args, "--node")?,
            "--engine-timeout" => {
                let timeout = parse_duration(&next_value(&mut args, "--engine-timeout")?)?;
                config.engine262_timeout = timeout;
                config.v8_timeout = timeout;
            }
            "--engine262-timeout" => {
                config.engine262_timeout =
                    parse_duration(&next_value(&mut args, "--engine262-timeout")?)?;
            }
            "--v8-timeout" => {
                config.v8_timeout = parse_duration(&next_value(&mut args, "--v8-timeout")?)?;
            }
            "--slow-ratio" => {
                config.slow_ratio = next_value(&mut args, "--slow-ratio")?
                    .parse::<f64>()
                    .context("--slow-ratio must be a number")?;
            }
            "--slow-min" => {
                config.slow_min = parse_duration(&next_value(&mut args, "--slow-min")?)?;
            }
            "--stop-after-correctness-mismatches" | "--stop-after-mismatches" => {
                config.stop_after.correctness_mismatches =
                    parse_optional_positive(&next_value(&mut args, &argument)?)?;
            }
            "--save-all" => config.save_all = true,
            "--skip-build" => config.skip_build = true,
            "-h" | "--help" => config.help = true,
            _ => bail!("unknown argument '{argument}'\n\n{HELP}"),
        }
    }
    Ok(())
}

#[must_use]
pub fn stop_after_json(stop_after: StopAfter) -> serde_json::Value {
    serde_json::json!({
        "correctness_mismatches": stop_after_count(stop_after.correctness_mismatches),
        "performance_slow": stop_after_count(stop_after.performance_slow),
        "v8_timeouts": stop_after_count(stop_after.v8_timeouts),
        "v8_crashes": stop_after_count(stop_after.v8_crashes),
        "engine262_timeouts": stop_after_count(stop_after.engine262_timeouts),
        "engine262_crashes": stop_after_count(stop_after.engine262_crashes),
    })
}

fn ensure_absolute(path: &Path, label: &str) -> anyhow::Result<()> {
    ensure!(
        path.is_absolute(),
        "{label} must be absolute: {}",
        path.display()
    );
    Ok(())
}

fn stop_after_count(value: Option<NonZeroUsize>) -> usize {
    value.map_or(0, NonZeroUsize::get)
}

fn find_config_path(args: &[String], differential_dir: &Path) -> anyhow::Result<PathBuf> {
    let mut iter = args.iter();
    while let Some(argument) = iter.next() {
        if argument == "--config" {
            let value = iter.next().context("missing value after --config")?;
            let path = PathBuf::from(value);
            return Ok(if path.is_absolute() {
                path
            } else {
                env::current_dir()
                    .context("failed to read current directory")?
                    .join(path)
            });
        }
    }
    Ok(differential_dir.join(DEFAULT_CONFIG))
}

fn apply_config_file(config: &mut Config, path: &Path) -> anyhow::Result<()> {
    if !path.is_file() {
        return Ok(());
    }
    let raw =
        fs::read(path).with_context(|| format!("failed to read config '{}'", path.display()))?;
    let value: serde_json::Value = serde_json::from_slice(&raw)
        .with_context(|| format!("failed to parse config '{}'", path.display()))?;
    if let Some(duration) = optional_string(&value, "duration")? {
        config.duration = Some(parse_duration(&duration)?);
    }
    if let Some(jobs) = optional_u64(&value, "jobs")? {
        config.jobs = parse_positive(&jobs.to_string())?;
    }
    if let Some(node) = optional_string(&value, "node")? {
        config.node_binary = node;
    }
    if let Some(timeout) = optional_string(&value, "engine_timeout")? {
        let timeout = parse_duration(&timeout)?;
        config.engine262_timeout = timeout;
        config.v8_timeout = timeout;
    }
    if let Some(timeout) = optional_string(&value, "engine262_timeout")? {
        config.engine262_timeout = parse_duration(&timeout)?;
    }
    if let Some(timeout) = optional_string(&value, "v8_timeout")? {
        config.v8_timeout = parse_duration(&timeout)?;
    }
    if let Some(slow_ratio) = optional_f64(&value, "slow_ratio")? {
        config.slow_ratio = slow_ratio;
    }
    if let Some(slow_min) = optional_string(&value, "slow_min")? {
        config.slow_min = parse_duration(&slow_min)?;
    }
    if let Some(save_all) = optional_bool(&value, "save_all")? {
        config.save_all = save_all;
    }
    if let Some(stop_after) = value.get("stop_after") {
        apply_stop_after(config, stop_after)?;
    }
    Ok(())
}

fn apply_stop_after(config: &mut Config, value: &serde_json::Value) -> anyhow::Result<()> {
    if let StopAfterField::Value(value) = optional_stop_after(value, "correctness_mismatches")? {
        config.stop_after.correctness_mismatches = value;
    }
    if let StopAfterField::Value(value) = optional_stop_after(value, "performance_slow")? {
        config.stop_after.performance_slow = value;
    }
    if let StopAfterField::Value(value) = optional_stop_after(value, "v8_timeouts")? {
        config.stop_after.v8_timeouts = value;
    }
    if let StopAfterField::Value(value) = optional_stop_after(value, "v8_crashes")? {
        config.stop_after.v8_crashes = value;
    }
    if let StopAfterField::Value(value) = optional_stop_after(value, "engine262_timeouts")? {
        config.stop_after.engine262_timeouts = value;
    }
    if let StopAfterField::Value(value) = optional_stop_after(value, "engine262_crashes")? {
        config.stop_after.engine262_crashes = value;
    }
    Ok(())
}

fn default_artifact_root() -> PathBuf {
    env::var("VELUM_DIFF_ARTIFACT_ROOT")
        .map_or_else(|_| PathBuf::from(DEFAULT_ARTIFACT_ROOT), PathBuf::from)
}

fn next_value(args: &mut impl Iterator<Item = String>, option: &str) -> anyhow::Result<String> {
    args.next()
        .with_context(|| format!("missing value after {option}"))
}

fn parse_positive(value: &str) -> anyhow::Result<NonZeroUsize> {
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("expected a positive integer, got '{value}'"))?;
    NonZeroUsize::new(parsed).with_context(|| format!("expected a positive integer, got '{value}'"))
}

fn parse_optional_positive(value: &str) -> anyhow::Result<Option<NonZeroUsize>> {
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("expected a non-negative integer, got '{value}'"))?;
    Ok(NonZeroUsize::new(parsed))
}

fn optional_string(value: &serde_json::Value, key: &str) -> anyhow::Result<Option<String>> {
    let Some(value) = value.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(|value| Some(value.to_owned()))
        .with_context(|| format!("config field '{key}' must be a string or null"))
}

fn optional_u64(value: &serde_json::Value, key: &str) -> anyhow::Result<Option<u64>> {
    let Some(value) = value.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_u64()
        .map(Some)
        .with_context(|| format!("config field '{key}' must be a non-negative integer or null"))
}

fn optional_f64(value: &serde_json::Value, key: &str) -> anyhow::Result<Option<f64>> {
    let Some(value) = value.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_f64()
        .map(Some)
        .with_context(|| format!("config field '{key}' must be a number or null"))
}

fn optional_bool(value: &serde_json::Value, key: &str) -> anyhow::Result<Option<bool>> {
    let Some(value) = value.get(key) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_bool()
        .map(Some)
        .with_context(|| format!("config field '{key}' must be a boolean or null"))
}

enum StopAfterField {
    Missing,
    Value(Option<NonZeroUsize>),
}

fn optional_stop_after(value: &serde_json::Value, key: &str) -> anyhow::Result<StopAfterField> {
    let Some(value) = value.get(key) else {
        return Ok(StopAfterField::Missing);
    };
    if value.is_null() {
        return Ok(StopAfterField::Value(None));
    }
    let raw = value
        .as_u64()
        .with_context(|| format!("stop_after.{key} must be a non-negative integer or null"))?;
    let count = usize::try_from(raw).with_context(|| format!("stop_after.{key} is too large"))?;
    Ok(StopAfterField::Value(NonZeroUsize::new(count)))
}
