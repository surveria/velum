use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::Path,
};

use anyhow::{Context as _, Result, ensure};
use serde::Serialize;

use super::{
    evidence::collect_report,
    types::{
        CASE_SUFFIXES, Checksum, Experiment, PerformanceEvidence, VARIANTS, VerifiedReport,
        check_hash, label_path, read_json, sha256, write_json,
    },
};

#[derive(Serialize)]
struct Comparison {
    round: u32,
    case: String,
    ordinary_median_ns: u64,
    pgo_median_ns: u64,
    ordinary_over_pgo_speedup: f64,
    pgo_time_change_percent: f64,
    ordinary_cv_permille: u32,
    pgo_cv_permille: u32,
}

#[derive(Serialize)]
struct RoundMean {
    round: u32,
    ordinary_over_pgo_geomean: f64,
}

#[derive(Serialize)]
struct Summary<'a> {
    schema_version: u32,
    commit: &'a str,
    tree: &'a str,
    preset: &'a str,
    cpu_affinity: String,
    profile_sha256: String,
    rows: Vec<Comparison>,
    round_geomeans: Vec<RoundMean>,
    limitations: [&'static str; 4],
}

pub fn summarize(run: &Path) -> Result<()> {
    let experiment = Experiment::load(run)?;
    for variant in VARIANTS {
        experiment.binary(variant)?;
    }
    let profile_sha256 = checked_profile(run)?;
    check_execution_order(run, experiment.rounds)?;
    let mut affinity = None;
    for suffix in CASE_SUFFIXES {
        let id = format!("representative_{suffix}");
        let label = format!("training-{id}");
        revalidate(
            run,
            &experiment,
            "instrumented",
            &label,
            Some(&id),
            &mut affinity,
        )?;
    }
    let mut baseline = BTreeMap::new();
    let mut rows = Vec::new();
    let mut round_geomeans = Vec::new();
    for round in 1..=experiment.rounds {
        let mut log_speedups = 0.0;
        for suffix in CASE_SUFFIXES {
            let case = format!("holdout_{suffix}");
            let ordinary = holdout(run, &experiment, round, "ordinary", &case, &mut affinity)?;
            let pgo = holdout(run, &experiment, round, "pgo", &case, &mut affinity)?;
            check_checksum(&mut baseline, &case, &ordinary.checksum)?;
            check_checksum(&mut baseline, &case, &pgo.checksum)?;
            let before = duration_float(ordinary.engine_ns)?;
            let after = duration_float(pgo.engine_ns)?;
            let speedup = before / after;
            ensure!(
                speedup.is_finite() && speedup > 0.0,
                "invalid paired speedup"
            );
            log_speedups += speedup.ln();
            rows.push(Comparison {
                round,
                case,
                ordinary_median_ns: ordinary.engine_ns,
                pgo_median_ns: pgo.engine_ns,
                ordinary_over_pgo_speedup: speedup,
                pgo_time_change_percent: 100.0 * (after / before - 1.0),
                ordinary_cv_permille: ordinary.engine_cv_permille,
                pgo_cv_permille: pgo.engine_cv_permille,
            });
        }
        for variant in ["ordinary", "pgo"] {
            let label = format!("round-{round}-{variant}-memory");
            revalidate(run, &experiment, variant, &label, None, &mut affinity)?;
        }
        round_geomeans.push(RoundMean {
            round,
            ordinary_over_pgo_geomean: (log_speedups
                / f64::from(u32::try_from(CASE_SUFFIXES.len())?))
            .exp(),
        });
    }
    let summary = Summary {
        schema_version: 1,
        commit: &experiment.commit,
        tree: &experiment.tree,
        preset: &experiment.preset,
        cpu_affinity: affinity.context("summary has no verified CPU affinity")?,
        profile_sha256,
        rows,
        round_geomeans,
        limitations: [
            "Observed paired timings are descriptive, not a significance test or adoption decision.",
            "Speedup above one compares absolute Velum execution times, not Velum-to-QuickJS ratios.",
            "ELF sizes describe the complete runner. Memory reports retain their separate physical and logical scopes.",
            "Training weights follow observed execution counts. Results apply only to this preset, toolchain, host and prepared cohort.",
        ],
    };
    let tsv = comparison_tsv(&summary.rows)?;
    write_json(&run.join("holdout-comparison.json"), &summary)?;
    fs::write(run.join("holdout-comparison.tsv"), tsv)
        .context("failed to write paired PGO comparison")
}

