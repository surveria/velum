use std::{
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, bail, ensure};
use nix::{
    errno::Errno,
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use velum_differential_fuzz::{
    report::build_report,
    time::{duration_millis_u64, parse_duration, unix_timestamp_millis},
};

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
const DEFAULT_ARTIFACT_ROOT: &str = "/home/user/velum-fuzzing-artifacts/differential-v8";
const HELP: &str = r"Usage: velum-diff-fuzz [OPTIONS]

Run a local Fuzzilli differential campaign comparing Velum with V8/Node.

Options:
  --duration TIME       Stop after a human duration such as 30s, 10m, or 1h
  --iterations N        Stop after N Fuzzilli iterations
  --jobs N              Run N Fuzzilli workers (default: 1)
  --artifact-root PATH  Shared artifact root (default: /home/user/velum-fuzzing-artifacts/differential-v8)
  --output PATH         Store this session at PATH
  --resume PATH         Resume an existing session
  --node PATH           Node/V8 executable (default: node)
  --engine-timeout TIME Per-engine V8 timeout such as 4s (default: 4s)
  --slow-ratio N        Save equivalent cases with Velum/V8 ratio >= N (default: 10)
  --slow-min TIME       Minimum Velum time before a slow case is saved (default: 5ms)
  --save-all            Save every generated JavaScript program
  --skip-build          Reuse existing Fuzzilli and differential target binaries
  -h, --help            Show this help
";

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let config = parse_arguments(env::args().skip(1))?;
    if config.help {
        println!("{HELP}");
        return Ok(());
    }
    let differential_dir = differential_directory()?;
    let repo_root = repository_root(&differential_dir)?;
    if !config.skip_build {
        run_checked(
            Command::new(differential_dir.join("scripts/build.sh")).current_dir(&repo_root),
            "failed to build differential fuzzing tools",
        )?;
    }

    let fuzzilli = repo_root.join("fuzzing-test/.bin/FuzzilliCli");
    let target_triple = rust_target_triple()?;
    let target = differential_dir.join(format!(
        "driver/target/{target_triple}/release/velum-diff-target"
    ));
    ensure_file(&fuzzilli, "Fuzzilli")?;
    ensure_file(&target, "differential target")?;

    let session_dir = config.session_dir()?;
    prepare_session(&session_dir, config.resume())?;
    write_manifest(&session_dir, &repo_root, &config)?;
    install_ctrlc_handler()?;

    let fuzzilli_storage = session_dir.join("fuzzilli");
    let mut command = Command::new(&fuzzilli);
    command
        .arg("--profile=velum")
        .arg("--forDifferentialFuzzing")
        .arg(format!("--jobs={}", config.jobs))
        .arg(format!("--storagePath={}", fuzzilli_storage.display()))
        .arg("--exportStatistics")
        .arg("--statisticsExportInterval=1")
        .arg(format!(
            "--timeout={}",
            fuzzilli_timeout_millis(config.engine_timeout)
        ))
        .arg(format!(
            "--additionalArguments={}",
            additional_target_arguments(&session_dir, &config).join(",")
        ))
        .arg(format!("--tag=velum-diff-{}", git_head(&repo_root)?))
        .env("VELUM_DIFF_ARTIFACT_DIR", &session_dir);
    if config.jobs.get() > 1 {
        command.arg("--immediateWorkers");
    }
    if let Some(iterations) = config.iterations {
        command.arg(format!("--maxIterations={iterations}"));
    }
    if config.resume() {
        command.arg("--resume");
    }
    command.arg(&target);

    let pending_log = PendingLog::new(&session_dir)?;
    print_startup(&session_dir, &pending_log, &config);
    let started_at = Instant::now();
    let mut child = pending_log.spawn(command, config.duration)?;
    let status = wait_for_fuzzilli(&mut child, config.duration)?;
    let elapsed = started_at.elapsed();
    let log_path = pending_log.finalize(&session_dir)?;
    let report = build_report(&session_dir, elapsed, &status.to_string())?;
    append_summary_to_log(&log_path, &report.render())?;
    println!("{}", report.render());
    ensure!(status.success(), "Fuzzilli exited with status {status}");
    Ok(())
}

