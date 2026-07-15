use std::{
    env, fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, bail, ensure};

const HELP: &str = "\
Usage: velum-fuzz [OPTIONS]\n\
       velum-fuzz --reproduce FILE [--skip-build]\n\
\n\
Run a local Fuzzilli security campaign against the instrumented Velum target.\n\
The campaign runs until Ctrl-C unless --iterations is provided.\n\
\n\
Options:\n\
  --iterations N   Stop after N Fuzzilli iterations\n\
  --jobs N         Run N Fuzzilli workers (default: 1)\n\
  --output PATH    Store this session at PATH\n\
  --diagnostics    Also retain invalid programs and timeouts\n\
  --reproduce FILE Run one saved JavaScript reproducer\n\
  --skip-build     Reuse existing Fuzzilli and target binaries\n\
  -h, --help       Show this help\n";

static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
struct Config {
    iterations: Option<NonZeroUsize>,
    jobs: NonZeroUsize,
    output: Option<PathBuf>,
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
    let mut iterations = None;
    let mut jobs = NonZeroUsize::MIN;
    let mut output = None;
    let mut diagnostics = false;
    let mut skip_build = false;
    let mut reproduce = None;
    let mut run_only_option_used = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--iterations" => {
                iterations = Some(parse_positive(&next_value(&mut args, "--iterations")?)?);
                run_only_option_used = true;
            }
            "--jobs" => {
                jobs = parse_positive(&next_value(&mut args, "--jobs")?)?;
                run_only_option_used = true;
            }
            "--output" => {
                output = Some(PathBuf::from(next_value(&mut args, "--output")?));
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
        iterations,
        jobs,
        output,
        diagnostics,
        skip_build,
    }))
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
    let fuzzilli = fuzzing_dir.join("fuzzilli/.build/release/FuzzilliCli");
    let velum_target = fuzzing_dir.join(format!(
        "velum-reprl/target/{target_triple}/release/velum-fuzzilli"
    ));
    ensure_file(&fuzzilli, "Fuzzilli")?;
    ensure_file(&velum_target, "Velum Fuzzilli target")?;

    let run_dir = match &config.output {
        Some(path) => path.clone(),
        None => default_run_directory(&fuzzing_dir)?,
    };
    ensure!(
        !run_dir.exists(),
        "fuzzing output path already exists: {}",
        run_dir.display()
    );
    if let Some(parent) = run_dir.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output parent '{}'", parent.display()))?;
    }

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
        .arg(format!("--tag=velum-{engine_commit}"));
    if let Some(iterations) = config.iterations {
        command.arg(format!("--maxIterations={iterations}"));
    }
    if config.diagnostics {
        command.arg("--diagnostics");
    }
    command.arg(&velum_target).current_dir(&repo_root);

    println!("Velum Fuzzilli session: {}", run_dir.display());
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
    if config.iterations.is_none() {
        println!("The campaign runs until Ctrl-C.");
    }

    let status = command
        .status()
        .with_context(|| format!("failed to start Fuzzilli '{}'", fuzzilli.display()))?;
    summarize_results(&run_dir, config.diagnostics)?;
    ensure!(status.success(), "Fuzzilli exited with status {status}");
    Ok(())
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
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    Ok(fuzzing_dir
        .join("runs")
        .join(format!("session-{timestamp}")))
}

fn ensure_file(path: &Path, label: &str) -> anyhow::Result<()> {
    ensure!(path.is_file(), "{label} is missing: {}", path.display());
    Ok(())
}

fn summarize_results(run_dir: &Path, diagnostics: bool) -> anyhow::Result<()> {
    let crashes = count_javascript_files(&run_dir.join("crashes"))?;
    let duplicate_crashes = count_javascript_files(&run_dir.join("crashes/duplicates"))?;
    println!("Fuzzilli stopped. Unique crashes: {crashes}; duplicates: {duplicate_crashes}.");
    if diagnostics {
        let timeouts = count_javascript_files(&run_dir.join("timeouts"))?;
        let rejected = count_javascript_files(&run_dir.join("failed"))?;
        println!("Recorded timeouts: {timeouts}; rejected JavaScript samples: {rejected}.");
    }
    println!("Session data: {}", run_dir.display());
    Ok(())
}

fn count_javascript_files(directory: &Path) -> anyhow::Result<usize> {
    if !directory.is_dir() {
        return Ok(0);
    }
    let mut count = 0_usize;
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read '{}'", directory.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read '{}'", directory.display()))?;
        if entry.path().extension().and_then(|value| value.to_str()) == Some("js") {
            count = count
                .checked_add(1)
                .context("JavaScript file count overflow")?;
        }
    }
    Ok(count)
}