fn revalidate(
    run: &Path,
    experiment: &Experiment,
    variant: &str,
    label: &str,
    case: Option<&str>,
    affinity: &mut Option<String>,
) -> Result<VerifiedReport> {
    let kind = if case.is_some() {
        "performance"
    } else {
        "memory"
    };
    let saved: VerifiedReport = read_json(&label_path(run, label, ".verified.json")?)?;
    let actual = collect_report(run, experiment, variant, label, kind, case)?;
    ensure!(
        saved == actual,
        "verified report or sidecar identity changed: {label}"
    );
    if let Some(expected) = affinity {
        ensure!(
            *expected == actual.cpu_affinity,
            "CPU affinity drift across PGO reports"
        );
    } else {
        *affinity = Some(actual.cpu_affinity.clone());
    }
    Ok(actual)
}

fn holdout(
    run: &Path,
    experiment: &Experiment,
    round: u32,
    variant: &str,
    case: &str,
    affinity: &mut Option<String>,
) -> Result<PerformanceEvidence> {
    let label = format!("round-{round}-{variant}-{case}");
    revalidate(run, experiment, variant, &label, Some(case), affinity)?
        .performance
        .context("holdout has no performance evidence")
}

fn check_checksum(
    baseline: &mut BTreeMap<String, Checksum>,
    case: &str,
    checksum: &Checksum,
) -> Result<()> {
    if let Some(expected) = baseline.get(case) {
        ensure!(
            expected == checksum,
            "ordinary/PGO or cross-round checksum mismatch: {case}"
        );
    } else {
        ensure!(
            baseline.insert(case.to_owned(), checksum.clone()).is_none(),
            "duplicate initial checksum"
        );
    }
    Ok(())
}

fn duration_float(value: u64) -> Result<f64> {
    // Decimal conversion avoids silently truncating u64 before ratio calculation.
    let result: f64 = value.to_string().parse().context("invalid timing value")?;
    ensure!(
        result.is_finite() && result > 0.0,
        "timing must be positive and finite"
    );
    Ok(result)
}

fn checked_profile(run: &Path) -> Result<String> {
    let profile = run.join("merged.profdata");
    let text =
        fs::read_to_string(run.join("profile.sha256")).context("missing frozen profile hash")?;
    ensure!(
        text.lines().count() == 1,
        "profile hash manifest must contain exactly one record"
    );
    let (digest, name) = text
        .trim_end()
        .split_once("  ")
        .context("invalid profile hash manifest")?;
    ensure!(
        Path::new(name) == profile,
        "profile hash manifest targets another profile"
    );
    ensure!(
        fs::metadata(&profile)?.len() > 0,
        "empty merged PGO profile"
    );
    check_hash(&profile, digest)?;
    sha256(&profile)
}

