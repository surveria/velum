use std::{
    env,
    fs::{self, OpenOptions},
    io::Write as _,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, bail, ensure};
use nix::{
    errno::Errno,
    sys::signal::{Signal, kill},
    unistd::Pid,
};

use velum_fuzz_driver::report::{SessionSnapshot, build_report};

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);

const HELP: &str = r"Usage: velum-fuzz [OPTIONS]
       velum-fuzz --reproduce FILE [--skip-build]

Run a local Fuzzilli security campaign against the instrumented Velum target.
The campaign runs until Ctrl-C unless --duration or --iterations is provided.

Options:
  --duration TIME  Stop after a human duration such as 30s, 2m, or 1h
  --iterations N   Stop after N Fuzzilli iterations
  --jobs N         Run N Fuzzilli workers (default: 1)
  --output PATH    Store this session at PATH
  --resume PATH    Resume the corpus from an existing session
  --diagnostics    Also retain invalid programs and timeouts
  --reproduce FILE Run one saved JavaScript reproducer
  --skip-build     Reuse existing Fuzzilli and target binaries
  -h, --help       Show this help
";

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
struct Config {
    duration: Option<Duration>,
    iterations: Option<NonZeroUsize>,
    jobs: NonZeroUsize,
    output: Option<PathBuf>,
    resume: bool,
    diagnostics: bool,
    skip_build: bool,
}

enum Invocation {
    Run(Config),
    Reproduce { path: PathBuf, skip_build: bool },
    Help,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let invocation = parse_arguments(env::args().skip(1))?;
    match invocation {
        Invocation::Run(config) => run_campaign(&config),
        Invocation::Reproduce { path, skip_build } => reproduce(&path, skip_build),
        Invocation::Help => {
            println!("{HELP}");
            Ok(())
        }
    }
}

fn parse_arguments(mut args: impl Iterator<Item = String>) -> anyhow::Result<Invocation> {
    let mut duration = None;
    let mut iterations = None;
    let mut jobs = NonZeroUsize::MIN;
    let mut output = None;
    let mut resume = false;
    let mut diagnostics = false;
    let mut skip_build = false;
    let mut reproduce = None;
    let mut run_only_option_used = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--duration" => {
                duration = Some(parse_duration(&next_value(&mut args, "--duration")?)?);
                run_only_option_used = true;
            }
            "--iterations" => {
                iterations = Some(parse_positive(&next_value(&mut args, "--iterations")?)?);
                run_only_option_used = true;
            }
            "--jobs" => {
                jobs = parse_positive(&next_value(&mut args, "--jobs")?)?;
                run_only_option_used = true;
            }
            "--output" => {
                ensure!(output.is_none(), "output path may only be specified once");
                output = Some(PathBuf::from(next_value(&mut args, "--output")?));
                run_only_option_used = true;
            }
            "--resume" => {
                ensure!(
                    output.is_none(),
                    "--resume cannot be combined with --output"
                );
                output = Some(PathBuf::from(next_value(&mut args, "--resume")?));
                resume = true;
                run_only_option_used = true;
            }
            "--diagnostics" => {
                diagnostics = true;
                run_only_option_used = true;
            }
            "--reproduce" => {
                ensure!(
                    reproduce.is_none(),
                    "--reproduce may only be specified once"
                );
                reproduce = Some(PathBuf::from(next_value(&mut args, "--reproduce")?));
            }
            "--skip-build" => skip_build = true,
            "-h" | "--help" => return Ok(Invocation::Help),
            _ => bail!("unknown argument '{argument}'\n\n{HELP}"),
        }
    }

    if let Some(path) = reproduce {
        ensure!(
            !run_only_option_used,
            "--reproduce cannot be combined with campaign options"
        );
        return Ok(Invocation::Reproduce { path, skip_build });
    }

    Ok(Invocation::Run(Config {
        duration,
        iterations,
        jobs,
        output,
        resume,
        diagnostics,
        skip_build,
    }))
}

