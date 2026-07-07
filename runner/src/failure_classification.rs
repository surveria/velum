use std::collections::BTreeMap;

use tabled::{Table, Tabled};

use super::{CaseRow, percent};

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
const SUBTITLE_DIRECTORIES: &str = "#### Top Failing Directories";
const SUBTITLE_HINTS: &str = "#### Top Failure Hints";
const SUBTITLE_MISSING_BINDINGS: &str = "#### Top Missing Bindings";
const SUBTITLE_VARIANTS: &str = "#### Variant Suffixes";
const TEST262_SOURCE_PREFIX: &str = "test262:";
const TOP_ROW_LIMIT: usize = 15;
const DIRECTORY_DEPTH: usize = 3;

#[derive(Debug, Clone)]
struct Counted {
    label: String,
    count: usize,
}

#[derive(Debug, Tabled)]
struct CategoryRow {
    category: String,
    failed: usize,
    share: String,
}

#[derive(Debug, Tabled)]
struct DirectoryRow {
    directory: String,
    failed: usize,
    share: String,
}

#[derive(Debug, Tabled)]
struct VariantRow {
    variant: String,
    failed: usize,
    share: String,
}

#[derive(Debug, Tabled)]
struct BindingRow {
    binding: String,
    failed: usize,
    share: String,
}

#[derive(Debug, Tabled)]
struct HintRow {
    hint: String,
    failed: usize,
    share: String,
}

#[must_use]
pub fn sections(failed_rows: &[CaseRow]) -> Vec<String> {
    if failed_rows.is_empty() {
        return Vec::new();
    }

    let mut sections = vec![SECTION_TITLE.to_owned(), String::new()];
    push_table_section(
        &mut sections,
        SUBTITLE_CATEGORIES,
        category_rows(failed_rows),
    );
    push_table_section(
        &mut sections,
        SUBTITLE_DIRECTORIES,
        directory_rows(failed_rows),
    );
    push_table_section(&mut sections, SUBTITLE_VARIANTS, variant_rows(failed_rows));

    let binding_rows = missing_binding_rows(failed_rows);
    if !binding_rows.is_empty() {
        push_table_section(&mut sections, SUBTITLE_MISSING_BINDINGS, binding_rows);
    }

    let hint_rows = failure_hint_rows(failed_rows);
    if !hint_rows.is_empty() {
        push_table_section(&mut sections, SUBTITLE_HINTS, hint_rows);
    }

    sections
}

fn push_table_section<T: Tabled>(sections: &mut Vec<String>, title: &str, rows: Vec<T>) {
    sections.push(title.to_owned());
    sections.push(String::new());
    sections.push(fenced_table(&Table::new(rows)));
    sections.push(String::new());
}

fn category_rows(failed_rows: &[CaseRow]) -> Vec<CategoryRow> {
    let total = failed_rows.len();
    count_by(failed_rows, |row| {
        Some(failure_category(&row.detail).to_owned())
    })
    .into_iter()
    .map(|item| CategoryRow {
        category: item.label,
        failed: item.count,
        share: percent(item.count, total),
    })
    .collect()
}

fn directory_rows(failed_rows: &[CaseRow]) -> Vec<DirectoryRow> {
    let total = failed_rows.len();
    count_by(failed_rows, |row| Some(test262_directory(&row.source)))
        .into_iter()
        .take(TOP_ROW_LIMIT)
        .map(|item| DirectoryRow {
            directory: item.label,
            failed: item.count,
            share: percent(item.count, total),
        })
        .collect()
}

fn variant_rows(failed_rows: &[CaseRow]) -> Vec<VariantRow> {
    let total = failed_rows.len();
    count_by(failed_rows, |row| Some(case_variant(&row.case).to_owned()))
        .into_iter()
        .map(|item| VariantRow {
            variant: item.label,
            failed: item.count,
            share: percent(item.count, total),
        })
        .collect()
}

fn missing_binding_rows(failed_rows: &[CaseRow]) -> Vec<BindingRow> {
    let total = failed_rows.len();
    count_by(failed_rows, |row| missing_binding(&row.detail))
        .into_iter()
        .take(TOP_ROW_LIMIT)
        .map(|item| BindingRow {
            binding: item.label,
            failed: item.count,
            share: percent(item.count, total),
        })
        .collect()
}

fn failure_hint_rows(failed_rows: &[CaseRow]) -> Vec<HintRow> {
    let total = failed_rows.len();
    count_by(failed_rows, |row| failure_hint(&row.detail))
        .into_iter()
        .take(TOP_ROW_LIMIT)
        .map(|item| HintRow {
            hint: item.label,
            failed: item.count,
            share: percent(item.count, total),
        })
        .collect()
}

fn count_by(
    rows: &[CaseRow],
    mut label_for: impl FnMut(&CaseRow) -> Option<String>,
) -> Vec<Counted> {
    let mut counts = BTreeMap::<String, usize>::new();
    for row in rows {
        if let Some(label) = label_for(row) {
            let count = counts.entry(label).or_default();
            *count = count.saturating_add(1);
        }
    }
    let mut counted = counts
        .into_iter()
        .map(|(label, count)| Counted { label, count })
        .collect::<Vec<_>>();
    counted.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    counted
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

fn test262_directory(source: &str) -> String {
    let path = source.strip_prefix(TEST262_SOURCE_PREFIX).unwrap_or(source);
    let mut parts = path.split('/');
    let mut selected = Vec::new();
    for _ in 0..DIRECTORY_DEPTH {
        let Some(part) = parts.next() else {
            break;
        };
        selected.push(part);
    }
    if selected.is_empty() {
        return DEFAULT_VARIANT.to_owned();
    }
    selected.join("/")
}

fn case_variant(case: &str) -> &str {
    let Some((_, variant)) = case.rsplit_once('#') else {
        return DEFAULT_VARIANT;
    };
    variant
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
        case_variant, failure_category, failure_hint, missing_binding, test262_directory,
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
    fn extracts_test262_directory_groups() -> TestResult {
        ensure_text(
            &test262_directory("test262:test/built-ins/Array/prototype/map/name.js"),
            "test/built-ins/Array",
        )?;
        ensure_text(
            &test262_directory("test262:test/language/statements/if/S12.js"),
            "test/language/statements",
        )
    }

    #[test]
    fn extracts_variant_suffixes() -> TestResult {
        ensure_text(case_variant("test/language/example.js#strict"), "strict")?;
        ensure_text(case_variant("test/language/example.js"), "unknown")
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