#[derive(Debug)]
struct Config {
    duration: Option<Duration>,
    iterations: Option<NonZeroUsize>,
    jobs: NonZeroUsize,
    artifact_root: PathBuf,
    output: Option<PathBuf>,
    resume_path: Option<PathBuf>,
    node_binary: String,
    engine_timeout: Duration,
    slow_ratio: f64,
    slow_min: Duration,
    save_all: bool,
    skip_build: bool,
    help: bool,
}

impl Config {
    fn session_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(path) = &self.resume_path {
            return Ok(path.clone());
        }
        if let Some(path) = &self.output {
            return Ok(path.clone());
        }
        let timestamp = unix_timestamp_millis()?;
        Ok(self.artifact_root.join(format!("session-{timestamp}")))
    }

    const fn resume(&self) -> bool {
        self.resume_path.is_some()
    }
}

fn parse_arguments(mut args: impl Iterator<Item = String>) -> anyhow::Result<Config> {
    let mut config = Config {
        duration: None,
        iterations: None,
        jobs: NonZeroUsize::MIN,
        artifact_root: default_artifact_root(),
        output: None,
        resume_path: None,
        node_binary: "node".to_owned(),
        engine_timeout: parse_duration("4s")?,
        slow_ratio: 10.0,
        slow_min: parse_duration("5ms")?,
        save_all: false,
        skip_build: false,
        help: false,
    };

    while let Some(argument) = args.next() {
        match argument.as_str() {
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
            "--node" => config.node_binary = next_value(&mut args, "--node")?,
            "--engine-timeout" => {
                config.engine_timeout =
                    parse_duration(&next_value(&mut args, "--engine-timeout")?)?;
            }
            "--slow-ratio" => {
                config.slow_ratio = next_value(&mut args, "--slow-ratio")?
                    .parse::<f64>()
                    .context("--slow-ratio must be a number")?;
            }
            "--slow-min" => {
                config.slow_min = parse_duration(&next_value(&mut args, "--slow-min")?)?;
            }
            "--save-all" => config.save_all = true,
            "--skip-build" => config.skip_build = true,
            "-h" | "--help" => config.help = true,
            _ => bail!("unknown argument '{argument}'\n\n{HELP}"),
        }
    }

    ensure!(
        config.slow_ratio.is_finite() && config.slow_ratio > 0.0,
        "--slow-ratio must be a positive finite number"
    );
    ensure!(
        config.artifact_root.is_absolute(),
        "artifact root must be absolute: {}",
        config.artifact_root.display()
    );
    if let Some(path) = &config.output {
        ensure!(
            path.is_absolute(),
            "output path must be absolute: {}",
            path.display()
        );
    }
    if let Some(path) = &config.resume_path {
        ensure!(
            path.is_absolute(),
            "resume path must be absolute: {}",
            path.display()
        );
    }
    Ok(config)
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

fn prepare_session(path: &Path, resume: bool) -> anyhow::Result<()> {
    if resume {
        ensure!(
            path.is_dir(),
            "session to resume is missing: {}",
            path.display()
        );
        return Ok(());
    }
    ensure!(
        !path.exists(),
        "differential fuzzing output path already exists: {}",
        path.display()
    );
    fs::create_dir_all(path)
        .with_context(|| format!("failed to create session directory '{}'", path.display()))
}

fn write_manifest(session_dir: &Path, repo_root: &Path, config: &Config) -> anyhow::Result<()> {
    let path = session_dir.join("manifest.json");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "repo_root": repo_root,
        "engine_commit": git_head(repo_root)?,
        "duration": config.duration.map(|value| humantime::format_duration(value).to_string()),
        "iterations": config.iterations.map(NonZeroUsize::get),
        "jobs": config.jobs.get(),
        "node_binary": config.node_binary,
        "engine_timeout": humantime::format_duration(config.engine_timeout).to_string(),
        "slow_ratio": config.slow_ratio,
        "slow_min": humantime::format_duration(config.slow_min).to_string(),
        "save_all": config.save_all,
        "resume": config.resume(),
    });
    fs::write(&path, serde_json::to_vec_pretty(&manifest)?)
        .with_context(|| format!("failed to write '{}'", path.display()))
}

