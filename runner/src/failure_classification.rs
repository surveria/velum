use std::collections::BTreeMap;

use anyhow::Context as _;
use tabled::{Table, Tabled};

use super::{
    report_schema::{
        CaseRecord, CaseStatus, DiagnosticGroup, FailureCategorySummary, FailureDiagnostics,
    },
    report_text,
};

const CATEGORY_ASYNC: &str = "async protocol";
const CATEGORY_HARNESS: &str = "harness include";
const CATEGORY_LEXER: &str = "lexer";
const CATEGORY_METADATA: &str = "metadata";
const CATEGORY_NEGATIVE: &str = "negative metadata";
const CATEGORY_OTHER: &str = "other";
const CATEGORY_PARSER: &str = "parser";
const CATEGORY_RESOURCE_LIMIT: &str = "resource limit";
const CATEGORY_RUNTIME: &str = "runtime";
const DEFAULT_VARIANT: &str = "unknown";
const DETAIL_ASSERT_CONSTRUCTOR_PREFIX: &str = "assert.throws error constructor '";
const DETAIL_ASSERT_CONSTRUCTOR_SUFFIX: &str = "' is not supported";
const DETAIL_ASYNC_FAILURE: &str = "signaled failure";
const DETAIL_ASYNC_PREFIX: &str = "upstream async Test262 case";
const DETAIL_HARNESS_PREFIX: &str = "Test262 harness include";
const DETAIL_LEXER: &str = "lexer error";
const DETAIL_METADATA: &str = "metadata";
const DETAIL_NEGATIVE: &str = "negative";
const DETAIL_PARSER: &str = "parser error";
const DETAIL_REFERENCE_PREFIX: &str = "ReferenceError: '";
const DETAIL_REFERENCE_SUFFIX: &str = "' is not defined";
const DETAIL_RESOURCE_LIMIT: &str = "limit exceeded";
const DETAIL_RUNTIME: &str = "runtime error";
const HINT_ASSERT_CONSTRUCTOR_PREFIX: &str = "unsupported error constructor: ";
const HINT_EXPECTED_EXPRESSION: &str = "parser: expected expression";
const HINT_EXPECTED_TOKEN_PREFIX: &str = "parser: expected ";
const HINT_LEXER_UNEXPECTED_CHARACTER: &str = "lexer: unexpected character";
const HINT_LEXER_UNSUPPORTED_ESCAPE: &str = "lexer: unsupported escape sequence";
const HINT_MISSING_BINDING_PREFIX: &str = "missing binding: ";
const HINT_NEGATIVE_PHASE_PREFIX: &str = "unsupported negative phase: ";
const HINT_UNEXPECTED_CHARACTER: &str = "unexpected character";
const HINT_UNSUPPORTED_ESCAPE: &str = "unsupported escape sequence";
const SECTION_TITLE: &str = "### Failure Classification";
const SUBTITLE_CATEGORIES: &str = "#### Error Categories";
const TEST262_SOURCE_PREFIX: &str = "test262:";

#[derive(Debug, Clone)]
struct DiagnosticAccumulator {
    count: u64,
    representative: CaseRecord,
}

#[derive(Debug, Tabled)]
struct DiagnosticCategoryRow {
    category: String,
    failed: u64,
}

#[derive(Debug, Tabled)]
struct DiagnosticGroupRow {
    failed: u64,
    feature_area: String,
    category: String,
    reason: String,
    representative_case: String,
    representative_source: String,
    detail: String,
}

#[must_use]
pub fn diagnostic_sections(diagnostics: &FailureDiagnostics) -> Vec<String> {
    if diagnostics.total_failed == 0 {
        return Vec::new();
    }
    let mut sections = vec![
        SECTION_TITLE.to_owned(),
        String::new(),
        format!(
            "{} grouped diagnostics represent {} of {} failures; {} additional groups are omitted.",
            diagnostics.groups.len(),
            diagnostics.represented_failed,
            diagnostics.total_failed,
            diagnostics.omitted_groups,
        ),
        String::new(),
        SUBTITLE_CATEGORIES.to_owned(),
        String::new(),
        fenced_table(&Table::new(diagnostics.categories.iter().map(|category| {
            DiagnosticCategoryRow {
                category: category.category.clone(),
                failed: category.failed,
            }
        }))),
        String::new(),
        "#### Actionable Groups".to_owned(),
        String::new(),
    ];
    sections.push(fenced_table(&Table::new(diagnostics.groups.iter().map(
        |group| DiagnosticGroupRow {
            failed: group.count,
            feature_area: group.feature_area.clone(),
            category: group.category.clone(),
            reason: group.reason.clone(),
            representative_case: group.representative_case.clone(),
            representative_source: group.representative_source.clone(),
            detail: group.detail.clone(),
        },
    ))));
    sections.push(String::new());
    sections
}

