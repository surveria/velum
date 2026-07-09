use std::{
    collections::BTreeSet,
    env, fmt, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, bail};

use crate::{
    CaseRow, CorpusReport, CorpusStats, STATUS_FAILED, STATUS_PASSED,
    test262_external::source_label,
};

const BASELINE_CORPUS_NAME: &str = "Test262 expected-pass baseline";
const BASELINE_PATH: &str = "tests/corpora/test262/full-pass-baseline.txt";
const BASELINE_SCHEMA: &str = "# rsqjs-test262-pass-baseline-v1";
const PINNED_TEST262_COMMIT: &str = "64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1";
const UPDATE_BASELINE_ENV: &str = "RSQJS_TEST262_UPDATE_PASS_BASELINE";

pub fn verify_or_update(rows: &[CaseRow]) -> anyhow::Result<CorpusReport> {
    let current = passing_case_ids(rows);
    let expected = if update_requested() {
        write_baseline(&current)?;
        current.clone()
    } else {
        read_baseline()?
    };
    Ok(comparison_report(&expected, &current))
}

fn passing_case_ids(rows: &[CaseRow]) -> BTreeSet<String> {
    rows.iter()
        .filter(|row| row.status == STATUS_PASSED)
        .map(|row| row.case.clone())
        .collect()
}

fn comparison_report(expected: &BTreeSet<String>, current: &BTreeSet<String>) -> CorpusReport {
    let regressions = expected.difference(current).cloned().collect::<Vec<_>>();
    let improvements = current.difference(expected).cloned().collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(regressions.len().saturating_add(improvements.len()));
    rows.extend(regressions.iter().map(|case| {
        mismatch_row(
            case,
            "expected passing Test262 variant regressed or disappeared",
        )
    }));
    rows.extend(improvements.iter().map(|case| {
        mismatch_row(
            case,
            "new passing Test262 variant requires an explicit baseline refresh",
        )
    }));
    let failed = regressions.len().saturating_add(improvements.len());
    let passed = expected.intersection(current).count();
    CorpusReport {
        name: BASELINE_CORPUS_NAME,
        required: true,
        stats: CorpusStats {
            total: expected.union(current).count(),
            passed,
            failed,
            skipped: 0,
        },
        rows,
        skip_reasons: Vec::new(),
        feature_areas: Vec::new(),
        elapsed: Duration::ZERO,
    }
}

fn mismatch_row(case: &str, detail: &str) -> CaseRow {
    let path = case.split_once('#').map_or(case, |(path, _)| path);
    CaseRow {
        case: case.to_owned(),
        status: STATUS_FAILED.to_owned(),
        source: source_label(path),
        detail: detail.to_owned(),
        elapsed: Duration::ZERO,
    }
}

fn read_baseline() -> anyhow::Result<BTreeSet<String>> {
    let path = baseline_path()?;
    let text = fs::read_to_string(&path).with_context(|| {
        format!(
            "failed to read Test262 pass baseline '{}'; run with {UPDATE_BASELINE_ENV}=1",
            path.display()
        )
    })?;
    let mut lines = text.lines();
    let schema = lines.next().context("Test262 pass baseline is empty")?;
    if schema != BASELINE_SCHEMA {
        bail!("unsupported Test262 pass baseline schema '{schema}'");
    }
    let commit = lines
        .next()
        .context("Test262 pass baseline is missing its upstream commit")?;
    let expected_commit = format!("# test262_commit={PINNED_TEST262_COMMIT}");
    if commit != expected_commit {
        bail!(
            "Test262 pass baseline commit mismatch: expected '{expected_commit}', got '{commit}'"
        );
    }
    Ok(lines
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect())
}

fn write_baseline(cases: &BTreeSet<String>) -> anyhow::Result<()> {
    use fmt::Write as _;

    let path = baseline_path()?;
    let mut body = String::new();
    writeln!(&mut body, "{BASELINE_SCHEMA}").context("failed to render baseline schema")?;
    writeln!(&mut body, "# test262_commit={PINNED_TEST262_COMMIT}")
        .context("failed to render baseline commit")?;
    for case in cases {
        writeln!(&mut body, "{case}").context("failed to render Test262 baseline case")?;
    }
    fs::write(&path, body)
        .with_context(|| format!("failed to write Test262 pass baseline '{}'", path.display()))
}

fn baseline_path() -> anyhow::Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .context("runner manifest directory has no repository parent")?;
    Ok(repo_root.join(BASELINE_PATH))
}

fn update_requested() -> bool {
    env::var(UPDATE_BASELINE_ENV).is_ok_and(|value| {
        let value = value.trim();
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}
