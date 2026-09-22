//! Local Git isolation fixtures. Fake runners never execute JavaScript or compile.
#![cfg(target_os = "linux")]

use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

#[path = "fixtures/pgo/mock_time.rs"]
mod mock_time;

type TestResult = Result<(), Box<dyn std::error::Error>>;
type Fallible<T> = Result<T, Box<dyn std::error::Error>>;
type Overrides = Vec<(&'static str, OsString)>;
type Snapshot = BTreeMap<PathBuf, Vec<u8>>;

const LAUNCHER: &str = include_str!("../scripts/check-pgo-correctness.sh");
const PATCH_A_NAME: &str = "f2d1435644797268dca1f7988cad5a4e89ccd8d2.patch";
const PATCH_B_NAME: &str = "staging-annex-b-arguments-object.patch";
const PATCH_A: &str = "diff --git a/test/case.js b/test/case.js\n--- a/test/case.js\n+++ b/test/case.js\n@@ -1 +1 @@\n-// Upstream test fixture.\n+// Patched test fixture.\n";
const PATCH_B: &str = "diff --git a/harness/assert.js b/harness/assert.js\n--- a/harness/assert.js\n+++ b/harness/assert.js\n@@ -1 +1 @@\n-// Upstream harness fixture.\n+// Patched harness fixture.\n";
const MOCK_PGO: &str = r#"#!/usr/bin/env bash
set -euo pipefail
[[ $# == 2 && "$1" == --correctness ]] || exit 91
printf 'fake saved PGO reached\n' > "$MOCK_PGO_GIT_ROOT/executed.txt"
env | sort > "$MOCK_PGO_GIT_ROOT/child-environment.txt"
printf 'synthetic component\n' > "${2%.md}-component.yaml"
printf 'synthetic pass candidate\n' > "$VELUM_TEST262_PASS_CANDIDATE_PATH"
"#;
const MOCK_VERIFIER: &str = r#"#!/usr/bin/env bash
set -euo pipefail
[[ $# == 4 && "$1" == --pgo-correctness-check ]] || exit 92
printf '{"fixture":"only shell Git isolation is tested"}\n' > "$4"
"#;

struct Fixture {
    root: PathBuf,
    corpus: PathBuf,
    pristine: PathBuf,
    experiment: PathBuf,
    system_path: OsString,
}

#[test]
fn inherited_work_tree_cannot_hide_modified_supplied_corpus() -> TestResult {
    with_fixture(|fixture| {
        fixture.modify_supplied_corpus()?;
        let before = fixture.protected_snapshot()?;
        let output =
            fixture.launch(&[("GIT_WORK_TREE", fixture.pristine.clone().into_os_string())])?;
        fixture.reject_before_runner(&output, "GIT_WORK_TREE")?;
        require(
            fixture.protected_snapshot()? == before,
            "Git isolation changed protected metadata",
        )
    })
}

#[test]
fn inherited_git_context_never_redirects_validation_to_pristine_clone() -> TestResult {
    with_fixture(|fixture| {
        fixture.modify_supplied_corpus()?;
        let before = fixture.protected_snapshot()?;
        for (name, overrides) in fixture.redirecting_overrides() {
            let output = fixture.launch(&overrides)?;
            fixture.reject_before_runner(&output, name)?;
            require(
                fixture.protected_snapshot()? == before,
                &format!("{name} changed protected metadata"),
            )?;
        }
        Ok(())
    })
}

#[test]
fn clean_corpus_runs_with_sanitized_git_environment_and_preserved_sentinels() -> TestResult {
    with_fixture(|fixture| {
        let before = fixture.protected_snapshot()?;
        for (name, overrides) in fixture.all_overrides() {
            let output = fixture.launch(&overrides)?;
            successful(&output, &format!("clean corpus with {name}"))?;
            require(
                fixture.root.join("executed.txt").is_file(),
                "clean fixture never reached fake PGO",
            )?;
            fixture.check_child_git_environment()?;
            fixture.check_synthetic_timing()?;
            require(
                fixture.protected_snapshot()? == before,
                &format!("{name} changed corpus, pristine clone, or external Git storage"),
            )?;
            fs::remove_file(fixture.root.join("executed.txt"))?;
            fs::remove_file(fixture.root.join("child-environment.txt"))?;
        }
        Ok(())
    })
}

#[test]
fn missing_owned_timer_is_rejected_before_fake_correctness_runner() -> TestResult {
    with_fixture(|fixture| {
        fs::remove_file(fixture.root.join("mock-time"))?;
        let output = fixture.launch(&[])?;
        require(
            output.status.code() == Some(2),
            "missing fixture timer was not rejected",
        )?;
        let stderr = String::from_utf8(output.stderr)?;
        require(
            stderr.contains("missing /usr/bin/time"),
            "missing fixture timer failed for an unrelated reason",
        )?;
        require(
            !fixture.root.join("executed.txt").exists(),
            "missing timer reached fake PGO",
        )?;
        for entry in fs::read_dir(&fixture.experiment)? {
            let entry = entry?;
            require(
                !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("correctness-"),
                "missing timer created a correctness run before preflight completed",
            )?;
        }
        Ok(())
    })
}

impl Fixture {
    fn new() -> Fallible<Self> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = env::temp_dir().join(format!("velum pgo git-{}-{stamp}", std::process::id()));
        fs::create_dir(&root)?;
        Ok(Self {
            corpus: root.join("supplied-corpus"),
            pristine: root.join("pristine-patched-clone"),
            experiment: root.join("experiment"),
            system_path: env::var_os("PATH").ok_or("PATH is unavailable")?,
            root,
        })
    }

    fn prepare(&self) -> TestResult {
        fs::create_dir_all(self.corpus.join("test"))?;
        fs::create_dir_all(self.corpus.join("harness"))?;
        fs::write(
            self.corpus.join("test/case.js"),
            "// Upstream test fixture.\n",
        )?;
        fs::write(
            self.corpus.join("harness/assert.js"),
            "// Upstream harness fixture.\n",
        )?;
        self.git(&self.corpus, &["init", "--quiet"])?;
        self.git(&self.corpus, &["add", "test", "harness"])?;
        self.git(
            &self.corpus,
            &[
                "-c",
                "user.name=Git isolation fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "--quiet",
                "-m",
                "Create tiny local corpus",
            ],
        )?;
        let pin = self.git_text(&self.corpus, &["rev-parse", "HEAD"])?;
        let tree = self.git_text(&self.corpus, &["rev-parse", "HEAD:"])?;
        self.clone_pristine()?;
        self.prepare_experiment(&pin, &tree)?;
        for corpus in [&self.corpus, &self.pristine] {
            fs::write(corpus.join("test/case.js"), "// Patched test fixture.\n")?;
            fs::write(
                corpus.join("harness/assert.js"),
                "// Patched harness fixture.\n",
            )?;
        }
        fs::write(
            self.root.join("external-index"),
            "external index sentinel: do not overwrite\n",
        )?;
        for directory in ["external-objects", "external-alternates"] {
            fs::create_dir(self.root.join(directory))?;
            fs::write(
                self.root.join(directory).join("sentinel"),
                "external Git objects must not change\n",
            )?;
        }
        fs::write(
            self.root.join("launcher.sh"),
            mock_time::launcher(LAUNCHER, &self.root)?,
        )?;
        Ok(())
    }

    fn clone_pristine(&self) -> TestResult {
        let output = self
            .command("git")
            .args(["clone", "--quiet", "--no-hardlinks"])
            .arg(&self.corpus)
            .arg(&self.pristine)
            .output()?;
        successful(&output, "create pristine local clone")
    }

    fn prepare_experiment(&self, pin: &str, tree: &str) -> TestResult {
        let source = self.experiment.join("source");
        let test262 = source.join("tests/corpora/test262");
        let patches = test262.join("patches");
        fs::create_dir_all(&patches)?;
        fs::create_dir(self.experiment.join("bin"))?;
        fs::write(patches.join(PATCH_A_NAME), PATCH_A)?;
        fs::write(patches.join(PATCH_B_NAME), PATCH_B)?;
        fs::write(
            test262.join("full-pass-baseline.txt"),
            format!(
                "# velum-test262-pass-baseline-v2\n# test262_commit={pin}\n# test262_patches=f2d1435644797268dca1f7988cad5a4e89ccd8d2,staging-annex-b-arguments-object\ntest/case.js#default\n"
            ),
        )?;
        fs::write(
            self.experiment.join("provenance.txt"),
            format!("commit={pin}\ntree={tree}\n"),
        )?;
        fs::write(
            self.experiment.join("result.txt"),
            "status=complete-needs-review\nexit_code=0\n",
        )?;
        for (name, script) in [("pgo", MOCK_PGO), ("ordinary", MOCK_VERIFIER)] {
            let binary = self.experiment.join("bin").join(name);
            executable(&binary, script)?;
            self.hash_files(
                &self.experiment.join("bin").join(format!("{name}.sha256")),
                &[binary],
                &self.root,
            )?;
        }
        executable(&self.root.join("qjs"), "#!/usr/bin/env bash\nexit 0\n")?;
        let profile = self.experiment.join("merged.profdata");
        fs::write(&profile, "synthetic profile, never interpreted\n")?;
        self.hash_files(
            &self.experiment.join("profile.sha256"),
            &[profile],
            &self.root,
        )?;
        let files = [
            PathBuf::from("./tests/corpora/test262/full-pass-baseline.txt"),
            PathBuf::from(format!("./tests/corpora/test262/patches/{PATCH_A_NAME}")),
            PathBuf::from(format!("./tests/corpora/test262/patches/{PATCH_B_NAME}")),
        ];
        self.hash_files(
            &self.experiment.join("source-files.sha256"),
            &files,
            &source,
        )?;
        let layout = self
            .command("find")
            .current_dir(&source)
            .args([".", "-printf", "%y\t%p\t%l\n"])
            .output()?;
        successful(&layout, "capture synthetic source layout")?;
        let layout_text = String::from_utf8(layout.stdout)?;
        let mut lines: Vec<_> = layout_text.lines().collect();
        lines.sort_unstable();
        fs::write(
            self.experiment.join("source-layout.tsv"),
            format!("{}\n", lines.join("\n")),
        )?;
        Ok(())
    }

    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        command
            .env_clear()
            .env("PATH", &self.system_path)
            .env("HOME", &self.root)
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null");
        command
    }

    fn git(&self, directory: &Path, arguments: &[&str]) -> TestResult {
        successful(
            &self
                .command("git")
                .current_dir(directory)
                .args(arguments)
                .output()?,
            "temporary Git fixture",
        )
    }

    fn git_text(&self, directory: &Path, arguments: &[&str]) -> Fallible<String> {
        let output = self
            .command("git")
            .current_dir(directory)
            .args(arguments)
            .output()?;
        successful(&output, "read temporary Git identity")?;
        Ok(String::from_utf8(output.stdout)?.trim().to_owned())
    }

    fn hash_files(&self, output: &Path, files: &[PathBuf], directory: &Path) -> TestResult {
        let result = self
            .command("sha256sum")
            .current_dir(directory)
            .arg("--")
            .args(files)
            .output()?;
        successful(&result, "hash synthetic evidence")?;
        fs::write(output, result.stdout)?;
        Ok(())
    }

    fn launch(&self, overrides: &[(&str, OsString)]) -> Fallible<Output> {
        let mut command = self.command("timeout");
        command
            .args(["--kill-after=1s", "20s", "bash"])
            .arg(self.root.join("launcher.sh"))
            .arg(&self.experiment)
            .arg(&self.corpus)
            .arg(self.root.join("qjs"))
            .env("MOCK_PGO_GIT_ROOT", &self.root)
            .stdin(Stdio::null());
        for (name, value) in overrides {
            command.env(name, value);
        }
        Ok(command.output()?)
    }

    fn modify_supplied_corpus(&self) -> TestResult {
        fs::write(
            self.corpus.join("test/case.js"),
            "// Modified supplied corpus must fail.\n",
        )?;
        Ok(())
    }

    fn redirecting_overrides(&self) -> Vec<(&'static str, Overrides)> {
        let worktree = self.pristine.clone().into_os_string();
        let git_dir = self.pristine.join(".git").into_os_string();
        vec![
            (
                "GIT_DIR",
                vec![
                    ("GIT_DIR", git_dir.clone()),
                    ("GIT_WORK_TREE", worktree.clone()),
                ],
            ),
            (
                "GIT_COMMON_DIR",
                vec![
                    ("GIT_COMMON_DIR", git_dir),
                    ("GIT_WORK_TREE", worktree.clone()),
                ],
            ),
            (
                "GIT_CONFIG_COUNT",
                vec![
                    ("GIT_CONFIG_COUNT", "1".into()),
                    ("GIT_CONFIG_KEY_0", "core.worktree".into()),
                    ("GIT_CONFIG_VALUE_0", worktree),
                ],
            ),
            (
                "GIT_CONFIG_PARAMETERS",
                vec![(
                    "GIT_CONFIG_PARAMETERS",
                    format!("'core.worktree={}'", self.pristine.display()).into(),
                )],
            ),
        ]
    }

    fn all_overrides(&self) -> Vec<(&'static str, Overrides)> {
        let mut cases = self.redirecting_overrides();
        cases.extend([
            (
                "GIT_WORK_TREE",
                vec![("GIT_WORK_TREE", self.pristine.clone().into_os_string())],
            ),
            (
                "standalone GIT_DIR",
                vec![(
                    "GIT_DIR",
                    self.root.join("missing-git-directory").into_os_string(),
                )],
            ),
            (
                "standalone GIT_COMMON_DIR",
                vec![(
                    "GIT_COMMON_DIR",
                    self.root.join("missing-common-directory").into_os_string(),
                )],
            ),
            (
                "GIT_INDEX_FILE",
                vec![(
                    "GIT_INDEX_FILE",
                    self.root.join("external-index").into_os_string(),
                )],
            ),
            (
                "GIT_OBJECT_DIRECTORY",
                vec![
                    (
                        "GIT_OBJECT_DIRECTORY",
                        self.root.join("external-objects").into_os_string(),
                    ),
                    (
                        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                        self.root.join("external-alternates").into_os_string(),
                    ),
                ],
            ),
            ("GIT_PREFIX", vec![("GIT_PREFIX", "test/".into())]),
            (
                "unknown exported Git override",
                vec![("GIT_FUTURE_OVERRIDE_FIXTURE", "must-be-cleared".into())],
            ),
        ]);
        cases
    }

    fn reject_before_runner(&self, output: &Output, name: &str) -> TestResult {
        require(
            output.status.code() == Some(2),
            &format!(
                "{name}: expected corpus rejection, got {:?}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ),
        )?;
        require(
            String::from_utf8_lossy(&output.stderr).contains(
                "Test262 content differs from the exact pinned commit plus two tracked patches",
            ),
            &format!("{name}: rejected for an unrelated reason"),
        )?;
        require(
            !self.root.join("executed.txt").exists(),
            &format!("{name}: modified corpus reached the fake PGO runner"),
        )
    }

    fn protected_snapshot(&self) -> Fallible<Snapshot> {
        let mut snapshot = BTreeMap::new();
        for directory in [
            self.corpus.join(".git"),
            self.pristine.join(".git"),
            self.root.join("external-objects"),
            self.root.join("external-alternates"),
        ] {
            collect_files(&directory, &mut snapshot)?;
        }
        let index = self.root.join("external-index");
        require(
            snapshot.insert(index.clone(), fs::read(index)?).is_none(),
            "duplicate external index snapshot",
        )?;
        Ok(snapshot)
    }

    fn check_child_git_environment(&self) -> TestResult {
        let text = fs::read_to_string(self.root.join("child-environment.txt"))?;
        let actual: Vec<_> = text
            .lines()
            .filter(|line| line.starts_with("GIT_"))
            .collect();
        let expected = [
            "GIT_CONFIG_GLOBAL=/dev/null",
            "GIT_CONFIG_NOSYSTEM=1",
            "GIT_NO_REPLACE_OBJECTS=1",
            "GIT_OPTIONAL_LOCKS=0",
        ];
        require(
            actual == expected,
            &format!("fake PGO inherited unexpected Git context: {actual:?}"),
        )
    }

    fn check_synthetic_timing(&self) -> TestResult {
        let mut found = false;
        for entry in fs::read_dir(&self.experiment)? {
            let entry = entry?;
            if !entry
                .file_name()
                .to_string_lossy()
                .starts_with("correctness-")
            {
                continue;
            }
            let timing = fs::read_to_string(entry.path().join("time.txt"))?;
            require(
                timing
                    .lines()
                    .any(|line| line == "timing_source=synthetic-fixture"),
                "successful correctness fixture omitted synthetic timing provenance",
            )?;
            require(
                timing.lines().any(|line| line == "exit_status=0"),
                "successful correctness fixture did not record the child exit status",
            )?;
            found = true;
        }
        require(
            found,
            "successful correctness fixture has no owned timing report",
        )
    }
}

fn collect_files(directory: &Path, snapshot: &mut Snapshot) -> TestResult {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files(&path, snapshot)?;
        } else {
            require(
                entry.file_type()?.is_file(),
                "unexpected symlink in tiny Git fixture",
            )?;
            require(
                snapshot.insert(path.clone(), fs::read(path)?).is_none(),
                "duplicate protected Git path",
            )?;
        }
    }
    Ok(())
}

fn executable(path: &Path, script: &str) -> TestResult {
    fs::write(path, script)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

fn successful(output: &Output, description: &str) -> TestResult {
    require(
        output.status.success(),
        &format!(
            "{description}: {:?}\nstdout: {}\nstderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
}

fn require(condition: bool, message: &str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned().into())
    }
}

fn with_fixture(test: impl FnOnce(&Fixture) -> TestResult) -> TestResult {
    let fixture = Fixture::new()?;
    let result = fixture.prepare().and_then(|()| test(&fixture));
    let cleanup = fs::remove_dir_all(&fixture.root);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error.into()),
        (Err(error), Err(cleanup)) => {
            Err(format!("{error}; fixture cleanup failed: {cleanup}").into())
        }
    }
}
