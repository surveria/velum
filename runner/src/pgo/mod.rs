//! Opt-in PGO artifact validation; this module never runs benchmark workloads.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context as _, bail, ensure};
use serde_json::{Value, json};

mod evidence;
mod memory;
mod profile;
mod summary;
mod types;

pub fn run(command: &str, arguments: Vec<String>) -> anyhow::Result<()> {
    match command {
        "--pgo-manifest" => write_manifest(arguments),
        "--pgo-profile-check" => {
            let [input, output] = exact(arguments)?;
            profile::verify_profile(Path::new(&input), Path::new(&output))
        }
        "--pgo-diagnostics-check" => {
            let [input, symbols, output] = exact(arguments)?;
            profile::verify_diagnostics(Path::new(&input), Path::new(&symbols), Path::new(&output))
        }
        "--pgo-report-check" => match arguments.as_slice() {
            [run, variant, label, kind] => {
                evidence::verify_report(Path::new(run), variant, label, kind, None)
            }
            [run, variant, label, kind, case] => {
                evidence::verify_report(Path::new(run), variant, label, kind, Some(case))
            }
            _ => bail!("expected RUN VARIANT LABEL KIND [CASE] after --pgo-report-check"),
        },
        "--pgo-summary" => {
            let [run] = exact(arguments)?;
            summary::summarize(Path::new(&run))
        }
        "--pgo-correctness-check" => {
            let [run, report, output] = exact(arguments)?;
            evidence::verify_correctness(Path::new(&run), Path::new(&report), Path::new(&output))
        }
        _ => bail!("unknown PGO artifact command: {command}"),
    }
}

fn exact<const N: usize>(arguments: Vec<String>) -> anyhow::Result<[String; N]> {
    arguments.try_into().map_err(|arguments: Vec<String>| {
        anyhow::anyhow!("expected {N} PGO arguments, received {}", arguments.len())
    })
}

fn write_manifest(arguments: Vec<String>) -> anyhow::Result<()> {
    let [run, commit, tree, host, cpu, rounds, preset] = exact(arguments)?;
    let root = Path::new(&run)
        .canonicalize()
        .context("invalid PGO artifact directory")?;
    let source = root
        .join("source")
        .canonicalize()
        .context("missing PGO source snapshot")?;
    let rounds: u32 = rounds.parse().context("invalid PGO round count")?;
    ensure!(
        (1..=5).contains(&rounds),
        "PGO rounds must be between one and five"
    );
    ensure!(
        preset == "release" || preset == "thin-lto",
        "unsupported PGO preset"
    );
    ensure!(host == "x86_64-unknown-linux-gnu", "unsupported PGO target");
    ensure!(
        cpu == "inherit" || cpu.parse::<u32>().is_ok(),
        "invalid PGO CPU selection"
    );
    ensure!(
        source == root.join("source"),
        "PGO source must not be an external symlink"
    );
    for (name, value) in [("commit", &commit), ("tree", &tree)] {
        ensure!(
            value.len() == 40
                && value
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "invalid PGO {name}"
        );
    }
    let cases = read_cases(&root.join("cases.tsv"), &source)?;
    let binaries = read_binaries(&root.join("binaries.tsv"), &root)?;
    let manifest = json!({
        "schema_version": 1, "protocol_version": "2-frozen-before-pgo-training",
        "commit": commit, "tree": tree, "host": host, "cpu": cpu,
        "rounds": rounds, "preset": preset, "source": source,
        "cases": cases, "binaries": binaries,
    });
    fs::write(
        root.join("experiment.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )
    .context("failed to record PGO manifest")
}

fn read_cases(path: &Path, root: &Path) -> anyhow::Result<Vec<Value>> {
    let text = fs::read_to_string(path).context("failed to read PGO workload manifest")?;
    let mut lines = text.lines();
    ensure!(
        lines.next() == Some("id\tsource\tsha256"),
        "invalid PGO workload header"
    );
    let mut seen = BTreeSet::new();
    let expected: BTreeSet<_> = ["representative", "holdout"]
        .into_iter()
        .flat_map(|cohort| types::CASE_SUFFIXES.map(|suffix| format!("{cohort}_{suffix}")))
        .collect();
    let cases = lines
        .map(|line| {
            let mut fields = line.split('\t');
            let id = fields.next().context("missing PGO workload ID")?;
            let source = fields.next().context("missing PGO workload source")?;
            let sha256 = fields.next().context("missing PGO workload digest")?;
            ensure!(fields.next().is_none(), "extra PGO workload fields");
            ensure!(
                expected.contains(id) && seen.insert(id.to_owned()),
                "unknown or duplicate PGO workload ID"
            );
            ensure!(
                source == format!("tests/corpora/benchmarks/prepared/{id}.js"),
                "invalid PGO workload source path"
            );
            let workload = root.join(source);
            ensure!(
                workload.canonicalize()? == workload,
                "PGO workload must not resolve through a symlink"
            );
            types::check_hash(&workload, sha256)?;
            Ok(json!({"id": id, "source": source, "sha256": sha256}))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    ensure!(seen == expected, "missing PGO workload IDs");
    Ok(cases)
}

fn read_binaries(path: &Path, root: &Path) -> anyhow::Result<BTreeMap<String, Value>> {
    let text = fs::read_to_string(path).context("failed to read PGO binary manifest")?;
    let mut lines = text.lines();
    ensure!(
        lines.next() == Some("variant\tbytes\tsha256"),
        "invalid PGO binary header"
    );
    let mut binaries = BTreeMap::new();
    for line in lines {
        let mut fields = line.split('\t');
        let variant = fields.next().context("missing PGO variant")?;
        ensure!(
            matches!(variant, "ordinary" | "instrumented" | "pgo"),
            "invalid PGO variant"
        );
        let bytes: u64 = fields.next().context("missing PGO binary size")?.parse()?;
        let sha256 = fields.next().context("missing PGO binary digest")?;
        ensure!(fields.next().is_none(), "extra PGO binary fields");
        let binary = root.join("bin").join(variant);
        ensure!(
            binary.canonicalize()? == binary,
            "PGO binary must not resolve through a symlink"
        );
        ensure!(
            bytes > 0 && fs::metadata(&binary)?.len() == bytes,
            "PGO binary size mismatch"
        );
        types::check_hash(&binary, sha256)?;
        let entry = json!({"path": binary, "sha256": sha256, "bytes": bytes});
        ensure!(
            binaries.insert(variant.to_owned(), entry).is_none(),
            "duplicate PGO variant"
        );
    }
    ensure!(
        binaries.contains_key("ordinary"),
        "ordinary PGO verifier is missing"
    );
    Ok(binaries)
}