struct PendingLog {
    temporary_path: PathBuf,
    final_path: PathBuf,
}

impl PendingLog {
    fn new(session_dir: &Path) -> anyhow::Result<Self> {
        let timestamp = unix_timestamp_millis()?;
        let process_id = std::process::id();
        let temporary_path =
            session_dir.join(format!(".velum-diff-fuzz-{timestamp}-{process_id}.log"));
        let final_path = session_dir.join(format!("fuzzilli-{timestamp}-{process_id}.log"));
        ensure!(
            !temporary_path.exists() && !final_path.exists(),
            "differential fuzzing log path collision"
        );
        Ok(Self {
            temporary_path,
            final_path,
        })
    }

    fn spawn(&self, mut command: Command, duration: Option<Duration>) -> anyhow::Result<Child> {
        let mut log = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&self.temporary_path)
            .with_context(|| {
                format!(
                    "failed to create temporary log '{}'",
                    self.temporary_path.display()
                )
            })?;
        writeln!(
            log,
            "Differential fuzzing duration: {}",
            duration.map_or_else(
                || "none".to_owned(),
                |value| humantime::format_duration(value).to_string()
            )
        )
        .context("failed to write differential fuzzing log header")?;
        log.flush()
            .context("failed to flush differential fuzzing log header")?;
        let stdout = log
            .try_clone()
            .context("failed to clone differential fuzzing log handle")?;
        command.stdout(Stdio::from(stdout)).stderr(Stdio::from(log));
        command
            .spawn()
            .context("failed to start Fuzzilli differential campaign")
    }

    fn finalize(&self, session_dir: &Path) -> anyhow::Result<PathBuf> {
        fs::create_dir_all(session_dir)
            .with_context(|| format!("failed to create session '{}'", session_dir.display()))?;
        fs::rename(&self.temporary_path, &self.final_path).with_context(|| {
            format!(
                "failed to move differential log from '{}' to '{}'",
                self.temporary_path.display(),
                self.final_path.display()
            )
        })?;
        Ok(self.final_path.clone())
    }
}

fn print_startup(session_dir: &Path, pending_log: &PendingLog, config: &Config) {
    println!(
        "Velum/V8 differential fuzzing session: {}",
        session_dir.display()
    );
    println!("Parallel Fuzzilli workers: {}", config.jobs);
    println!("Node/V8 executable: {}", config.node_binary);
    println!(
        "Per-engine timeout: {}",
        humantime::format_duration(config.engine_timeout)
    );
    println!(
        "Live log while running: {}",
        pending_log.temporary_path.display()
    );
    println!("Final detailed log: {}", pending_log.final_path.display());
    println!("Case JSONL: {}/cases/*.jsonl", session_dir.display());
    println!("Findings: {}/findings/**/*.js", session_dir.display());
    if let Some(duration) = config.duration {
        println!(
            "The campaign will stop after {}.",
            humantime::format_duration(duration)
        );
    } else if config.iterations.is_none() {
        println!("The campaign runs until Ctrl-C.");
    }
}

fn additional_target_arguments(session_dir: &Path, config: &Config) -> Vec<String> {
    let mut args = vec![
        "--artifact-dir".to_owned(),
        session_dir.display().to_string(),
        "--node".to_owned(),
        config.node_binary.clone(),
        "--engine-timeout".to_owned(),
        humantime::format_duration(config.engine_timeout).to_string(),
        "--slow-ratio".to_owned(),
        config.slow_ratio.to_string(),
        "--slow-min".to_owned(),
        humantime::format_duration(config.slow_min).to_string(),
    ];
    if config.save_all {
        args.push("--save-all".to_owned());
    }
    args
}

