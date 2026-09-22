//! Linux-only orchestration fixtures; no real compiler or benchmark is invoked.
#![cfg(target_os = "linux")]

use std::{
    env,
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const LAUNCHER: &str = include_str!("../scripts/run-pgo-experiment.sh");
const MOCK_TOOL: &str = include_str!("fixtures/pgo/mock-toolchain.sh");
const HOST: &str = "x86_64-unknown-linux-gnu";
const TEST_TIMEOUT: Duration = Duration::from_secs(40);
const SIGNAL_TIMEOUT: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const SUFFIXES: [&str; 6] = [
    "object_transform",
    "method_dispatch",
    "json_ingestion",
    "string_processing",
    "collection_index",
    "tree_allocation",
];

struct Fixture {
    root: PathBuf,
    checkout: PathBuf,
    artifacts: PathBuf,
    tools: PathBuf,
    system_path: OsString,
}

#[test]
fn pgo_help_and_invalid_arguments_do_not_invoke_compiler_tools() -> TestResult {
    with_fixture(|fixture| {
        let output = fixture.launch(&["--help"], "success")?;
        success(&output, "PGO help")?;
        require(
            String::from_utf8(output.stdout)?.contains("--preset release|thin-lto"),
            "help omitted presets",
        )?;
        let cases: &[(&[&str], &str)] = &[
            (&[], "explicit --execute"),
            (&["--execute", "--preset", "unknown"], "--preset must"),
            (&["--execute", "--rounds", "0"], "--rounds must"),
            (&["--execute", "--cpu", "invalid"], "--cpu must"),
            (&["--execute", "--artifact-root", "relative"], "absolute"),
            (&["--execute", "--preset"], "missing value"),
            (&["--execute", "--unknown"], "unknown argument"),
        ];
        for (arguments, reason) in cases {
            rejected(&fixture.launch(arguments, "success")?, reason)?;
        }
        require(
            !fixture.root.join("events.tsv").exists(),
            "preflight invoked compiler tools",
        )?;
        require(
            !fixture.artifacts.exists(),
            "early rejection created artifacts",
        )
    })
}

#[test]
fn pgo_rejects_inherited_build_flags_and_ordinary_ci() -> TestResult {
    with_fixture(|fixture| {
        for variable in [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_TARGET",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "RUSTC_BOOTSTRAP",
            "CARGO_PROFILE_RELEASE_LTO",
        ] {
            let mut command = fixture.launcher(&["--execute"], "success")?;
            command.env(variable, "unexpected");
            rejected(
                &bounded_output(command.spawn()?, TEST_TIMEOUT)?,
                &format!("unset {variable}"),
            )?;
        }
        let mut command = fixture.launcher(&["--execute"], "success")?;
        command.env("GITHUB_ACTIONS", "true");
        rejected(
            &bounded_output(command.spawn()?, TEST_TIMEOUT)?,
            "ordinary CI",
        )?;
        require(
            !fixture.root.join("events.tsv").exists(),
            "inherited flags reached compiler tools",
        )
    })
}

#[test]
fn pgo_rejects_dirty_checkout_before_toolchain_selection() -> TestResult {
    with_fixture(|fixture| {
        fs::write(fixture.checkout.join("workload.txt"), "uncommitted edit\n")?;
        rejected(
            &fixture.launch(&["--execute"], "success")?,
            "clean committed checkout",
        )?;
        require(
            !fixture.root.join("events.tsv").exists(),
            "dirty source reached compiler tools",
        )?;
        require(
            fs::read_dir(&fixture.artifacts)?.next().is_none(),
            "dirty checkout created a run",
        )
    })
}

#[test]
fn pgo_git_overrides_cannot_hide_a_dirty_requested_checkout() -> TestResult {
    with_fixture(|fixture| {
        let pristine = fixture.root.join("pristine");
        success(
            &fixture.command("git").args(["clone", "--quiet", "--no-local"])
                .arg(&fixture.checkout).arg(&pristine).output()?,
            "create pristine local fixture",
        )?;
        let sentinel = fixture.root.join("external-index");
        fs::write(&sentinel, "external index must remain untouched\n")?;
        fs::write(fixture.checkout.join("workload.txt"), "uncommitted edit\n")?;
        let work_tree = pristine.to_string_lossy().into_owned();
        let overrides = [
            vec![("GIT_WORK_TREE", work_tree.clone())],
            vec![("GIT_DIR", pristine.join(".git").to_string_lossy().into_owned()), ("GIT_WORK_TREE", work_tree.clone())],
            vec![("GIT_COMMON_DIR", pristine.join(".git").to_string_lossy().into_owned())],
            vec![("GIT_INDEX_FILE", sentinel.to_string_lossy().into_owned())],
            vec![("GIT_OBJECT_DIRECTORY", pristine.join(".git/objects").to_string_lossy().into_owned())],
            vec![("GIT_CONFIG_COUNT", "1".to_owned()), ("GIT_CONFIG_KEY_0", "core.worktree".to_owned()), ("GIT_CONFIG_VALUE_0", work_tree.clone())],
            vec![("GIT_CONFIG_PARAMETERS", format!("'core.worktree'='{work_tree}'"))],
            vec![("GIT_PREFIX", "outside/".to_owned())],
        ];
        for values in overrides {
            let mut command = fixture.launcher(&["--execute"], "success")?;
            command.envs(values);
            rejected(&bounded_output(command.spawn()?, TEST_TIMEOUT)?, "clean committed checkout")?;
        }
        require(!fixture.root.join("events.tsv").exists(), "Git overrides reached compiler tools")?;
        require(fs::read_to_string(sentinel)? == "external index must remain untouched\n", "inherited index was modified")
    })
}

#[test]
fn pgo_fake_full_run_preserves_ab_ba_training_and_owned_artifacts() -> TestResult {
    with_fixture(|fixture| {
        success(
            &fixture.launch(&["--execute", "--rounds", "2"], "success")?,
            "synthetic PGO experiment",
        )?;
        let run = fixture.run_directory()?;
        let result = fs::read_to_string(run.join("result.txt"))?;
        require(
            result.contains("status=complete-needs-review\nexit_code=0\n"),
            "wrong completion status",
        )?;
        require(
            run.join("mock-summary.txt").is_file(),
            "summary was not reached",
        )?;
        check_stage_codes(&run, None)?;
        check_lane_order(&fixture.events()?, 2)?;
        check_build_order(&fixture.events()?, "release")?;
        check_preserved_artifacts(fixture, &run)?;
        require(
            fs::read_to_string(run.join("provenance.txt"))?.contains("preset=release\n"),
            "release preset missing",
        )?;
        require(
            fs::read_dir(run.join("raw"))?.count() == SUFFIXES.len(),
            "raw profile count changed during holdouts",
        )?;
        fs::remove_dir_all(&fixture.checkout)?;
        let output = fixture
            .command(run.join("bin/ordinary"))
            .env("MOCK_PGO_ROOT", &fixture.root)
            .arg("--pgo-summary")
            .arg(&run)
            .output()?;
        success(&output, "saved fake runner after checkout deletion")
    })
}

#[test]
fn pgo_thin_lto_preset_is_identical_for_all_three_fake_builds() -> TestResult {
    with_fixture(|fixture| {
        success(
            &fixture.launch(
                &["--execute", "--preset", "thin-lto", "--rounds", "1"],
                "success",
            )?,
            "synthetic ThinLTO experiment",
        )?;
        check_build_order(&fixture.events()?, "thin-lto")?;
        check_lane_order(&fixture.events()?, 1)?;
        let run = fixture.run_directory()?;
        require(
            fs::read_to_string(run.join("provenance.txt"))?.contains("preset=thin-lto\n"),
            "ThinLTO preset missing",
        )?;
        check_stage_codes(&run, None)
    })
}

#[test]
fn pgo_preserves_build_exit_seven_and_does_not_start_training() -> TestResult {
    with_fixture(|fixture| {
        rejected(
            &fixture.launch(&["--execute"], "build_failure")?,
            "ordinary-build failed with status 7",
        )?;
        let run = fixture.run_directory()?;
        check_stage_codes(&run, Some(("ordinary-build", "7")))?;
        check_failure_artifacts(fixture, &run, "partial synthetic build", "2")
    })
}

#[test]
fn pgo_preserves_injected_timeout_status_without_waiting_for_real_limits() -> TestResult {
    with_fixture(|fixture| {
        rejected(
            &fixture.launch(&["--execute"], "timeout")?,
            "ordinary-build failed with status 125",
        )?;
        let run = fixture.run_directory()?;
        check_stage_codes(&run, Some(("ordinary-build", "125")))?;
        check_failure_artifacts(fixture, &run, "partial synthetic timeout", "2")
    })
}

#[test]
fn pgo_actual_launcher_sigterm_stops_and_reaps_fake_compiler_child() -> TestResult {
    with_fixture(|fixture| {
        let child = fixture.launcher(&["--execute"], "signal")?.spawn()?;
        let pid = child.id();
        let ready = fixture.root.join("ready.txt");
        let started = Instant::now();
        while !ready.is_file() && started.elapsed() < TEST_TIMEOUT {
            thread::sleep(POLL_INTERVAL);
        }
        if !ready.is_file() {
            let output = bounded_output(child, SIGNAL_TIMEOUT)?;
            return Err(format!("fake compiler did not become ready: {}", output.status).into());
        }
        send_signal(pid, "TERM")?;
        let output = bounded_output(child, SIGNAL_TIMEOUT)?;
        require(
            output.status.code() == Some(143),
            "SIGTERM status was not preserved",
        )?;
        let run = fixture.run_directory()?;
        check_stage_codes(&run, Some(("ordinary-build", "143")))?;
        check_failure_artifacts(fixture, &run, "partial synthetic build", "143")?;
        require(
            fixture.root.join("child-reaped.txt").is_file(),
            "fake compiler did not reap child",
        )?;
        let descendant = fs::read_to_string(&ready)?.trim().parse::<u32>()?;
        require(
            !Path::new("/proc").join(descendant.to_string()).exists(),
            "fake child survived or was not reaped",
        )?;
        require(
            fs::read_to_string(run.join("steps/ordinary-build.time"))?
                .contains("interrupted_status=143"),
            "signal timing limitation missing",
        )
    })
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root =
            env::temp_dir().join(format!("velum pgo launcher-{}-{stamp}", std::process::id()));
        fs::create_dir(&root)?;
        Ok(Self {
            checkout: root.join("checkout"),
            artifacts: root.join("artifacts"),
            tools: root.join("tools"),
            system_path: env::var_os("PATH").ok_or("PATH is unavailable")?,
            root,
        })
    }

    fn prepare(&self) -> TestResult {
        fs::create_dir_all(self.checkout.join("scripts"))?;
        fs::create_dir_all(self.checkout.join("runner"))?;
        fs::create_dir_all(&self.tools)?;
        fs::write(
            self.checkout.join("scripts/run-pgo-experiment.sh"),
            LAUNCHER,
        )?;
        fs::write(
            self.checkout.join("runner/Cargo.toml"),
            "# Synthetic; never compiled.\n",
        )?;
        fs::write(self.checkout.join("workload.txt"), "committed workload\n")?;
        fs::write(self.root.join("ambient-build-sentinel"), "must survive\n")?;
        let cases = self.checkout.join("tests/corpora/benchmarks/prepared");
        fs::create_dir_all(&cases)?;
        for cohort in ["representative", "holdout"] {
            for suffix in SUFFIXES {
                fs::write(
                    cases.join(format!("{cohort}_{suffix}.js")),
                    "// Never executed.\n",
                )?;
            }
        }
        for name in ["cargo", "rustc", "timeout"] {
            write_executable(&self.tools.join(name), MOCK_TOOL)?;
        }
        for name in ["cargo", "rustc"] {
            write_executable(&self.root.join("toolchain/bin").join(name), MOCK_TOOL)?;
        }
        for name in ["llvm-profdata", "llvm-size"] {
            write_executable(
                &self
                    .root
                    .join(format!("toolchain/lib/rustlib/{HOST}/bin"))
                    .join(name),
                MOCK_TOOL,
            )?;
        }
        self.git(&["init", "--quiet"])?;
        self.git(&["add", "."])?;
        self.git(&[
            "-c",
            "user.name=PGO launcher fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "--quiet",
            "-m",
            "Create synthetic PGO inputs",
        ])
    }

    fn command(&self, executable: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(executable);
        command
            .env_clear()
            .env("PATH", &self.system_path)
            .env("HOME", &self.root)
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null");
        command
    }

    fn git(&self, arguments: &[&str]) -> TestResult {
        success(
            &self
                .command("git")
                .current_dir(&self.checkout)
                .args(arguments)
                .output()?,
            "temporary Git fixture",
        )
    }

    fn launcher(
        &self,
        arguments: &[&str],
        scenario: &str,
    ) -> Result<Command, Box<dyn std::error::Error>> {
        let path = env::join_paths(
            std::iter::once(self.tools.clone()).chain(env::split_paths(&self.system_path)),
        )?;
        let mut command = self.command("bash");
        command
            .arg(self.checkout.join("scripts/run-pgo-experiment.sh"))
            .arg("--repo")
            .arg(&self.checkout)
            .arg("--artifact-root")
            .arg(&self.artifacts)
            .args(["--cpu", "inherit"])
            .args(arguments)
            .env("PATH", path)
            .env("MOCK_PGO_ROOT", &self.root)
            .env("MOCK_PGO_SCENARIO", scenario)
            .env("CARGO_TARGET_DIR", self.root.join("ambient-target"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Ok(command)
    }

    fn launch(
        &self,
        arguments: &[&str],
        scenario: &str,
    ) -> Result<Output, Box<dyn std::error::Error>> {
        bounded_output(self.launcher(arguments, scenario)?.spawn()?, TEST_TIMEOUT)
    }

    fn run_directory(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let entries = fs::read_dir(&self.artifacts)?.collect::<Result<Vec<_>, _>>()?;
        require(entries.len() == 1, "expected exactly one owned run")?;
        Ok(entries.first().ok_or("PGO run directory missing")?.path())
    }

    fn events(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(fs::read_to_string(self.root.join("events.tsv"))?)
    }
}

fn with_fixture(test: impl FnOnce(&Fixture) -> TestResult) -> TestResult {
    let fixture = Fixture::new()?;
    let result = fixture.prepare().and_then(|()| test(&fixture));
    let cleanup = cleanup_processes(&fixture.root)
        .and_then(|()| make_writable(&fixture.root))
        .and_then(|()| fs::remove_dir_all(&fixture.root).map_err(Into::into));
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => Err(format!("{error}; cleanup failed: {cleanup}").into()),
    }
}

fn bounded_output(mut child: Child, limit: Duration) -> Result<Output, Box<dyn std::error::Error>> {
    let started = Instant::now();
    while child.try_wait()?.is_none() {
        if started.elapsed() >= limit {
            send_signal(child.id(), "TERM")?;
            let grace = Instant::now();
            while child.try_wait()?.is_none() && grace.elapsed() < SIGNAL_TIMEOUT {
                thread::sleep(POLL_INTERVAL);
            }
            if child.try_wait()?.is_none() {
                child.kill()?;
            }
            child.wait()?;
            return Err("fake-tool launcher exceeded outer deadline".into());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Ok(child.wait_with_output()?)
}

fn send_signal(pid: u32, signal: &str) -> TestResult {
    let output = Command::new("kill")
        .args([format!("-{signal}"), "--".to_owned(), pid.to_string()])
        .output()?;
    if !output.status.success() && process_is_live(pid)? {
        return Err(format!("failed to send {signal} to fixture process {pid}").into());
    }
    Ok(())
}

fn process_is_live(pid: u32) -> Result<bool, Box<dyn std::error::Error>> {
    match fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("stat")) {
        Ok(stat) => Ok(stat
            .rsplit_once(") ")
            .and_then(|(_, tail)| tail.split_whitespace().next())
            != Some("Z")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn cleanup_processes(root: &Path) -> TestResult {
    let path = root.join("mock-pids.txt");
    if !path.exists() {
        return Ok(());
    }
    for line in fs::read_to_string(path)?.lines() {
        let pid = line.parse::<u32>()?;
        if process_is_live(pid)? {
            send_signal(pid, "KILL")?;
        }
    }
    Ok(())
}

fn make_writable(path: &Path) -> TestResult {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            make_writable(&entry?.path())?;
        }
    }
    Ok(())
}

fn write_executable(path: &Path, contents: &str) -> TestResult {
    fs::create_dir_all(path.parent().ok_or("fixture tool has no parent")?)?;
    fs::write(path, contents)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

fn check_stage_codes(run: &Path, failed: Option<(&str, &str)>) -> TestResult {
    let text = fs::read_to_string(run.join("steps.tsv"))?;
    let mut names = std::collections::BTreeSet::new();
    let mut observed_failure = false;
    for row in text.lines().skip(1) {
        let mut fields = row.split('\t');
        let name = fields.next().ok_or("missing stage name")?;
        let code = fields.next().ok_or("missing stage exit code")?;
        require(names.insert(name), "duplicate step outcome")?;
        if let Some((expected_name, expected_code)) = failed
            && name == expected_name
        {
            require(code == expected_code, "original failure code was lost")?;
            observed_failure = true;
        } else {
            require(code == "0", &format!("unexpected stage failure: {row}"))?;
        }
    }
    require(
        observed_failure == failed.is_some(),
        "expected failing stage omitted",
    )
}

fn check_build_order(events: &str, preset: &str) -> TestResult {
    let actual = events
        .lines()
        .filter(|line| line.starts_with("build\t"))
        .collect::<Vec<_>>();
    require(actual.len() == 3, "expected three fake builds")?;
    for (line, variant) in actual.into_iter().zip(["ordinary", "instrumented", "pgo"]) {
        require(
            line.starts_with(&format!("build\t{variant}\t{preset}\t")),
            "build order or preset drift",
        )?;
    }
    require(
        events
            .lines()
            .filter(|line| line.starts_with("clean\t"))
            .count()
            == 3,
        "builds were not cold-cleaned",
    )
}

fn check_lane_order(events: &str, rounds: usize) -> TestResult {
    let mut expected = Vec::new();
    for suffix in SUFFIXES {
        let id = format!("representative_{suffix}");
        expected.push(format!("lane\tinstrumented\t{id}\ttraining-{id}.md"));
    }
    for round in 1..=rounds {
        let order = if round % 2 == 0 {
            ["pgo", "ordinary"]
        } else {
            ["ordinary", "pgo"]
        };
        for variant in order {
            for suffix in SUFFIXES {
                let id = format!("holdout_{suffix}");
                expected.push(format!(
                    "lane\t{variant}\t{id}\tround-{round}-{variant}-{id}.md"
                ));
            }
            expected.push(format!(
                "lane\t{variant}\tmemory\tround-{round}-{variant}-memory.md"
            ));
        }
    }
    let actual = events
        .lines()
        .filter(|line| line.starts_with("lane\t"))
        .collect::<Vec<_>>();
    require(actual == expected, "training/AB/BA lane order changed")?;
    let merge = events.find("merge\t").ok_or("merge omitted")?;
    let evaluation = events
        .find("lane\tordinary\tholdout_")
        .ok_or("evaluation omitted")?;
    require(merge < evaluation, "holdouts started before profile merge")
}

fn check_preserved_artifacts(fixture: &Fixture, run: &Path) -> TestResult {
    require(
        !run.starts_with(&fixture.checkout),
        "artifacts inside checkout",
    )?;
    require(
        fs::read_to_string(fixture.root.join("ambient-build-sentinel"))? == "must survive\n",
        "ambient data changed",
    )?;
    for variant in ["ordinary", "instrumented", "pgo"] {
        require(
            fs::read_to_string(run.join("bin").join(variant))? == MOCK_TOOL,
            "saved fake runner changed",
        )?;
        let output = fixture
            .command("sha256sum")
            .arg("--check")
            .arg(run.join("bin").join(format!("{variant}.sha256")))
            .output()?;
        success(&output, "saved runner checksum")?;
    }
    for manifest in ["profile.sha256", "raw.sha256"] {
        success(
            &fixture
                .command("sha256sum")
                .arg("--check")
                .arg(run.join(manifest))
                .output()?,
            "profile hashes",
        )?;
    }
    require(
        fs::read_to_string(run.join("source/workload.txt"))? == "committed workload\n",
        "frozen source changed",
    )
}

fn check_failure_artifacts(fixture: &Fixture, run: &Path, marker: &str, code: &str) -> TestResult {
    let result = fs::read_to_string(run.join("result.txt"))?;
    require(
        result.contains("status=incomplete\n"),
        "failed run marked complete",
    )?;
    require(
        result.contains(&format!("exit_code={code}\n")),
        "launcher exit not recorded",
    )?;
    require(
        result.contains("last_step=ordinary-build\n"),
        "failed step identity lost",
    )?;
    require(
        fs::read_to_string(run.join("steps/ordinary-build.log"))?.contains(marker),
        "partial stdout discarded",
    )?;
    require(
        fs::metadata(run.join("steps/ordinary-build.stderr"))?.len() > 0,
        "partial stderr discarded",
    )?;
    require(
        run.join("steps/ordinary-build.command").is_file(),
        "failed command discarded",
    )?;
    require(
        run.join("source-archive.sha256").is_file(),
        "source evidence discarded",
    )?;
    require(
        !fixture
            .events()?
            .lines()
            .any(|line| line.starts_with("lane\t")),
        "training ran after failure",
    )?;
    require(
        !run.join("mock-summary.txt").exists(),
        "summary accepted failure",
    )
}

fn success(output: &Output, operation: &str) -> TestResult {
    if !output.status.success() {
        return Err(format!(
            "{operation} failed: {}; stdout={}; stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

fn rejected(output: &Output, reason: &str) -> TestResult {
    require(
        output.status.code() == Some(2),
        "failure did not exit with code 2",
    )?;
    require(
        String::from_utf8_lossy(&output.stderr).contains(reason),
        &format!(
            "missing diagnostic {reason}: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

fn require(condition: bool, message: &str) -> TestResult {
    if !condition {
        return Err(message.to_owned().into());
    }
    Ok(())
}
