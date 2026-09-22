//! Validate manifests without compiling or executing a benchmark workload.

use std::{
    fmt::Write as _,
    fs,
    path::PathBuf,
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail, ensure};

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
    cases: String,
    binaries: String,
}

impl Fixture {
    fn new() -> Result<Self> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root =
            std::env::temp_dir().join(format!("velum-pgo-manifest-{}-{stamp}", std::process::id()));
        fs::create_dir_all(root.join("source/tests/corpora/benchmarks/prepared"))?;
        fs::create_dir(root.join("bin"))?;
        let mut cases = String::from("id\tsource\tsha256\n");
        for cohort in ["representative", "holdout"] {
            for suffix in SUFFIXES {
                let id = format!("{cohort}_{suffix}");
                let source = format!("tests/corpora/benchmarks/prepared/{id}.js");
                let file = root.join("source").join(&source);
                fs::write(&file, "1;\n")?;
                writeln!(cases, "{id}\t{source}\t{}", digest(&file)?)?;
            }
        }
        let binary = root.join("bin/ordinary");
        fs::write(&binary, "mock binary\n")?;
        let binaries = format!(
            "variant\tbytes\tsha256\nordinary\t{}\t{}\n",
            fs::metadata(&binary)?.len(),
            digest(&binary)?
        );
        let fixture = Self {
            root,
            cases,
            binaries,
        };
        fixture.reset()?;
        Ok(fixture)
    }

    fn reset(&self) -> Result<()> {
        fs::write(self.root.join("cases.tsv"), &self.cases)?;
        fs::write(self.root.join("binaries.tsv"), &self.binaries)?;
        Ok(())
    }

    fn run(&self) -> Result<Output> {
        Command::new(env!("CARGO_BIN_EXE_velum-test-runner"))
            .arg("--pgo-manifest")
            .arg(&self.root)
            .args([
                &"a".repeat(40),
                &"b".repeat(40),
                "x86_64-unknown-linux-gnu",
                "0",
                "2",
                "release",
            ])
            .output()
            .context("failed to run manifest verifier")
    }
}

fn digest(path: &std::path::Path) -> Result<String> {
    let output = Command::new("sha256sum").arg(path).output()?;
    ensure!(output.status.success(), "fixture hashing failed");
    String::from_utf8(output.stdout)?
        .split_whitespace()
        .next()
        .map(str::to_owned)
        .context("missing fixture hash")
}

fn with_fixture(test: impl FnOnce(&Fixture) -> Result<()>) -> Result<()> {
    let fixture = Fixture::new()?;
    let outcome = test(&fixture);
    let cleanup = fs::remove_dir_all(&fixture.root);
    match (outcome, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error).context("failed to clean manifest fixture"),
        (Err(error), Err(cleanup)) => bail!("{error:#}; cleanup failed: {cleanup}"),
    }
}

fn rejection(output: &Output, message: &str) -> Result<()> {
    ensure!(
        !output.status.success(),
        "accepted invalid manifest: {message}"
    );
    ensure!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "wrong rejection: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn manifest_pins_source_binary_size_and_protocol() -> Result<()> {
    with_fixture(|fixture| {
        let output = fixture.run()?;
        ensure!(
            output.status.success(),
            "manifest failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture.root.join("experiment.json"))?)?;
        ensure!(
            manifest
                .pointer("/protocol_version")
                .and_then(serde_json::Value::as_str)
                == Some("2-frozen-before-pgo-training")
        );
        ensure!(
            manifest
                .pointer("/binaries/ordinary/bytes")
                .and_then(serde_json::Value::as_u64)
                == Some(12)
        );
        ensure!(
            manifest
                .get("cases")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|cases| cases.len() == 12)
        );
        Ok(())
    })
}

#[test]
fn manifest_rejects_malformed_or_incomplete_workload_tables() -> Result<()> {
    with_fixture(|fixture| {
        for (text, message) in [
            (String::from("wrong\n"), "invalid PGO workload header"),
            (
                String::from("id\tsource\tsha256\n"),
                "missing PGO workload IDs",
            ),
            (
                fixture.cases.replace(
                    "representative_object_transform",
                    "holdout_object_transform",
                ),
                "unknown or duplicate PGO workload ID",
            ),
            (
                fixture.cases.replace(
                    "tests/corpora/benchmarks/prepared/representative_object_transform.js",
                    "../outside.js",
                ),
                "invalid PGO workload source path",
            ),
        ] {
            fs::write(fixture.root.join("cases.tsv"), text)?;
            rejection(&fixture.run()?, message)?;
        }
        Ok(())
    })
}

#[test]
fn manifest_rejects_malformed_binary_tables_and_changed_saved_binary() -> Result<()> {
    with_fixture(|fixture| {
        for (text, message) in [
            (String::from("wrong\n"), "invalid PGO binary header"),
            (
                String::from("variant\tbytes\tsha256\n"),
                "ordinary PGO verifier is missing",
            ),
            (
                fixture.binaries.replace("ordinary\t12", "ordinary\t13"),
                "PGO binary size mismatch",
            ),
            (
                fixture.binaries.replace("ordinary\t", "unknown\t"),
                "invalid PGO variant",
            ),
        ] {
            fs::write(fixture.root.join("binaries.tsv"), text)?;
            rejection(&fixture.run()?, message)?;
        }
        fixture.reset()?;
        fs::write(fixture.root.join("bin/ordinary"), "mock change\n")?;
        rejection(&fixture.run()?, "SHA-256 changed")
    })
}

#[test]
fn manifest_rejects_changed_workload_bytes() -> Result<()> {
    with_fixture(|fixture| {
        fs::write(
            fixture
                .root
                .join("source/tests/corpora/benchmarks/prepared/holdout_tree_allocation.js"),
            "2;\n",
        )?;
        rejection(&fixture.run()?, "SHA-256 changed")
    })
}
