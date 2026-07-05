use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context as _, bail};

use super::{
    CaseRow, CorpusReport, CorpusStats, STATUS_FAILED, SkipReasonRow,
    test262_external::{
        MODE_NEGATIVE_PARSE, MODE_RUN, MODE_SKIP, ManifestCase, REASON_TEST262_DIR_MISSING,
        execute_manifest_case, execute_negative_parse_case, manifest_cases, source_label,
    },
    test262_metadata::{Test262CaseResult, Test262Outcome, execute_test262_path},
};

const CORPUS_NAME: &str = "Test262 full corpus";
const TEST262_TEST_ROOT: &str = "test";
const TEST262_RUN_ALL_ENV: &str = "RSQJS_TEST262_RUN_ALL";
const MODULE_FIXTURE_MARKER: &str = "_FIXTURE";
const UNKNOWN_AREA: &str = "unknown";

pub fn run(test262_dir: Option<&Path>) -> CorpusReport {
    let Some(test262_dir) = test262_dir else {
        return unavailable_report();
    };

    match execute_full_corpus(test262_dir) {
        Ok(report) => report,
        Err(error) => CorpusReport {
            name: CORPUS_NAME,
            required: false,
            stats: CorpusStats {
                total: 1,
                passed: 0,
                failed: 1,
                skipped: 0,
            },
            rows: vec![CaseRow {
                case: "test262-full-discovery".to_owned(),
                status: STATUS_FAILED.to_owned(),
                source: test262_dir.display().to_string(),
                detail: error.to_string(),
            }],
            skip_reasons: Vec::new(),
        },
    }
}

fn unavailable_report() -> CorpusReport {
    CorpusReport {
        name: CORPUS_NAME,
        required: false,
        stats: CorpusStats {
            total: 0,
            passed: 0,
            failed: 0,
            skipped: 0,
        },
        rows: Vec::new(),
        skip_reasons: vec![SkipReasonRow {
            skipped: 0,
            reason: REASON_TEST262_DIR_MISSING.to_owned(),
        }],
    }
}

fn execute_full_corpus(test262_dir: &Path) -> anyhow::Result<CorpusReport> {
    let test_paths = discover_test_files(test262_dir)?;
    if should_run_all() {
        return Ok(execute_metadata_corpus(test262_dir, &test_paths));
    }
    execute_manifest_corpus(test262_dir, &test_paths)
}

fn execute_metadata_corpus(test262_dir: &Path, test_paths: &[String]) -> CorpusReport {
    let mut rows = Vec::<CaseRow>::new();
    let mut stats = CorpusStats {
        total: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
    };

    for path in test_paths {
        run_discovered_case(test262_dir, path, &mut stats, &mut rows);
    }

    CorpusReport {
        name: CORPUS_NAME,
        required: false,
        stats,
        rows,
        skip_reasons: Vec::new(),
    }
}

fn execute_manifest_corpus(
    test262_dir: &Path,
    test_paths: &[String],
) -> anyhow::Result<CorpusReport> {
    let manifest = manifest_cases()?;
    let mut manifest_by_path = BTreeMap::<String, ManifestCase>::new();
    let mut rows = Vec::<CaseRow>::new();
    let mut stats = CorpusStats {
        total: test_paths.len(),
        passed: 0,
        failed: 0,
        skipped: 0,
    };

    for case in manifest {
        if manifest_by_path.insert(case.path.clone(), case).is_some() {
            stats.total = stats.total.saturating_add(1);
            stats.failed = stats.failed.saturating_add(1);
            rows.push(CaseRow {
                case: "duplicate-manifest-path".to_owned(),
                status: STATUS_FAILED.to_owned(),
                source: "tests/corpora/test262/manifest.tsv".to_owned(),
                detail: "duplicate Test262 manifest path".to_owned(),
            });
        }
    }

    let discovered = test_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut skip_reasons = BTreeMap::<String, usize>::new();
    for path in test_paths {
        if let Some(case) = manifest_by_path.get(path) {
            run_enabled_case(test262_dir, case, &mut stats, &mut rows, &mut skip_reasons);
        } else {
            record_skip(&mut stats, &mut skip_reasons, default_skip_reason(path));
        }
    }
    for case in manifest_by_path.values() {
        if !discovered.contains(&case.path) {
            stats.total = stats.total.saturating_add(1);
            stats.failed = stats.failed.saturating_add(1);
            rows.push(CaseRow {
                case: case.id.clone(),
                status: STATUS_FAILED.to_owned(),
                source: source_label(&case.path),
                detail: "manifest path is not present in pinned Test262 checkout".to_owned(),
            });
        }
    }

    Ok(CorpusReport {
        name: CORPUS_NAME,
        required: false,
        stats,
        rows,
        skip_reasons: skip_reason_rows(skip_reasons),
    })
}

