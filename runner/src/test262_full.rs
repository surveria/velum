use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::Duration,
};

use anyhow::{Context as _, bail};

use super::{
    CaseRow, CorpusReport, CorpusStats, FeatureAreaStats, REASON_MATCHED, STATUS_FAILED,
    STATUS_PASSED, SkipReasonRow, feature_area_rows,
    test262_external::{
        MODE_NEGATIVE_PARSE, MODE_RUN, MODE_SKIP, ManifestCase, REASON_TEST262_DIR_MISSING,
        execute_manifest_case, execute_negative_parse_case, manifest_cases, source_label,
    },
    test262_metadata::{
        Test262CaseResult, Test262Outcome, execute_test262_path, test262_path_has_all_flags,
    },
    timing,
};

const FILE_CORPUS_NAME: &str = "Test262 file conformance";
const VARIANT_CORPUS_NAME: &str = "Test262 full corpus";
const TEST262_TEST_ROOT: &str = "test";
const TEST262_RUN_ALL_ENV: &str = "RSQJS_TEST262_RUN_ALL";
const TEST262_PATH_FILTER_ENV: &str = "RSQJS_TEST262_PATH_FILTER";
const TEST262_FLAG_FILTER_ENV: &str = "RSQJS_TEST262_FLAG_FILTER";
const MODULE_FIXTURE_MARKER: &str = "_FIXTURE";
const UNKNOWN_AREA: &str = "unknown";
const STORED_TIMING_ROW_LIMIT: usize = 200;

type FeatureStatsByArea = BTreeMap<String, FeatureAreaStats>;
#[derive(Debug)]
struct CorpusBuilder {
    rows: Vec<CaseRow>,
    timing_rows: Vec<CaseRow>,
    feature_stats: FeatureStatsByArea,
    stats: CorpusStats,
    elapsed: Duration,
}

impl CorpusBuilder {
    const fn new() -> Self {
        Self {
            rows: Vec::new(),
            timing_rows: Vec::new(),
            feature_stats: FeatureStatsByArea::new(),
            stats: CorpusStats {
                total: 0,
                passed: 0,
                failed: 0,
                skipped: 0,
            },
            elapsed: Duration::ZERO,
        }
    }

    fn into_report(mut self, name: &'static str) -> CorpusReport {
        self.rows.extend(self.timing_rows);
        CorpusReport {
            name,
            required: false,
            stats: self.stats,
            rows: self.rows,
            skip_reasons: Vec::new(),
            feature_areas: feature_area_rows(self.feature_stats.into_values().collect()),
            elapsed: self.elapsed,
        }
    }
}

pub fn run_reports(test262_dir: Option<&Path>) -> Vec<CorpusReport> {
    let Some(test262_dir) = test262_dir else {
        return vec![unavailable_report()];
    };

    match execute_full_corpus(test262_dir) {
        Ok(reports) => reports,
        Err(error) => vec![error_report(test262_dir, &error)],
    }
}

fn unavailable_report() -> CorpusReport {
    CorpusReport {
        name: VARIANT_CORPUS_NAME,
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
        feature_areas: Vec::new(),
        elapsed: Duration::ZERO,
    }
}

fn error_report(test262_dir: &Path, error: &anyhow::Error) -> CorpusReport {
    CorpusReport {
        name: VARIANT_CORPUS_NAME,
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
            elapsed: Duration::ZERO,
        }],
        skip_reasons: Vec::new(),
        feature_areas: Vec::new(),
        elapsed: Duration::ZERO,
    }
}

fn execute_full_corpus(test262_dir: &Path) -> anyhow::Result<Vec<CorpusReport>> {
    let discovered = discover_test_files(test262_dir)?;
    let test_paths = filtered_test_paths(test262_dir, &discovered)?;
    if should_run_all() {
        return Ok(execute_metadata_corpora(test262_dir, &test_paths));
    }
    Ok(vec![execute_manifest_corpus(test262_dir, &test_paths)?])
}