pub fn diagnostic_groups(rows: &[CaseRecord]) -> anyhow::Result<Option<FailureDiagnostics>> {
    let mut groups = BTreeMap::<(String, String, String), DiagnosticAccumulator>::new();
    let mut category_counts = BTreeMap::<String, u64>::new();
    let mut total_failed = 0u64;
    for row in rows.iter().filter(|row| row.status == CaseStatus::Failed) {
        let category = failure_category(&row.detail).to_owned();
        total_failed = total_failed
            .checked_add(1)
            .context("failure diagnostic total overflows")?;
        let category_count = category_counts.entry(category.clone()).or_default();
        *category_count = category_count
            .checked_add(1)
            .context("failure category count overflows")?;
        let reason = failure_hint(&row.detail).unwrap_or_else(|| category.clone());
        let feature_area = diagnostic_feature_area(&row.source);
        let key = (feature_area, category, reason);
        let group = groups.entry(key).or_insert_with(|| DiagnosticAccumulator {
            count: 0,
            representative: row.clone(),
        });
        group.count = group
            .count
            .checked_add(1)
            .context("failure diagnostic group count overflows")?;
        if representative_key(row) < representative_key(&group.representative) {
            row.clone_into(&mut group.representative);
        }
    }
    let mut diagnostics = groups
        .into_iter()
        .map(
            |((feature_area, category, reason), group)| DiagnosticGroup {
                count: group.count,
                feature_area,
                category,
                reason,
                representative_case: group.representative.id,
                representative_source: group.representative.source,
                detail: report_text::table_detail(&group.representative.detail),
            },
        )
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.feature_area.cmp(&right.feature_area))
            .then_with(|| left.category.cmp(&right.category))
            .then_with(|| left.reason.cmp(&right.reason))
            .then_with(|| left.representative_case.cmp(&right.representative_case))
    });
    let mut categories = category_counts
        .into_iter()
        .map(|(category, failed)| FailureCategorySummary { category, failed })
        .collect::<Vec<_>>();
    categories.sort_by(|left, right| {
        right
            .failed
            .cmp(&left.failed)
            .then_with(|| left.category.cmp(&right.category))
    });
    if total_failed == 0 {
        return Ok(None);
    }
    let represented_failed = diagnostics.iter().try_fold(0u64, |total, diagnostic| {
        total
            .checked_add(diagnostic.count)
            .context("represented failure diagnostic count overflows")
    })?;
    let total_groups = u64::try_from(diagnostics.len())
        .context("failure diagnostic group count does not fit u64")?;
    Ok(Some(FailureDiagnostics {
        total_failed,
        represented_failed,
        total_groups,
        omitted_groups: 0,
        categories,
        groups: diagnostics,
    }))
}

fn representative_key(row: &CaseRecord) -> (&str, &str) {
    (&row.id, &row.source)
}

fn diagnostic_feature_area(source: &str) -> String {
    let path = source.strip_prefix(TEST262_SOURCE_PREFIX).unwrap_or(source);
    let mut parts = path.split('/');
    if parts.next() == Some("test") {
        let area = parts.next().unwrap_or(DEFAULT_VARIANT);
        let feature = parts.next();
        return feature.map_or_else(|| area.to_owned(), |feature| format!("{area}/{feature}"));
    }
    parts
        .next()
        .map_or_else(|| DEFAULT_VARIANT.to_owned(), ToOwned::to_owned)
}

fn failure_category(detail: &str) -> &'static str {
    if detail.starts_with(DETAIL_HARNESS_PREFIX) {
        return CATEGORY_HARNESS;
    }
    if detail.starts_with(DETAIL_ASYNC_PREFIX) {
        if detail.contains(DETAIL_ASYNC_FAILURE) {
            return CATEGORY_RUNTIME;
        }
        return CATEGORY_ASYNC;
    }
    if detail.contains(DETAIL_LEXER) {
        return CATEGORY_LEXER;
    }
    if detail.contains(DETAIL_PARSER) {
        return CATEGORY_PARSER;
    }
    if detail.contains(DETAIL_RESOURCE_LIMIT) {
        return CATEGORY_RESOURCE_LIMIT;
    }
    if detail.contains(DETAIL_RUNTIME) {
        return CATEGORY_RUNTIME;
    }
    if detail.contains(DETAIL_METADATA) {
        return CATEGORY_METADATA;
    }
    if detail.contains(DETAIL_NEGATIVE) {
        return CATEGORY_NEGATIVE;
    }
    CATEGORY_OTHER
}