fn should_run_all() -> bool {
    std::env::var(TEST262_RUN_ALL_ENV).is_ok_and(|value| is_run_all_value(&value))
}

fn is_run_all_value(value: &str) -> bool {
    let value = value.trim();
    value == "1"
        || value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("all")
        || value.eq_ignore_ascii_case("full")
}

fn run_discovered_case(
    test262_dir: &Path,
    path: &str,
    stats: &mut CorpusStats,
    rows: &mut Vec<CaseRow>,
) {
    match execute_test262_path(test262_dir, path) {
        Ok(results) => {
            for result in results {
                record_discovered_result(path, result, stats, rows);
            }
        }
        Err(error) => {
            stats.total = stats.total.saturating_add(1);
            stats.failed = stats.failed.saturating_add(1);
            rows.push(CaseRow {
                case: path.to_owned(),
                status: STATUS_FAILED.to_owned(),
                source: source_label(path),
                detail: error.to_string(),
            });
        }
    }
}

fn record_discovered_result(
    path: &str,
    result: Test262CaseResult,
    stats: &mut CorpusStats,
    rows: &mut Vec<CaseRow>,
) {
    stats.total = stats.total.saturating_add(1);
    match result.outcome {
        Test262Outcome::Passed => {
            stats.passed = stats.passed.saturating_add(1);
        }
        Test262Outcome::Failed(detail) => {
            stats.failed = stats.failed.saturating_add(1);
            rows.push(CaseRow {
                case: result.id,
                status: STATUS_FAILED.to_owned(),
                source: source_label(path),
                detail,
            });
        }
    }
}

fn run_enabled_case(
    test262_dir: &Path,
    case: &ManifestCase,
    stats: &mut CorpusStats,
    rows: &mut Vec<CaseRow>,
    skip_reasons: &mut BTreeMap<String, usize>,
) {
    if case.mode == MODE_SKIP {
        record_skip(stats, skip_reasons, case.reason.clone());
        return;
    }

    let result = if case.mode == MODE_RUN {
        execute_manifest_case(test262_dir, case)
    } else if case.mode == MODE_NEGATIVE_PARSE {
        execute_negative_parse_case(test262_dir, case)
    } else {
        Err(anyhow::anyhow!("unknown manifest mode '{}'", case.mode))
    };

    match result {
        Ok(()) => {
            stats.passed = stats.passed.saturating_add(1);
        }
        Err(error) => {
            stats.failed = stats.failed.saturating_add(1);
            rows.push(CaseRow {
                case: case.id.clone(),
                status: STATUS_FAILED.to_owned(),
                source: source_label(&case.path),
                detail: error.to_string(),
            });
        }
    }
}

fn record_skip(
    stats: &mut CorpusStats,
    skip_reasons: &mut BTreeMap<String, usize>,
    reason: String,
) {
    stats.skipped = stats.skipped.saturating_add(1);
    let count = skip_reasons.entry(reason).or_default();
    *count = count.saturating_add(1);
}

