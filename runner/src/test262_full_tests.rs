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
    ensure_usize(files.rows.len(), 1)?;
    let Some(row) = files.rows.first() else {
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
