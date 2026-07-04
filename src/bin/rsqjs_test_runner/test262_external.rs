use std::{fs, path::Path};

use anyhow::{Context as _, bail};
use rs_quickjs::{Error, Runtime};

use super::{CaseRow, STATUS_FAILED, STATUS_PASSED, STATUS_SKIPPED};

const MANIFEST_PATH: &str = "tests/corpora/test262/manifest.tsv";
const REASON_TEST262_DIR_MISSING: &str =
    "set RSQJS_TEST262_DIR or enable scripts/prepare-test262.sh";
const REASON_UPSTREAM_MATCHED: &str = "official upstream Test262 case passed";
const REASON_NEGATIVE_PARSE_MATCHED: &str = "official upstream negative parse case failed to parse";
const MODE_RUN: &str = "run";
const MODE_SKIP: &str = "skip";
const MODE_NEGATIVE_PARSE: &str = "negative-parse";
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
    let Some(test262_dir) = test262_dir else {
        return skipped(case, REASON_TEST262_DIR_MISSING);
    };

    if case.mode == MODE_RUN {
        return match execute_manifest_case(test262_dir, case) {
            Ok(()) => passed(case, REASON_UPSTREAM_MATCHED),
            Err(error) => failed(case, error.to_string()),
        };
    }
    if case.mode == MODE_NEGATIVE_PARSE {
        return match execute_negative_parse_case(test262_dir, case) {
            Ok(()) => passed(case, REASON_NEGATIVE_PARSE_MATCHED),
            Err(error) => failed(case, error.to_string()),
        };
    }

    failed(case, format!("unknown manifest mode '{}'", case.mode))
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

fn execute_negative_parse_case(test262_dir: &Path, case: &ManifestCase) -> anyhow::Result<()> {
    let path = test262_dir.join(&case.path);
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read upstream Test262 case '{}'", case.path))?;
    ensure_negative_parse_failure(case, eval_source(&source))
}

fn eval_source(source: &str) -> rs_quickjs::Result<()> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source).map(|_| ())
}

fn ensure_negative_parse_failure(
    case: &ManifestCase,
    result: rs_quickjs::Result<()>,
) -> anyhow::Result<()> {
    match result {
        Ok(()) => bail!(
            "upstream negative parse case '{}' unexpectedly evaluated successfully",
            case.id
        ),
        Err(Error::Lex { .. } | Error::Parse { .. }) => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "upstream negative parse case '{}' failed at runtime",
                case.id
            )
        }),
    }
}

fn passed(case: &ManifestCase, detail: &str) -> CaseRow {
    CaseRow {
        case: case.id.clone(),
        status: STATUS_PASSED.to_owned(),
        source: source_label(&case.path),
        detail: detail.to_owned(),
    }
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

#[cfg(test)]
mod tests {
    use rs_quickjs::Error;

    use super::{ManifestCase, ensure_negative_parse_failure};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn accepts_lex_or_parse_error_for_negative_parse_case() -> TestResult {
        let case = manifest_case();
        ensure_negative_parse_failure(
            &case,
            Err(Error::Lex {
                message: "bad token".to_owned(),
                offset: 0,
            }),
        )?;
        ensure_negative_parse_failure(
            &case,
            Err(Error::Parse {
                message: "bad syntax".to_owned(),
                offset: 0,
            }),
        )?;
        Ok(())
    }

    #[test]
    fn rejects_success_for_negative_parse_case() -> TestResult {
        let case = manifest_case();
        let Err(error) = ensure_negative_parse_failure(&case, Ok(())) else {
            return Err("expected successful evaluation to fail negative parse case".into());
        };
        ensure_text_contains(&error.to_string(), "unexpectedly evaluated successfully")
    }

    #[test]
    fn rejects_runtime_error_for_negative_parse_case() -> TestResult {
        let case = manifest_case();
        let Err(error) = ensure_negative_parse_failure(
            &case,
            Err(Error::Runtime {
                message: "runtime failure".to_owned(),
            }),
        ) else {
            return Err("expected runtime error to fail negative parse case".into());
        };
        ensure_text_contains(&error.to_string(), "failed at runtime")
    }

    fn manifest_case() -> ManifestCase {
        ManifestCase {
            id: "negative-smoke".to_owned(),
            path: "test/language/example.js".to_owned(),
            mode: "negative-parse".to_owned(),
            reason: "test".to_owned(),
        }
    }

    fn ensure_text_contains(text: &str, expected: &str) -> TestResult {
        if text.contains(expected) {
            return Ok(());
        }
        Err(format!("expected '{text}' to contain '{expected}'").into())
    }
}