fn parse_duration(value: &str) -> anyhow::Result<Duration> {
    let duration = humantime::parse_duration(value)
        .with_context(|| format!("invalid duration '{value}'; examples: 30s, 2m, 1h"))?;
    ensure!(!duration.is_zero(), "duration must be greater than zero");
    Ok(duration)
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

fn run_campaign(config: &Config) -> anyhow::Result<()> {
    let fuzzing_dir = fuzzing_directory()?;
    let repo_root = repository_root(&fuzzing_dir)?;

    if !config.skip_build {
        run_checked(
            Command::new(fuzzing_dir.join("scripts/build.sh")).current_dir(&repo_root),
            "failed to build Fuzzilli and the Velum target",
        )?;
    }

    let target_triple = rust_target_triple()?;
    let fuzzilli = fuzzing_dir.join(".bin/FuzzilliCli");
    let velum_target = fuzzing_dir.join(format!(
        "velum-reprl/target/{target_triple}/release/velum-fuzzilli"
    ));
    ensure_file(&fuzzilli, "Fuzzilli")?;
    ensure_file(&velum_target, "Velum Fuzzilli target")?;

    let run_dir = match &config.output {
        Some(path) => path.clone(),
        None => default_run_directory(&fuzzing_dir)?,
    };
    if config.resume {
        ensure!(
            run_dir.is_dir(),
            "fuzzing session to resume is missing: {}",
            run_dir.display()
        );
    } else {
        ensure!(
            !run_dir.exists(),
            "fuzzing output path already exists: {}",
            run_dir.display()
        );
        if let Some(parent) = run_dir.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output parent '{}'", parent.display())
            })?;
        }
    }
    let before = SessionSnapshot::capture(&run_dir)?;

    ctrlc::set_handler(|| {
        if !STOP_REQUESTED.swap(true, Ordering::SeqCst) {
            eprintln!("Stopping Fuzzilli; waiting for the current program and corpus save.");
        }
    })
    .context("failed to install the Ctrl-C handler")?;

    let engine_commit = git_head(&repo_root)?;
    let mut command = Command::new(&fuzzilli);
    command
        .arg("--profile=velum")
        .arg(format!("--jobs={}", config.jobs))
        .arg(format!("--storagePath={}", run_dir.display()))
        .arg("--exportStatistics")
        .arg("--statisticsExportInterval=1")
        .arg(format!("--tag=velum-{engine_commit}"));
    if let Some(iterations) = config.iterations {
        command.arg(format!("--maxIterations={iterations}"));
    }
    if config.diagnostics {
        command.arg("--diagnostics");
    }
    if config.resume {
        command.arg("--resume");
    }
    command.arg(&velum_target);

    let pending_log = PendingLog::new(&run_dir)?;

    println!("Velum Fuzzilli session: {}", run_dir.display());
    println!(
        "Live log while running: {}",
        pending_log.temporary_path().display()
    );
    println!("Final detailed log: {}", pending_log.final_path().display());
    println!(
        "Unique crash reproducers: {}/crashes/*.js",
        run_dir.display()
    );
    if config.diagnostics {
        println!("Timeout reproducers: {}/timeouts/*.js", run_dir.display());
        println!(
            "Rejected JavaScript samples: {}/failed/*.js",
            run_dir.display()
        );
    }
    if let Some(duration) = config.duration {
        println!(
            "The campaign will stop after {}.",
            humantime::format_duration(duration)
        );
    } else if config.iterations.is_none() {
        println!("The campaign runs until Ctrl-C.");
    }

    let started_at = Instant::now();
    let mut child = pending_log.spawn(command, &run_dir, config.duration)?;
    let status = wait_for_fuzzilli(&mut child, config.duration)?;
    let elapsed = started_at.elapsed();
    let log_path = pending_log.finalize(&run_dir)?;
    let report = build_report(&run_dir, &before, elapsed, &status.to_string(), &log_path)?;
    println!("{}", report.render());
    report.append_to_log()?;
    ensure!(status.success(), "Fuzzilli exited with status {status}");
    Ok(())
}

struct PendingLog {
    temporary_path: PathBuf,
    final_path: PathBuf,
}

impl PendingLog {
    fn new(run_dir: &Path) -> anyhow::Result<Self> {
        let parent = run_dir
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create log directory '{}'", parent.display()))?;
        let timestamp = unix_timestamp_millis()?;
        let process_id = std::process::id();
        let temporary_path = parent.join(format!(".velum-fuzz-{timestamp}-{process_id}.log"));
        let final_path = run_dir.join(format!("fuzzilli-{timestamp}-{process_id}.log"));
        ensure!(
            !temporary_path.exists() && !final_path.exists(),
            "fuzzing log path collision"
        );
        Ok(Self {
            temporary_path,
            final_path,
        })
    }

