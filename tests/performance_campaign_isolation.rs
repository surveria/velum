//! Exercise the real launcher with fake compiler and benchmark executables.
#![cfg(target_os = "linux")]

use std::{
    env,
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const LAUNCHER: &str = include_str!("../scripts/run-performance-campaign.sh");
const MOCK_CARGO: &str = include_str!("fixtures/performance-campaign/cargo.sh");
const MOCK_RUSTC: &str = include_str!("fixtures/performance-campaign/rustc.sh");
const MOCK_RUNNER: &str = include_str!("fixtures/performance-campaign/runner.sh");
const COMMITTED_WORKLOAD: &str = "committed workload\n";
const CHANGED_WORKLOAD: &str = "changed during mock build\n";

struct Fixture {
    root: PathBuf,
    checkout: PathBuf,
    artifacts: PathBuf,
    tools: PathBuf,
    system_path: OsString,
}

#[test]
fn campaign_snapshots_committed_inputs_and_preserves_external_replay() -> TestResult {
    with_fixture(|fixture| {
        let output = fixture.launch("representative", false)?;
        ensure_success(&output, "isolated campaign")?;
        let campaign = fixture.campaign_directory()?;
        let snapshot = campaign.join("source");
        ensure_file(&fixture.checkout.join("workload.txt"), CHANGED_WORKLOAD)?;
        ensure_file(&snapshot.join("workload.txt"), COMMITTED_WORKLOAD)?;
        ensure_file(&campaign.join("bin/velum-test-runner"), MOCK_RUNNER)?;
        let report = fs::read_to_string(campaign.join("representative.md"))?;
        require(
            report.contains("source=committed workload\n"),
            "report used changed input",
        )?;
        let provenance = fs::read_to_string(campaign.join("provenance.txt"))?;
        require(
            provenance.contains("Finished UTC:"),
            "campaign has no completion marker",
        )?;
        let mut verify = fixture.command("sha256sum");
        verify.arg("--check").arg(campaign.join("binary.sha256"));
        ensure_success(&verify.output()?, "saved executable digest")?;
        require(
            !campaign.starts_with(&fixture.checkout),
            "artifacts are inside checkout",
        )?;
        fs::remove_dir_all(&fixture.checkout)?;
        let mut replay = fixture.command(campaign.join("bin/velum-test-runner"));
        replay
            .current_dir(&snapshot)
            .arg("--performance")
            .arg(campaign.join("replay.md"));
        ensure_success(&replay.output()?, "replay after checkout removal")?;
        let replay_report = fs::read_to_string(campaign.join("replay.md"))?;
        require(
            replay_report.contains("source=committed workload\n"),
            "saved replay lost input",
        )?;
        ensure_no_mock_processes(&campaign)
    })
}

#[test]
fn campaign_records_failed_and_timed_out_lanes_then_continues() -> TestResult {
    with_fixture(|fixture| {
        let output = fixture.launch("all", true)?;
        require(
            output.status.code() == Some(1),
            "failed campaign did not exit with status 1",
        )?;
        let stderr = String::from_utf8(output.stderr)?;
        let campaign = fixture.campaign_directory()?;
        let watchdog_log = fs::read_to_string(campaign.join("holdout.log"))?;
        require(
            stderr.contains("process watchdog"),
            &format!("watchdog diagnostic was lost: {stderr}; log: {watchdog_log}"),
        )?;
        let statuses = fs::read_to_string(campaign.join("lanes.tsv"))?;
        let mut rows = statuses.lines().skip(1);
        for (lane, status) in [
            ("sentinel", "0"),
            ("representative", "7"),
            ("holdout", "124"),
            ("embedding", "0"),
            ("jetstream", "0"),
            ("memory", "0"),
        ] {
            let row = rows.next().ok_or("a campaign lane status was omitted")?;
            let mut fields = row.split('\t');
            require(
                fields.next() == Some(lane),
                "lane order changed or execution stopped",
            )?;
            require(
                fields.next() == Some(status),
                &format!("wrong exit code for {lane}: {row}"),
            )?;
            require(
                campaign.join(format!("{lane}.md")).is_file(),
                "lane report was discarded",
            )?;
            require(
                campaign.join(format!("{lane}.log")).is_file(),
                "lane log was discarded",
            )?;
        }
        require(rows.next().is_none(), "unexpected duplicate campaign lane")?;
        let log = fs::read_to_string(campaign.join("holdout.log"))?;
        require(
            log.contains("mock child reaped"),
            "watchdog child was not reaped",
        )?;
        ensure_no_mock_processes(&campaign)
    })
}

fn with_fixture(test: impl FnOnce(&Fixture) -> TestResult) -> TestResult {
    let fixture = Fixture::new()?;
    let result = fixture.prepare().and_then(|()| test(&fixture));
    let cleanup = fs::remove_dir_all(&fixture.root);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(format!("temporary fixture cleanup failed: {error}").into()),
        (Err(error), Err(cleanup)) => {
            Err(format!("{error}; fixture cleanup failed: {cleanup}").into())
        }
    }
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = env::temp_dir().join(format!(
            "velum campaign fixture-{}-{stamp}",
            std::process::id()
        ));
        let system_path = env::var_os("PATH").ok_or("PATH is unavailable")?;
        fs::create_dir(&root)?;
        Ok(Self {
            checkout: root.join("checkout"),
            artifacts: root.join("artifacts"),
            tools: root.join("tools"),
            system_path,
            root,
        })
    }

    fn prepare(&self) -> TestResult {
        fs::create_dir_all(self.checkout.join("scripts"))?;
        fs::create_dir(&self.tools)?;
        fs::write(
            self.checkout.join("scripts/run-performance-campaign.sh"),
            LAUNCHER,
        )?;
        fs::write(self.checkout.join("scripts/mock-runner.sh"), MOCK_RUNNER)?;
        fs::write(self.checkout.join("workload.txt"), COMMITTED_WORKLOAD)?;
        self.write_executable("cargo", MOCK_CARGO)?;
        self.write_executable("rustc", MOCK_RUSTC)?;
        self.git(&["init", "--quiet"])?;
        self.git(&["add", "."])?;
        self.git(&[
            "-c",
            "user.name=Campaign test fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "--quiet",
            "-m",
            "Create isolated launcher fixture",
        ])
    }

    fn write_executable(&self, name: &str, source: &str) -> TestResult {
        let path = self.tools.join(name);
        fs::write(&path, source)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        Ok(())
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
        let output = self
            .command("git")
            .current_dir(&self.checkout)
            .args(arguments)
            .output()?;
        ensure_success(&output, "temporary Git fixture")
    }

    fn launch(&self, lane: &str, failures: bool) -> Result<Output, Box<dyn std::error::Error>> {
        let path = env::join_paths(
            std::iter::once(self.tools.clone()).chain(env::split_paths(&self.system_path)),
        )?;
        let output = self
            .command("timeout")
            .args(["--kill-after=1s", "20s", "bash"])
            .arg(self.checkout.join("scripts/run-performance-campaign.sh"))
            .args(["--lane", lane, "--artifact-root"])
            .arg(&self.artifacts)
            .env("PATH", path)
            .env("CARGO_TARGET_DIR", self.root.join("build-cache"))
            .env("VELUM_PERFORMANCE_LANE_TIMEOUT_SECONDS", "1")
            .env("MOCK_ORIGINAL_CHECKOUT", &self.checkout)
            .env("MOCK_FAILURES", if failures { "1" } else { "0" })
            .output()?;
        Ok(output)
    }

    fn campaign_directory(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let entries = fs::read_dir(&self.artifacts)?.collect::<Result<Vec<_>, _>>()?;
        require(
            entries.len() == 1,
            "expected exactly one external campaign directory",
        )?;
        Ok(entries
            .first()
            .ok_or("campaign directory is missing")?
            .path())
    }
}

fn ensure_success(output: &Output, operation: &str) -> TestResult {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{operation}: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}

fn ensure_file(path: &Path, expected: &str) -> TestResult {
    let actual = fs::read_to_string(path)?;
    require(
        actual == expected,
        &format!("unexpected file contents at {}", path.display()),
    )
}

fn ensure_no_mock_processes(campaign: &Path) -> TestResult {
    for pid in fs::read_to_string(campaign.join("mock-pids.txt"))?.lines() {
        let pid = pid.parse::<u32>()?;
        require(
            !Path::new("/proc").join(pid.to_string()).exists(),
            &format!("mock process {pid} leaked"),
        )?;
    }
    Ok(())
}

fn require(condition: bool, message: &str) -> TestResult {
    if condition {
        return Ok(());
    }
    Err(message.to_owned().into())
}