fn check_execution_order(run: &Path, rounds: u32) -> Result<()> {
    let mut expected = vec!["source-export".to_owned(), "source-extract".to_owned()];
    build_steps(&mut expected, "ordinary");
    build_steps(&mut expected, "instrumented");
    for suffix in CASE_SUFFIXES {
        lane_steps(&mut expected, &format!("training-representative_{suffix}"));
    }
    expected.extend(["profile-merge", "profile-show", "profile-verify"].map(str::to_owned));
    build_steps(&mut expected, "pgo");
    expected.extend(["pgo-profile-integrity", "pgo-diagnostics"].map(str::to_owned));
    for round in 1..=rounds {
        let variants = if round % 2 == 1 {
            ["ordinary", "pgo"]
        } else {
            ["pgo", "ordinary"]
        };
        for variant in variants {
            for suffix in CASE_SUFFIXES {
                lane_steps(
                    &mut expected,
                    &format!("round-{round}-{variant}-holdout_{suffix}"),
                );
            }
            lane_steps(&mut expected, &format!("round-{round}-{variant}-memory"));
        }
    }
    expected.extend(
        [
            "final-source",
            "final-layout",
            "final-tools",
            "final-profile-integrity",
        ]
        .map(str::to_owned),
    );
    let text = fs::read_to_string(run.join("steps.tsv")).context("missing PGO execution record")?;
    let mut lines = text.lines();
    ensure!(
        lines.next() == Some("step\texit_code\twall_seconds\tcommand\tstdout\tstderr\ttime"),
        "invalid PGO execution record header"
    );
    let mut actual = Vec::new();
    let mut steps = BTreeSet::new();
    for line in lines {
        let fields: Vec<_> = line.split('\t').collect();
        ensure!(fields.len() == 7, "malformed PGO execution record");
        let name = *fields.first().context("missing PGO step name")?;
        ensure!(steps.insert(name), "duplicate PGO step {name}");
        ensure!(fields.get(1) == Some(&"0"), "failed PGO step {name}");
        ensure!(
            fields
                .get(2)
                .is_some_and(|value| value.parse::<u64>().is_ok()),
            "invalid PGO step duration {name}"
        );
        actual.push(name);
    }
    // The live summary runs before its own step is recorded. An offline rerun
    // may include that one successful final record, but no extra execution.
    let completed_summary = actual.last() == Some(&"compare-holdouts");
    if completed_summary {
        ensure!(actual.pop().is_some(), "missing final PGO summary record");
    }
    ensure!(
        actual == expected,
        "execution steps differ from the complete frozen PGO AB/BA protocol"
    );
    check_completion(run, completed_summary)?;
    Ok(())
}

fn check_completion(run: &Path, completed_summary: bool) -> Result<()> {
    let path = run.join("result.txt");
    if !path.exists() {
        ensure!(
            !completed_summary,
            "completed PGO summary is missing orchestration status"
        );
        return Ok(());
    }
    let text = fs::read_to_string(&path).context("failed to read PGO completion status")?;
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .context("malformed PGO completion status")?;
        ensure!(
            fields.insert(key, value).is_none(),
            "duplicate PGO completion status field {key}"
        );
    }
    ensure!(
        completed_summary
            && fields.get("status") == Some(&"complete-needs-review")
            && fields.get("exit_code") == Some(&"0"),
        "PGO orchestration is incomplete or failed despite report artifacts"
    );
    Ok(())
}

fn build_steps(steps: &mut Vec<String>, variant: &str) {
    steps.extend(
        [
            "source", "layout", "tools", "clean", "build", "sections", "manifest",
        ]
        .map(|suffix| format!("{variant}-{suffix}")),
    );
}

fn lane_steps(steps: &mut Vec<String>, label: &str) {
    steps.extend([
        format!("{label}-binary"),
        label.to_owned(),
        format!("{label}-verify"),
    ]);
}

fn comparison_tsv(rows: &[Comparison]) -> Result<String> {
    let mut text = "round\tcase\tordinary_median_ns\tpgo_median_ns\tordinary_over_pgo_speedup\tpgo_time_change_percent\tordinary_cv_permille\tpgo_cv_permille\n".to_owned();
    for row in rows {
        writeln!(
            text,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            row.round,
            row.case,
            row.ordinary_median_ns,
            row.pgo_median_ns,
            row.ordinary_over_pgo_speedup,
            row.pgo_time_change_percent,
            row.ordinary_cv_permille,
            row.pgo_cv_permille
        )?;
    }
    Ok(text)
}