    fn final_path(&self) -> &Path {
        &self.final_path
    }

    fn temporary_path(&self) -> &Path {
        &self.temporary_path
    }

    fn spawn(
        &self,
        mut command: Command,
        run_dir: &Path,
        duration: Option<Duration>,
    ) -> anyhow::Result<Child> {
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
        writeln!(log, "Velum Fuzzilli session: {}", run_dir.display())
            .context("failed to write the fuzzing log header")?;
        writeln!(
            log,
            "Duration limit: {}",
            duration.map_or_else(
                || "none".to_owned(),
                |value| humantime::format_duration(value).to_string()
            )
        )
        .context("failed to write the fuzzing duration")?;
        log.flush()
            .context("failed to flush the fuzzing log header")?;
        let stdout = log
            .try_clone()
            .context("failed to clone the fuzzing log handle")?;
        command.stdout(Stdio::from(stdout)).stderr(Stdio::from(log));
        match command.spawn() {
            Ok(child) => Ok(child),
            Err(error) => {
                drop(command);
                fs::remove_file(&self.temporary_path).with_context(|| {
                    format!(
                        "failed to remove temporary log '{}' after spawn error: {error}",
                        self.temporary_path.display()
                    )
                })?;
                Err(error).context("failed to start Fuzzilli")
            }
        }
    }

    fn finalize(&self, run_dir: &Path) -> anyhow::Result<PathBuf> {
        fs::create_dir_all(run_dir)
            .with_context(|| format!("failed to create fuzzing session '{}'", run_dir.display()))?;
        fs::rename(&self.temporary_path, &self.final_path).with_context(|| {
            format!(
                "failed to move fuzzing log from '{}' to '{}'",
                self.temporary_path.display(),
                self.final_path.display()
            )
        })?;
        Ok(self.final_path.clone())
    }
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

fn signal_interrupt(process_id: u32) -> anyhow::Result<()> {
    let process_id = i32::try_from(process_id).context("Fuzzilli process id does not fit i32")?;
    match kill(Pid::from_raw(process_id), Signal::SIGINT) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(error).context("failed to send SIGINT to Fuzzilli"),
    }
}

fn reproduce(path: &Path, skip_build: bool) -> anyhow::Result<()> {
    ensure!(path.is_file(), "reproducer is missing: {}", path.display());
    let reproducer = path
        .canonicalize()
        .with_context(|| format!("failed to resolve reproducer '{}'", path.display()))?;
    let fuzzing_dir = fuzzing_directory()?;
    let repo_root = repository_root(&fuzzing_dir)?;

    if !skip_build {
        run_checked(
            Command::new(fuzzing_dir.join("scripts/build.sh")).current_dir(&repo_root),
            "failed to build Fuzzilli and the Velum target",
        )?;
    }

    let target_triple = rust_target_triple()?;
    let velum_target = fuzzing_dir.join(format!(
        "velum-reprl/target/{target_triple}/release/velum-fuzzilli"
    ));
    ensure_file(&velum_target, "Velum Fuzzilli target")?;
    println!("Reproducing: {}", reproducer.display());
    run_checked(
        Command::new(&velum_target)
            .arg("--file")
            .arg(&reproducer)
            .current_dir(repo_root),
        "Velum reproducer failed",
    )
}

fn fuzzing_directory() -> anyhow::Result<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("fuzz driver manifest has no parent directory")
}

fn repository_root(fuzzing_dir: &Path) -> anyhow::Result<PathBuf> {
    fuzzing_dir
        .parent()
        .map(Path::to_path_buf)
        .context("fuzzing directory has no repository parent")
}

fn run_checked(command: &mut Command, context: &str) -> anyhow::Result<()> {
    let status = command.status().with_context(|| context.to_owned())?;
    ensure!(status.success(), "{context}: process exited with {status}");
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

fn default_run_directory(fuzzing_dir: &Path) -> anyhow::Result<PathBuf> {
    let timestamp = unix_timestamp_millis()?;
    Ok(fuzzing_dir
        .join("runs")
        .join(format!("session-{timestamp}")))
}

fn unix_timestamp_millis() -> anyhow::Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_millis())
}

fn ensure_file(path: &Path, label: &str) -> anyhow::Result<()> {
    ensure!(path.is_file(), "{label} is missing: {}", path.display());
    Ok(())
}
