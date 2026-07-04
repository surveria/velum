use std::{fs, path::Path};

use anyhow::{Context as _, bail};
use rs_quickjs::Runtime;

use super::{CaseRow, STATUS_FAILED, STATUS_PASSED, STATUS_SKIPPED};

const MANIFEST_PATH: &str = "tests/corpora/test262/manifest.tsv";
const REASON_TEST262_DIR_MISSING: &str =
    "set RSQJS_TEST262_DIR or enable scripts/prepare-test262.sh";
const REASON_UPSTREAM_MATCHED: &str = "official upstream Test262 case passed";
const MODE_RUN: &str = "run";
const MODE_SKIP: &str = "skip";
const COLUMN_COUNT: usize = 4;

#[derive(Debug)]
struct ManifestCase {
    id: String,
    path: String,
    mode: String,
    reason: String,
}

pub fn run(test262_dir: Option<&Path>) -> Vec<CaseRow> {
    match manifest_cases() {
        Ok(cases) => cases
            .iter()
            .map(|case| run_manifest_case(case, test262_dir))
            .collect(),
        Err(error) => vec![CaseRow {
            case: "manifest".to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: MANIFEST_PATH.to_owned(),
            detail: error.to_string(),
        }],
    }
}

fn manifest_cases() -> anyhow::Result<Vec<ManifestCase>> {
    let manifest = fs::read_to_string(MANIFEST_PATH)
        .with_context(|| format!("failed to read Test262 manifest '{MANIFEST_PATH}'"))?;
    manifest
        .lines()
        .enumerate()
        .filter(|(_, line)| is_case_line(line))
        .map(parse_manifest_line)
        .collect()
}

fn is_case_line(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && !trimmed.starts_with('#')
}

fn parse_manifest_line((index, line): (usize, &str)) -> anyhow::Result<ManifestCase> {
    let mut columns = line.split('\t');
    let id = next_column(&mut columns, index, "case id")?;
    let path = next_column(&mut columns, index, "path")?;
    let mode = next_column(&mut columns, index, "mode")?;
    let reason = next_column(&mut columns, index, "reason")?;
    if columns.next().is_some() {
        bail!(
            "manifest line {} has more than {} columns",
            display_line(index),
            COLUMN_COUNT
        );
    }
    Ok(ManifestCase {
        id: id.to_owned(),
        path: path.to_owned(),
        mode: mode.to_owned(),
        reason: reason.to_owned(),
    })
}

fn next_column<'a>(
    columns: &mut impl Iterator<Item = &'a str>,
    index: usize,
    name: &str,
) -> anyhow::Result<&'a str> {
    let Some(value) = columns.next() else {
        bail!("manifest line {} is missing {name}", display_line(index));
    };
    if value.is_empty() {
        bail!("manifest line {} has empty {name}", display_line(index));
    }
    Ok(value)
}

const fn display_line(index: usize) -> usize {
    index.saturating_add(1)
}

fn run_manifest_case(case: &ManifestCase, test262_dir: Option<&Path>) -> CaseRow {
    if case.mode == MODE_SKIP {
        return skipped(case, &case.reason);
    }
    if case.mode != MODE_RUN {
        return failed(case, format!("unknown manifest mode '{}'", case.mode));
    }

    let Some(test262_dir) = test262_dir else {
        return skipped(case, REASON_TEST262_DIR_MISSING);
    };

    match execute_manifest_case(test262_dir, case) {
        Ok(()) => CaseRow {
            case: case.id.clone(),
            status: STATUS_PASSED.to_owned(),
            source: source_label(&case.path),
            detail: REASON_UPSTREAM_MATCHED.to_owned(),
        },
        Err(error) => failed(case, error.to_string()),
    }
}

fn execute_manifest_case(test262_dir: &Path, case: &ManifestCase) -> anyhow::Result<()> {
    let path = test262_dir.join(&case.path);
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read upstream Test262 case '{}'", case.path))?;
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context
        .eval(&source)
        .with_context(|| format!("upstream Test262 case '{}' failed", case.id))?;
    if !context.output().is_empty() {
        bail!("upstream Test262 case '{}' produced host output", case.id);
    }
    Ok(())
}

fn skipped(case: &ManifestCase, detail: &str) -> CaseRow {
    CaseRow {
        case: case.id.clone(),
        status: STATUS_SKIPPED.to_owned(),
        source: source_label(&case.path),
        detail: detail.to_owned(),
    }
}

fn failed(case: &ManifestCase, detail: String) -> CaseRow {
    CaseRow {
        case: case.id.clone(),
        status: STATUS_FAILED.to_owned(),
        source: source_label(&case.path),
        detail,
    }
}

fn source_label(path: &str) -> String {
    format!("test262:{path}")
}