fn wait_for_fuzzilli(child: &mut Child, duration: Option<Duration>) -> anyhow::Result<ExitStatus> {
    let deadline = duration
        .map(|value| {
            Instant::now()
                .checked_add(value)
                .context("duration is too large for the monotonic clock")
        })
        .transpose()?;
    let mut shutdown_requested = false;

    loop {
        if let Some(status) = child.try_wait().context("failed to poll Fuzzilli")? {
            return Ok(status);
        }

        let manual_stop = STOP_REQUESTED.load(Ordering::SeqCst);
        let duration_expired = deadline.is_some_and(|value| Instant::now() >= value);
        if !shutdown_requested && (manual_stop || duration_expired) {
            if duration_expired && !manual_stop {
                println!("Duration limit reached; stopping Fuzzilli gracefully.");
            }
            if let Err(error) = signal_interrupt(child.id()) {
                child
                    .kill()
                    .context("failed to kill Fuzzilli after graceful shutdown failed")?;
                let status = child
                    .wait()
                    .context("failed to reap Fuzzilli after forced shutdown")?;
                bail!(
                    "failed to request graceful Fuzzilli shutdown: {error}; forced status {status}"
                );
            }
            shutdown_requested = true;
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

fn install_ctrlc_handler() -> anyhow::Result<()> {
    ctrlc::set_handler(|| {
        if !STOP_REQUESTED.swap(true, Ordering::SeqCst) {
            eprintln!("Stopping Fuzzilli; waiting for the current program and corpus save.");
        }
    })
    .context("failed to install the Ctrl-C handler")
}

fn signal_interrupt(process_id: u32) -> anyhow::Result<()> {
    let process_id = i32::try_from(process_id).context("Fuzzilli process id does not fit i32")?;
    match kill(Pid::from_raw(process_id), Signal::SIGINT) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(error).context("failed to send SIGINT to Fuzzilli"),
    }
}

fn append_summary_to_log(path: &Path, summary: &str) -> anyhow::Result<()> {
    let mut log = OpenOptions::new()
        .append(true)
        .open(path)
        .with_context(|| format!("failed to reopen log '{}'", path.display()))?;
    writeln!(
        log,
        "\n===== Velum/V8 differential summary =====\n{summary}"
    )
    .with_context(|| format!("failed to append summary to '{}'", path.display()))
}

fn fuzzilli_timeout_millis(engine_timeout: Duration) -> u64 {
    duration_millis_u64(engine_timeout)
        .saturating_mul(3)
        .saturating_add(1_000)
}

fn run_checked(command: &mut Command, context: &str) -> anyhow::Result<()> {
    let status = command.status().with_context(|| context.to_owned())?;
    ensure!(status.success(), "{context}: process exited with {status}");
    Ok(())
}

fn ensure_file(path: &Path, label: &str) -> anyhow::Result<()> {
    ensure!(
        path.is_file(),
        "{label} binary is missing: {}",
        path.display()
    );
    Ok(())
}

fn rust_target_triple() -> anyhow::Result<String> {
    let output = Command::new("rustc")
        .args(["+nightly", "--version", "--verbose"])
        .output()
        .context("failed to query the Rust nightly target")?;
    ensure!(
        output.status.success(),
        "Rust nightly target query failed with {}",
        output.status
    );
    let stdout = String::from_utf8(output.stdout).context("rustc output is not UTF-8")?;
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .context("rustc did not report its host target")
}

fn differential_directory() -> anyhow::Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("driver manifest has no parent directory")
}

fn repository_root(differential_dir: &Path) -> anyhow::Result<PathBuf> {
    differential_dir
        .parent()
        .map(Path::to_path_buf)
        .context("differential directory has no repository parent")
}

fn git_head(repo_root: &Path) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root)
        .output()
        .context("failed to query the Velum commit")?;
    ensure!(
        output.status.success(),
        "git rev-parse failed with {}",
        output.status
    );
    let commit = String::from_utf8(output.stdout).context("git output is not UTF-8")?;
    Ok(commit.trim().to_owned())
}