fn discover_test_files(test262_dir: &Path) -> anyhow::Result<Vec<String>> {
    let root = test262_dir.join(TEST262_TEST_ROOT);
    if !root.is_dir() {
        bail!(
            "Test262 checkout '{}' does not contain a '{}' directory",
            test262_dir.display(),
            TEST262_TEST_ROOT
        );
    }

    let mut paths = Vec::new();
    collect_js_files(&root, test262_dir, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_js_files(
    directory: &Path,
    test262_dir: &Path,
    paths: &mut Vec<String>,
) -> anyhow::Result<()> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("failed to read directory '{}'", directory.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to inspect directory '{}'", directory.display()))?;
    entries.sort_by_key(std::fs::DirEntry::path);

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;
        if file_type.is_dir() {
            collect_js_files(&path, test262_dir, paths)?;
        } else if is_standalone_js_test_file(&path) {
            paths.push(relative_test_path(test262_dir, &path)?);
        }
    }
    Ok(())
}

fn is_standalone_js_test_file(path: &Path) -> bool {
    is_js_file(path) && !is_module_fixture(path)
}

fn is_js_file(path: &Path) -> bool {
    path.extension().is_some_and(|extension| extension == "js")
}

fn is_module_fixture(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(MODULE_FIXTURE_MARKER))
}

fn relative_test_path(test262_dir: &Path, path: &Path) -> anyhow::Result<String> {
    let relative = path
        .strip_prefix(test262_dir)
        .with_context(|| format!("failed to relativize '{}'", path.display()))?;
    path_to_slash_string(relative)
}

fn path_to_slash_string(path: &Path) -> anyhow::Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Some(part) = component.as_os_str().to_str() else {
            bail!("Test262 path '{}' is not valid UTF-8", path.display());
        };
        parts.push(part.to_owned());
    }
    Ok(parts.join("/"))
}

fn default_skip_reason(path: &str) -> String {
    let area = test262_area(path);
    format!("not enabled yet: Test262 {area} cases are outside the active manifest")
}

fn test262_area(path: &str) -> &str {
    let mut parts = path.split('/');
    if parts.next() != Some(TEST262_TEST_ROOT) {
        return UNKNOWN_AREA;
    }
    if let Some(area) = parts.next() {
        return area;
    }
    UNKNOWN_AREA
}

fn skip_reason_rows(reasons: BTreeMap<String, usize>) -> Vec<SkipReasonRow> {
    reasons
        .into_iter()
        .map(|(reason, skipped)| SkipReasonRow { skipped, reason })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{default_skip_reason, is_standalone_js_test_file, test262_area};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn extracts_test262_area_from_test_path() -> TestResult {
        ensure_text(
            test262_area("test/language/statements/if/S12.js"),
            "language",
        )?;
        ensure_text(
            test262_area("test/built-ins/Array/prototype/map/name.js"),
            "built-ins",
        )?;
        ensure_text(test262_area("harness/assert.js"), "unknown")
    }

    #[test]
    fn groups_default_skip_reason_by_area() -> TestResult {
        ensure_text(
            &default_skip_reason("test/language/statements/if/S12.js"),
            "not enabled yet: Test262 language cases are outside the active manifest",
        )
    }

    #[test]
    fn rejects_module_fixture_files_as_standalone_tests() -> TestResult {
        ensure_bool(!is_standalone_js_test_file(Path::new(
            "test/language/module-code/dep_FIXTURE.js",
        )))?;
        ensure_bool(is_standalone_js_test_file(Path::new(
            "test/language/module-code/import-default.js",
        )))
    }

    fn ensure_text(actual: &str, expected: &str) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected '{expected}', got '{actual}'").into())
    }

    fn ensure_bool(value: bool) -> TestResult {
        if value {
            return Ok(());
        }
        Err("expected true".into())
    }
}