fn execute_metadata_corpora(test262_dir: &Path, test_paths: &[String]) -> Vec<CorpusReport> {
    let mut files = CorpusBuilder::new();
    let mut variants = CorpusBuilder::new();
    record_manifest_enabled_cases(&mut files.feature_stats);
    record_manifest_enabled_cases(&mut variants.feature_stats);

    for path in test_paths {
        run_discovered_case(test262_dir, path, &mut files, &mut variants);
    }

    vec![
        files.into_report(FILE_CORPUS_NAME),
        variants.into_report(VARIANT_CORPUS_NAME),
    ]
}

fn execute_manifest_corpus(
    test262_dir: &Path,
    test_paths: &[String],
) -> anyhow::Result<CorpusReport> {
    let timer = timing::RunTimer::start();
    let manifest = manifest_cases()?;
    let mut manifest_by_path = BTreeMap::<String, ManifestCase>::new();
    let mut feature_stats = FeatureStatsByArea::new();
    let mut rows = Vec::<CaseRow>::new();
    let mut stats = CorpusStats {
        total: test_paths.len(),
        passed: 0,
        failed: 0,
        skipped: 0,
    };

    for case in manifest {
        let path = case.path.clone();
        if manifest_by_path.insert(path.clone(), case).is_some() {
            stats.total = stats.total.saturating_add(1);
            stats.failed = stats.failed.saturating_add(1);
            feature_stats_for(&mut feature_stats, &path).record_failed();
            rows.push(CaseRow {
                case: "duplicate-manifest-path".to_owned(),
                status: STATUS_FAILED.to_owned(),
                source: "tests/corpora/test262/manifest.tsv".to_owned(),
                detail: "duplicate Test262 manifest path".to_owned(),
                elapsed: Duration::ZERO,
            });
        }
    }

    let discovered = test_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut skip_reasons = BTreeMap::<String, usize>::new();
    for path in test_paths {
        if let Some(case) = manifest_by_path.get(path) {
            run_enabled_case(
                test262_dir,
                case,
                &mut stats,
                &mut rows,
                &mut skip_reasons,
                &mut feature_stats,
            );
        } else {
            let reason = default_skip_reason(path);
            record_skip(&mut stats, &mut skip_reasons, reason.clone());
            feature_stats_for(&mut feature_stats, path).record_skipped(reason);
        }
    }
    for case in manifest_by_path.values() {
        if !discovered.contains(&case.path) {
            stats.total = stats.total.saturating_add(1);
            stats.failed = stats.failed.saturating_add(1);
            feature_stats_for(&mut feature_stats, &case.path).record_failed();
            rows.push(CaseRow {
                case: case.id.clone(),
                status: STATUS_FAILED.to_owned(),
                source: source_label(&case.path),
                detail: "manifest path is not present in pinned Test262 checkout".to_owned(),
                elapsed: Duration::ZERO,
            });
        }
    }

    Ok(CorpusReport {
        name: VARIANT_CORPUS_NAME,
        required: false,
        stats,
        rows,
        skip_reasons: skip_reason_rows(skip_reasons),
        feature_areas: feature_area_rows(feature_stats.into_values().collect()),
        elapsed: timer.elapsed(),
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

fn filtered_test_paths(test262_dir: &Path, test_paths: &[String]) -> anyhow::Result<Vec<String>> {
    let filter = Test262Filter::from_env();
    if filter.is_empty() {
        return Ok(test_paths.to_vec());
    }

    let mut filtered = Vec::new();
    for path in test_paths {
        if filter.matches_path(path)
            && test262_path_has_all_flags(test262_dir, path, &filter.flags)?
        {
            filtered.push(path.clone());
        }
    }
    Ok(filtered)
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
struct Test262Filter {
    path_fragments: Vec<String>,
    flags: Vec<String>,
}

impl Test262Filter {
    fn from_env() -> Self {
        Self {
            path_fragments: env_list(TEST262_PATH_FILTER_ENV),
            flags: env_list(TEST262_FLAG_FILTER_ENV),
        }
    }

    const fn is_empty(&self) -> bool {
        self.path_fragments.is_empty() && self.flags.is_empty()
    }

    fn matches_path(&self, path: &str) -> bool {
        self.path_fragments.is_empty()
            || self
                .path_fragments
                .iter()
                .any(|fragment| path.contains(fragment))
    }
}

fn env_list(name: &str) -> Vec<String> {
    let Some(value) = std::env::var_os(name) else {
        return Vec::new();
    };
    parse_env_list(&value.to_string_lossy())
}

fn parse_env_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn run_discovered_case(
    test262_dir: &Path,
    path: &str,
    files: &mut CorpusBuilder,
    variants: &mut CorpusBuilder,
) {
    let result = timing::timed(|| execute_test262_path(test262_dir, path));
    match result.value {
        Ok(results) => {
            files.elapsed = files.elapsed.saturating_add(result.elapsed);
            variants.elapsed = variants.elapsed.saturating_add(result.elapsed);
            record_file_result(path, &results, files);
            for result in results {
                record_discovered_result(path, result, variants);
            }
        }
        Err(error) => {
            let detail = error.to_string();
            files.elapsed = files.elapsed.saturating_add(result.elapsed);
            variants.elapsed = variants.elapsed.saturating_add(result.elapsed);
            record_failed_case(path, path, &detail, result.elapsed, files);
            record_failed_case(path, path, &detail, result.elapsed, variants);
        }
    }
}

fn record_file_result(path: &str, results: &[Test262CaseResult], files: &mut CorpusBuilder) {
    let mut failed_variants = Vec::<String>::new();
    for result in results {
        if matches!(result.outcome, Test262Outcome::Failed(_)) {
            failed_variants.push(variant_suffix(&result.id).to_owned());
        }
    }

    files.stats.total = files.stats.total.saturating_add(1);
    let elapsed = results
        .iter()
        .map(|result| result.elapsed)
        .fold(Duration::ZERO, Duration::saturating_add);
    if failed_variants.is_empty() {
        files.stats.passed = files.stats.passed.saturating_add(1);
        feature_stats_for(&mut files.feature_stats, path).record_passed();
        record_timing_row(
            files,
            CaseRow {
                case: path.to_owned(),
                status: STATUS_PASSED.to_owned(),
                source: source_label(path),
                detail: REASON_MATCHED.to_owned(),
                elapsed,
            },
        );
    } else {
        files.stats.failed = files.stats.failed.saturating_add(1);
        feature_stats_for(&mut files.feature_stats, path).record_failed();
        files.rows.push(CaseRow {
            case: path.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: source_label(path),
            detail: format!(
                "required Test262 variant(s) failed: {}",
                failed_variants.join(", ")
            ),
            elapsed,
        });
    }
}

fn record_discovered_result(path: &str, result: Test262CaseResult, variants: &mut CorpusBuilder) {
    variants.stats.total = variants.stats.total.saturating_add(1);
    match result.outcome {
        Test262Outcome::Passed => {
            variants.stats.passed = variants.stats.passed.saturating_add(1);
            feature_stats_for(&mut variants.feature_stats, path).record_passed();
            record_timing_row(
                variants,
                CaseRow {
                    case: result.id,
                    status: STATUS_PASSED.to_owned(),
                    source: source_label(path),
                    detail: REASON_MATCHED.to_owned(),
                    elapsed: result.elapsed,
                },
            );
        }
        Test262Outcome::Failed(detail) => {
            variants.stats.failed = variants.stats.failed.saturating_add(1);
            feature_stats_for(&mut variants.feature_stats, path).record_failed();
            variants.rows.push(CaseRow {
                case: result.id,
                status: STATUS_FAILED.to_owned(),
                source: source_label(path),
                detail,
                elapsed: result.elapsed,
            });
        }
    }
}

fn record_timing_row(corpus: &mut CorpusBuilder, row: CaseRow) {
    let rows = &mut corpus.timing_rows;
    rows.push(row);
    rows.sort_by_key(|row| std::cmp::Reverse(row.elapsed));
    if rows.len() > STORED_TIMING_ROW_LIMIT {
        rows.pop();
    }
}

fn record_failed_case(
    case: &str,
    path: &str,
    detail: &str,
    elapsed: Duration,
    corpus: &mut CorpusBuilder,
) {
    corpus.stats.total = corpus.stats.total.saturating_add(1);
    corpus.stats.failed = corpus.stats.failed.saturating_add(1);
    feature_stats_for(&mut corpus.feature_stats, path).record_failed();
    corpus.rows.push(CaseRow {
        case: case.to_owned(),
        status: STATUS_FAILED.to_owned(),
        source: source_label(path),
        detail: detail.to_owned(),
        elapsed,
    });
}

fn variant_suffix(case_id: &str) -> &str {
    let Some((_, variant)) = case_id.rsplit_once('#') else {
        return "unknown";
    };
    variant
}

fn run_enabled_case(
    test262_dir: &Path,
    case: &ManifestCase,
    stats: &mut CorpusStats,
    rows: &mut Vec<CaseRow>,
    skip_reasons: &mut BTreeMap<String, usize>,
    feature_stats: &mut FeatureStatsByArea,
) {
    if case.mode == MODE_SKIP {
        record_skip(stats, skip_reasons, case.reason.clone());
        feature_stats_for(feature_stats, &case.path).record_skipped(case.reason.clone());
        return;
    }
    feature_stats_for(feature_stats, &case.path).record_manifest_enabled();

    let result = timing::timed(|| {
        if case.mode == MODE_RUN {
            execute_manifest_case(test262_dir, case)
        } else if case.mode == MODE_NEGATIVE_PARSE {
            execute_negative_parse_case(test262_dir, case)
        } else {
            Err(anyhow::anyhow!("unknown manifest mode '{}'", case.mode))
        }
    });

    match result.value {
        Ok(()) => {
            stats.passed = stats.passed.saturating_add(1);
            feature_stats_for(feature_stats, &case.path).record_passed();
            rows.push(CaseRow {
                case: case.id.clone(),
                status: STATUS_PASSED.to_owned(),
                source: source_label(&case.path),
                detail: REASON_MATCHED.to_owned(),
                elapsed: result.elapsed,
            });
        }
        Err(error) => {
            stats.failed = stats.failed.saturating_add(1);
            feature_stats_for(feature_stats, &case.path).record_failed();
            rows.push(CaseRow {
                case: case.id.clone(),
                status: STATUS_FAILED.to_owned(),
                source: source_label(&case.path),
                detail: error.to_string(),
                elapsed: result.elapsed,
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

fn record_manifest_enabled_cases(feature_stats: &mut FeatureStatsByArea) {
    if let Ok(manifest) = manifest_cases() {
        for case in manifest {
            if case.mode != MODE_SKIP {
                feature_stats_for(feature_stats, &case.path).record_manifest_enabled();
            }
        }
    }
}

fn feature_stats_for<'stats>(
    stats: &'stats mut FeatureStatsByArea,
    path: &str,
) -> &'stats mut FeatureAreaStats {
    let area = test262_feature_area(path);
    stats
        .entry(area.clone())
        .or_insert_with(|| FeatureAreaStats::new(area))
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

fn test262_feature_area(path: &str) -> String {
    let mut parts = path.split('/');
    if parts.next() != Some(TEST262_TEST_ROOT) {
        return UNKNOWN_AREA.to_owned();
    }
    let Some(area) = parts.next() else {
        return UNKNOWN_AREA.to_owned();
    };
    let Some(feature) = parts.next() else {
        return area.to_owned();
    };
    format!("{area}/{feature}")
}

fn skip_reason_rows(reasons: BTreeMap<String, usize>) -> Vec<SkipReasonRow> {
    reasons
        .into_iter()
        .map(|(reason, skipped)| SkipReasonRow { skipped, reason })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{path::Path, time::Duration};

    use super::{
        CorpusBuilder, Test262CaseResult, Test262Filter, Test262Outcome, default_skip_reason,
        is_standalone_js_test_file, parse_env_list, record_file_result, test262_area,
        test262_feature_area,
    };

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
    fn extracts_test262_feature_area_from_test_path() -> TestResult {
        ensure_text(
            &test262_feature_area("test/language/statements/if/S12.js"),
            "language/statements",
        )?;
        ensure_text(
            &test262_feature_area("test/built-ins/Array/prototype/map/name.js"),
            "built-ins/Array",
        )?;
        ensure_text(
            &test262_feature_area("test/staging/sm/extensions/example.js"),
            "staging/sm",
        )?;
        ensure_text(&test262_feature_area("harness/assert.js"), "unknown")
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

    #[test]
    fn treats_empty_filter_as_no_filter() -> TestResult {
        ensure_bool(Test262Filter::default().is_empty())
    }

    #[test]
    fn matches_path_filter_fragments_as_any_match() -> TestResult {
        let filter = Test262Filter {
            path_fragments: vec![
                "test/built-ins/Promise".to_owned(),
                "test/language/statements/async-function".to_owned(),
            ],
            flags: Vec::new(),
        };
        ensure_bool(filter.matches_path("test/language/statements/async-function/basic.js"))?;
        ensure_bool(!filter.matches_path("test/language/statements/function/basic.js"))
    }

    #[test]
    fn parses_comma_separated_env_lists() -> TestResult {
        let items = parse_env_list(" async, generated ,,");
        ensure_texts(&items, &["async", "generated"])
    }

    #[test]
    fn collapses_passed_variants_to_one_passed_file() -> TestResult {
        let results = vec![
            passed_case("test/example.js#default"),
            passed_case("test/example.js#strict"),
        ];
        let mut files = CorpusBuilder::new();

        record_file_result("test/example.js", &results, &mut files);

        ensure_usize(files.stats.total, 1)?;
        ensure_usize(files.stats.passed, 1)?;
        ensure_usize(files.stats.failed, 0)?;
        ensure_usize(files.rows.len(), 0)?;
        ensure_usize(files.timing_rows.len(), 1)?;
        let Some(row) = files.timing_rows.first() else {
            return Err("expected passed file row".into());
        };
        ensure_text(&row.case, "test/example.js")?;
        ensure_text(&row.status, super::STATUS_PASSED)
    }

    #[test]
    fn collapses_failed_variant_to_one_failed_file() -> TestResult {
        let results = vec![
            passed_case("test/example.js#default"),
            failed_case("test/example.js#strict"),
        ];
        let mut files = CorpusBuilder::new();

        record_file_result("test/example.js", &results, &mut files);

        ensure_usize(files.stats.total, 1)?;
        ensure_usize(files.stats.passed, 0)?;
        ensure_usize(files.stats.failed, 1)?;
        let Some(row) = files.rows.first() else {
            return Err("expected failed file row".into());
        };
        ensure_text(&row.case, "test/example.js")?;
        ensure_text(&row.detail, "required Test262 variant(s) failed: strict")
    }

    fn passed_case(id: &str) -> Test262CaseResult {
        Test262CaseResult {
            id: id.to_owned(),
            outcome: Test262Outcome::Passed,
            elapsed: Duration::ZERO,
        }
    }

    fn failed_case(id: &str) -> Test262CaseResult {
        Test262CaseResult {
            id: id.to_owned(),
            outcome: Test262Outcome::Failed("failed".to_owned()),
            elapsed: Duration::ZERO,
        }
    }

    fn ensure_text(actual: &str, expected: &str) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected '{expected}', got '{actual}'").into())
    }

    fn ensure_texts(actual: &[String], expected: &[&str]) -> TestResult {
        if actual
            .iter()
            .zip(expected.iter())
            .all(|(left, right)| left == right)
            && actual.len() == expected.len()
        {
            return Ok(());
        }
        Err(format!("expected {expected:?}, got {actual:?}").into())
    }

    fn ensure_bool(value: bool) -> TestResult {
        if value {
            return Ok(());
        }
        Err("expected true".into())
    }

    fn ensure_usize(actual: usize, expected: usize) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected {expected}, got {actual}").into())
    }
}