fn missing_binding(detail: &str) -> Option<String> {
    let (_, after_prefix) = detail.split_once(DETAIL_REFERENCE_PREFIX)?;
    let (binding, _) = after_prefix.split_once(DETAIL_REFERENCE_SUFFIX)?;
    Some(binding.to_owned())
}

fn unsupported_assert_constructor(detail: &str) -> Option<String> {
    let (_, after_prefix) = detail.split_once(DETAIL_ASSERT_CONSTRUCTOR_PREFIX)?;
    let (constructor, _) = after_prefix.split_once(DETAIL_ASSERT_CONSTRUCTOR_SUFFIX)?;
    Some(constructor.to_owned())
}

fn unsupported_negative_phase(detail: &str) -> Option<String> {
    let (_, phase) = detail.rsplit_once("unsupported negative phase '")?;
    let (phase, _) = phase.split_once('\'')?;
    Some(phase.to_owned())
}

fn failure_hint(detail: &str) -> Option<String> {
    if let Some(binding) = missing_binding(detail) {
        return Some(format!("{HINT_MISSING_BINDING_PREFIX}{binding}"));
    }
    if let Some(constructor) = unsupported_assert_constructor(detail) {
        return Some(format!("{HINT_ASSERT_CONSTRUCTOR_PREFIX}{constructor}"));
    }
    if let Some(phase) = unsupported_negative_phase(detail) {
        return Some(format!("{HINT_NEGATIVE_PHASE_PREFIX}{phase}"));
    }
    if detail.contains(HINT_UNSUPPORTED_ESCAPE) {
        return Some(HINT_LEXER_UNSUPPORTED_ESCAPE.to_owned());
    }
    if detail.contains(HINT_UNEXPECTED_CHARACTER) {
        return Some(HINT_LEXER_UNEXPECTED_CHARACTER.to_owned());
    }
    if detail.contains("expected expression") {
        return Some(HINT_EXPECTED_EXPRESSION.to_owned());
    }
    expected_token_hint(detail)
}

fn expected_token_hint(detail: &str) -> Option<String> {
    if !detail.contains(DETAIL_PARSER) {
        return None;
    }
    let (_, expected) = detail.rsplit_once("expected ")?;
    Some(format!("{HINT_EXPECTED_TOKEN_PREFIX}{expected}"))
}

fn fenced_table(table: &Table) -> String {
    format!("```text\n{table}\n```")
}

#[cfg(test)]
mod tests {
    use super::{
        CATEGORY_ASYNC, CATEGORY_HARNESS, CATEGORY_LEXER, CATEGORY_PARSER, CATEGORY_RUNTIME,
        failure_category, failure_hint, missing_binding,
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn classifies_common_failure_details() -> TestResult {
        ensure_text(
            failure_category("upstream Test262 case failed: parser error at 10"),
            CATEGORY_PARSER,
        )?;
        ensure_text(
            failure_category("upstream Test262 case failed: lexer error at 10"),
            CATEGORY_LEXER,
        )?;
        ensure_text(
            failure_category("Test262 harness include 'sta.js' failed: runtime error"),
            CATEGORY_HARNESS,
        )?;
        ensure_text(
            failure_category("upstream Test262 case failed: runtime error"),
            CATEGORY_RUNTIME,
        )?;
        ensure_text(
            failure_category(
                "upstream async Test262 case 'test/language/statements/async-function/basic.js' did not signal completion",
            ),
            CATEGORY_ASYNC,
        )?;
        ensure_text(
            failure_category(
                "upstream async Test262 case 'test/language/statements/async-function/basic.js' signaled failure: Test262:AsyncTestFailure:ReferenceError: 'x' is not defined",
            ),
            CATEGORY_RUNTIME,
        )
    }

    #[test]
    fn extracts_missing_bindings() -> TestResult {
        let detail = "runtime error: uncaught throw: ReferenceError: 'globalThis' is not defined";
        let Some(binding) = missing_binding(detail) else {
            return Err("expected missing binding".into());
        };
        ensure_text(&binding, "globalThis")
    }

    #[test]
    fn builds_actionable_failure_hints() -> TestResult {
        let Some(hint) = failure_hint(
            "runtime error: assert.throws error constructor 'TypeError' is not supported",
        ) else {
            return Err("expected unsupported constructor hint".into());
        };
        ensure_text(&hint, "unsupported error constructor: TypeError")?;

        let Some(hint) = failure_hint("parser error at 42: expected expression") else {
            return Err("expected parser hint".into());
        };
        ensure_text(&hint, "parser: expected expression")?;

        if failure_hint("negative parse case expected SyntaxError, got runtime error").is_none() {
            return Ok(());
        }
        Err("expected non-parser expected text to be ignored".into())
    }

    fn ensure_text(actual: &str, expected: &str) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected '{expected}', got '{actual}'").into())
    }
}
