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
    artifacts::{ArtifactRecorder, TargetConfig},
    compare::CompareConfig,
    diff_config::{Config, HELP, StopAfter, parse_arguments, stop_after_json},
    report::build_report,
    time::{duration_millis_u64, unix_timestamp_millis},
};

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let differential_dir = differential_directory()?;
    let config = parse_arguments(env::args().skip(1), &differential_dir)?;
    if config.help {
        println!("{HELP}");
        return Ok(());
    }
    let repo_root = repository_root(&differential_dir)?;
    if !config.skip_build {
        run_checked(
            Command::new(differential_dir.join("scripts/build.sh")).current_dir(&repo_root),
            "failed to build differential fuzzing tools",
        )?;
    }

    let session_dir = config.session_dir()?;
    prepare_session(&session_dir, config.resume())?;
    write_manifest(&session_dir, &repo_root, &differential_dir, &config)?;
    install_ctrlc_handler()?;

    if let Some(replay_path) = &config.replay_path {
        return run_replay(&session_dir, replay_path, &differential_dir, &config);
    }

    let fuzzilli = repo_root.join("fuzzing-test/.bin/FuzzilliCli");
    let target_triple = rust_target_triple()?;
    let target = differential_dir.join(format!(
        "driver/target/{target_triple}/release/velum-diff-target"
    ));
    ensure_file(&fuzzilli, "Fuzzilli")?;
    ensure_file(&target, "differential target")?;

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
            fuzzilli_timeout_millis(config.engine262_timeout)
        ))
        .arg(format!(
            "--additionalArguments={}",
            additional_target_arguments(&session_dir, &differential_dir, &config).join(",")
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
    let status = wait_for_fuzzilli(&mut child, &session_dir, config.duration, config.stop_after)?;
    let elapsed = started_at.elapsed();
    let log_path = pending_log.finalize(&session_dir)?;
    let report = build_report(&session_dir, elapsed, &status.to_string())?;
    append_summary_to_log(&log_path, &report.render())?;
    println!("{}", report.render());
    ensure!(status.success(), "Fuzzilli exited with status {status}");
    Ok(())
}

fn run_replay(
    session_dir: &Path,
    replay_path: &Path,
    differential_dir: &Path,
    config: &Config,
) -> anyhow::Result<()> {
    ensure!(
        replay_path.is_dir() || replay_path.is_file(),
        "replay path is missing: {}",
        replay_path.display()
    );
    let mut scripts = Vec::new();
    if replay_path.is_file() {
        scripts.push(replay_path.to_path_buf());
    } else {
        collect_javascript_files(replay_path, &mut scripts)?;
    }
    ensure!(
        !scripts.is_empty(),
        "replay path contains no JavaScript files: {}",
        replay_path.display()
    );
    println!(
        "Replaying {} JavaScript scripts into {}",
        scripts.len(),
        session_dir.display()
    );
    let started_at = Instant::now();
    let mut recorder = ArtifactRecorder::new(TargetConfig {
        artifact_dir: session_dir.to_path_buf(),
        node_binary: config.node_binary.clone(),
        engine262_package_dir: differential_dir.to_path_buf(),
        compare: CompareConfig {
            engine262_timeout: config.engine262_timeout,
            v8_timeout: config.v8_timeout,
            slow_ratio: config.slow_ratio,
            slow_min: config.slow_min,
        },
        save_all: config.save_all,
    })?;
    let mut outcome = "replay completed".to_owned();
    for script in scripts {
        if STOP_REQUESTED.load(Ordering::SeqCst) {
            "replay stopped by Ctrl-C".clone_into(&mut outcome);
            break;
        }
        let bytes = fs::read(&script)
            .with_context(|| format!("failed to read replay script '{}'", script.display()))?;
        recorder
            .record(&bytes)
            .with_context(|| format!("failed to replay script '{}'", script.display()))?;
        if let Some(reason) = stop_after_reason(session_dir, config.stop_after)? {
            outcome = reason;
            break;
        }
    }
    let elapsed = started_at.elapsed();
    let report = build_report(session_dir, elapsed, &outcome)?;
    println!("{}", report.render());
    Ok(())
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

fn write_manifest(
    session_dir: &Path,
    repo_root: &Path,
    differential_dir: &Path,
    config: &Config,
) -> anyhow::Result<()> {
    let path = session_dir.join("manifest.json");
    let manifest = serde_json::json!({
        "schema_version": 1,
        "repo_root": repo_root,
        "differential_dir": differential_dir,
        "config_path": &config.config_path,
        "engine_commit": git_head(repo_root)?,
        "mode": if config.replay_path.is_some() { "replay" } else { "fuzzilli" },
        "duration": config.duration.map(|value| humantime::format_duration(value).to_string()),
        "iterations": config.iterations.map(NonZeroUsize::get),
        "jobs": config.jobs.get(),
        "replay_path": &config.replay_path,
        "node_binary": &config.node_binary,
        "engine262_timeout": humantime::format_duration(config.engine262_timeout).to_string(),
        "v8_timeout": humantime::format_duration(config.v8_timeout).to_string(),
        "slow_ratio": config.slow_ratio,
        "slow_min": humantime::format_duration(config.slow_min).to_string(),
        "stop_after": stop_after_json(config.stop_after),
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
        "Velum/Engine262/V8 differential session: {}",
        session_dir.display()
    );
    println!("Config: {}", config.config_path.display());
    println!("Parallel Fuzzilli workers: {}", config.jobs);
    println!("Node executable: {}", config.node_binary);
    println!(
        "Engine262 timeout: {}",
        humantime::format_duration(config.engine262_timeout)
    );
    println!(
        "V8 timeout: {}",
        humantime::format_duration(config.v8_timeout)
    );
    if let Some(limit) = config.stop_after.correctness_mismatches {
        println!("Stop after correctness mismatches: {}", limit.get());
    }
    if let Some(replay_path) = &config.replay_path {
        println!("Replay source: {}", replay_path.display());
    }
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
        println!("The campaign runs until a stop criterion is reached or Ctrl-C is pressed.");
    }
}

fn additional_target_arguments(
    session_dir: &Path,
    differential_dir: &Path,
    config: &Config,
) -> Vec<String> {
    let mut args = vec![
        "--artifact-dir".to_owned(),
        session_dir.display().to_string(),
        "--node".to_owned(),
        config.node_binary.clone(),
        "--engine262-package-dir".to_owned(),
        differential_dir.display().to_string(),
        "--engine262-timeout".to_owned(),
        humantime::format_duration(config.engine262_timeout).to_string(),
        "--v8-timeout".to_owned(),
        humantime::format_duration(config.v8_timeout).to_string(),
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

fn wait_for_fuzzilli(
    child: &mut Child,
    session_dir: &Path,
    duration: Option<Duration>,
    stop_after: StopAfter,
) -> anyhow::Result<ExitStatus> {
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
        let finding_limit = stop_after_reason(session_dir, stop_after)?;
        if !shutdown_requested && (manual_stop || duration_expired || finding_limit.is_some()) {
            if duration_expired && !manual_stop {
                println!("Duration limit reached; stopping Fuzzilli gracefully.");
            }
            if let Some(reason) = finding_limit {
                println!("{reason}; stopping Fuzzilli gracefully.");
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

fn stop_after_reason(session_dir: &Path, stop_after: StopAfter) -> anyhow::Result<Option<String>> {
    for (limit, relative, label) in [
        (
            stop_after.correctness_mismatches,
            "findings/correctness-mismatches",
            "correctness mismatches",
        ),
        (
            stop_after.performance_slow,
            "findings/performance-slow",
            "performance slow cases",
        ),
        (
            stop_after.v8_timeouts,
            "findings/v8-timeouts",
            "V8 timeouts",
        ),
        (stop_after.v8_crashes, "findings/v8-crashes", "V8 crashes"),
        (
            stop_after.engine262_timeouts,
            "findings/engine262-timeouts",
            "Engine262 timeouts",
        ),
        (
            stop_after.engine262_crashes,
            "findings/engine262-crashes",
            "Engine262 crashes",
        ),
    ] {
        let Some(limit) = limit else {
            continue;
        };
        let count = count_javascript_files(&session_dir.join(relative))?;
        if count >= limit.get() {
            return Ok(Some(format!(
                "Stop criterion reached: {count} {label} >= {}",
                limit.get()
            )));
        }
    }
    Ok(None)
}

fn count_javascript_files(directory: &Path) -> anyhow::Result<usize> {
    let mut files = Vec::new();
    collect_javascript_files(directory, &mut files)?;
    Ok(files.len())
}

fn collect_javascript_files(directory: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read '{}'", directory.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read '{}'", directory.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_javascript_files(&path, files)?;
        } else if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("js")
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(())
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
