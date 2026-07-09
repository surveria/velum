use std::collections::BTreeSet;

use anyhow::{Context as _, bail};

use super::{
    CaseCounts, CaseDetailCoverage, DetailCompleteness, MAX_FAILURE_DIAGNOSTICS, SuiteReport,
    SuiteStatus, TEST262_FILE_SUITE, TEST262_FULL_SUITE,
};

pub(super) const fn suite_status(counts: CaseCounts, unavailable: bool) -> SuiteStatus {
    if counts.failed > 0 {
        return SuiteStatus::Failed;
    }
    if counts.executed == 0 && (counts.skipped > 0 || unavailable) {
        return SuiteStatus::Skipped;
    }
    SuiteStatus::Passed
}

pub(super) fn case_counts(
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
) -> anyhow::Result<CaseCounts> {
    let executed = passed
        .checked_add(failed)
        .context("suite executed count overflows")?;
    Ok(CaseCounts {
        total: count_to_u64(total)?,
        executed: count_to_u64(executed)?,
        passed: count_to_u64(passed)?,
        failed: count_to_u64(failed)?,
        skipped: count_to_u64(skipped)?,
    })
}

pub(super) fn case_detail_coverage(
    total: u64,
    recorded_rows: u64,
) -> anyhow::Result<CaseDetailCoverage> {
    let Some(omitted_rows) = total.checked_sub(recorded_rows) else {
        bail!("suite contains {recorded_rows} detail rows but reports only {total} total cases");
    };
    Ok(CaseDetailCoverage {
        completeness: if omitted_rows == 0 {
            DetailCompleteness::Complete
        } else {
            DetailCompleteness::Partial
        },
        recorded_rows,
        omitted_rows,
    })
}

pub(super) fn limit_diagnostics(suites: &mut [SuiteReport]) -> anyhow::Result<()> {
    omit_duplicate_file_diagnostics(suites);
    let mut ranked = ranked_diagnostics(suites);
    ranked.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| right.count.cmp(&left.count))
            .then_with(|| left.feature_area.cmp(&right.feature_area))
            .then_with(|| left.category.cmp(&right.category))
            .then_with(|| left.reason.cmp(&right.reason))
            .then_with(|| left.suite_index.cmp(&right.suite_index))
            .then_with(|| left.diagnostic_index.cmp(&right.diagnostic_index))
    });
    let retained = retained_diagnostics(&ranked);
    apply_retained_diagnostics(suites, &retained)
}

#[derive(Debug)]
struct RankedDiagnostic {
    suite_index: usize,
    diagnostic_index: usize,
    required: bool,
    count: u64,
    feature_area: String,
    category: String,
    reason: String,
}

fn omit_duplicate_file_diagnostics(suites: &mut [SuiteReport]) {
    let has_full_corpus = suites
        .iter()
        .any(|suite| suite.summary.name == TEST262_FULL_SUITE);
    if !has_full_corpus {
        return;
    }
    for suite in suites {
        if suite.summary.name == TEST262_FILE_SUITE
            && suite.summary.failure_diagnostics.take().is_some()
        {
            suite.summary.diagnostics_derived_from = Some(TEST262_FULL_SUITE.to_owned());
        }
    }
}

fn ranked_diagnostics(suites: &[SuiteReport]) -> Vec<RankedDiagnostic> {
    suites
        .iter()
        .enumerate()
        .flat_map(|(suite_index, suite)| {
            suite
                .summary
                .failure_diagnostics
                .iter()
                .flat_map(|diagnostics| diagnostics.groups.iter())
                .enumerate()
                .map(move |(diagnostic_index, diagnostic)| RankedDiagnostic {
                    suite_index,
                    diagnostic_index,
                    required: suite.summary.required,
                    count: diagnostic.count,
                    feature_area: diagnostic.feature_area.clone(),
                    category: diagnostic.category.clone(),
                    reason: diagnostic.reason.clone(),
                })
        })
        .collect()
}

fn retained_diagnostics(ranked: &[RankedDiagnostic]) -> BTreeSet<(usize, usize)> {
    let mut retained = BTreeSet::new();
    for item in ranked.iter().filter(|item| item.required) {
        retain_if_room(&mut retained, item);
    }
    let mut seeded_categories = BTreeSet::new();
    for item in ranked {
        if retained.len() == MAX_FAILURE_DIAGNOSTICS {
            break;
        }
        if seeded_categories.insert((item.suite_index, item.category.as_str())) {
            retain_if_room(&mut retained, item);
        }
    }
    for item in ranked {
        retain_if_room(&mut retained, item);
    }
    retained
}

fn retain_if_room(retained: &mut BTreeSet<(usize, usize)>, item: &RankedDiagnostic) {
    if retained.len() < MAX_FAILURE_DIAGNOSTICS {
        retained.insert((item.suite_index, item.diagnostic_index));
    }
}

fn apply_retained_diagnostics(
    suites: &mut [SuiteReport],
    retained: &BTreeSet<(usize, usize)>,
) -> anyhow::Result<()> {
    for (suite_index, suite) in suites.iter_mut().enumerate() {
        let Some(diagnostics) = suite.summary.failure_diagnostics.as_mut() else {
            continue;
        };
        diagnostics.groups = diagnostics
            .groups
            .drain(..)
            .enumerate()
            .filter_map(|(diagnostic_index, diagnostic)| {
                retained
                    .contains(&(suite_index, diagnostic_index))
                    .then_some(diagnostic)
            })
            .collect();
        diagnostics.represented_failed = diagnostics
            .groups
            .iter()
            .try_fold(0u64, |total, group| total.checked_add(group.count))
            .context("represented failure diagnostic count overflows")?;
        let retained_count = count_to_u64(diagnostics.groups.len())?;
        diagnostics.omitted_groups = diagnostics
            .total_groups
            .checked_sub(retained_count)
            .context("retained failure diagnostics exceed total groups")?;
    }
    Ok(())
}

fn count_to_u64(value: usize) -> anyhow::Result<u64> {
    u64::try_from(value).context("report count does not fit u64")
}
