//! Deterministic benchmark-set and case selection.

use std::{collections::BTreeSet, env};

use anyhow::bail;

use crate::cases::BenchmarkCase;

pub const ENV_BENCH_SET: &str = "RSQJS_BENCH_SET";
pub const ENV_BENCH_FILTER: &str = "RSQJS_BENCH_FILTER";

const SET_FULL: &str = "full";
const SET_SENTINEL: &str = "sentinel";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum BenchmarkSet {
    Full,
    Sentinel,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum Selector {
    Exact(String),
    Prefix(String),
}

impl Selector {
    fn parse(raw: &str) -> anyhow::Result<Self> {
        let value = raw.trim();
        if value.is_empty() {
            bail!("benchmark filter contains an empty selector")
        }
        if let Some(prefix) = value.strip_suffix('*') {
            if prefix.contains('*') {
                bail!("benchmark selector '{value}' may contain only one trailing '*'")
            }
            return Ok(Self::Prefix(prefix.to_owned()));
        }
        if value.contains('*') {
            bail!("benchmark selector '{value}' must use '*' only as a suffix")
        }
        Ok(Self::Exact(value.to_owned()))
    }

    fn matches(&self, id: &str) -> bool {
        match self {
            Self::Exact(expected) => id == expected,
            Self::Prefix(prefix) => id.starts_with(prefix),
        }
    }

    fn display(&self) -> String {
        match self {
            Self::Exact(value) => value.clone(),
            Self::Prefix(value) => format!("{value}*"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BenchmarkSelection {
    set: BenchmarkSet,
    selectors: Vec<Selector>,
}

impl BenchmarkSelection {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_values(
            env::var(ENV_BENCH_SET).ok().as_deref(),
            env::var(ENV_BENCH_FILTER).ok().as_deref(),
        )
    }

    fn from_values(set: Option<&str>, filter: Option<&str>) -> anyhow::Result<Self> {
        let set = match set.map(str::trim).filter(|value| !value.is_empty()) {
            None | Some(SET_FULL) => BenchmarkSet::Full,
            Some(SET_SENTINEL) => BenchmarkSet::Sentinel,
            Some(value) => {
                bail!("unknown benchmark set '{value}'; expected '{SET_FULL}' or '{SET_SENTINEL}'")
            }
        };
        let selectors = filter
            .map(|value| value.split(',').map(Selector::parse).collect())
            .transpose()?
            .unwrap_or_default();
        Ok(Self { set, selectors })
    }

    pub fn select(&self, cases: Vec<BenchmarkCase>) -> anyhow::Result<Vec<BenchmarkCase>> {
        ensure_unique_ids(&cases)?;
        let candidates: Vec<_> = cases
            .into_iter()
            .filter(|case| self.set == BenchmarkSet::Full || case.sentinel)
            .collect();
        if candidates.is_empty() {
            bail!("benchmark set contains no cases")
        }
        if self.selectors.is_empty() {
            return Ok(candidates);
        }

        let mut selected_ids = BTreeSet::new();
        for selector in &self.selectors {
            let matched: Vec<_> = candidates
                .iter()
                .filter(|case| selector.matches(case.id))
                .map(|case| case.id)
                .collect();
            if matched.is_empty() {
                bail!(
                    "benchmark selector '{}' matched no cases in the selected set",
                    selector.display()
                )
            }
            selected_ids.extend(matched);
        }
        Ok(candidates
            .into_iter()
            .filter(|case| selected_ids.contains(case.id))
            .collect())
    }
}

fn ensure_unique_ids(cases: &[BenchmarkCase]) -> anyhow::Result<()> {
    let mut ids = BTreeSet::new();
    for case in cases {
        if !ids.insert(case.id) {
            bail!("duplicate benchmark case id '{}'", case.id)
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::BenchmarkSelection;
    use crate::cases::BenchmarkCase;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    const FULL_ONLY: BenchmarkCase = BenchmarkCase::cold("legacy_case", "legacy.js");
    const SENTINEL_A: BenchmarkCase =
        BenchmarkCase::prepared_sentinel("sentinel_array", "array.js");
    const SENTINEL_B: BenchmarkCase =
        BenchmarkCase::prepared_sentinel("sentinel_object", "object.js");

    #[test]
    fn defaults_to_full_set_for_backward_compatibility() -> TestResult {
        let selection = BenchmarkSelection::from_values(None, None)?;
        let selected = selection.select(vec![FULL_ONLY, SENTINEL_A, SENTINEL_B])?;
        ensure_ids(
            &selected,
            &["legacy_case", "sentinel_array", "sentinel_object"],
        )
    }

    #[test]
    fn sentinel_set_excludes_legacy_cases() -> TestResult {
        let selection = BenchmarkSelection::from_values(Some("sentinel"), None)?;
        let selected = selection.select(vec![FULL_ONLY, SENTINEL_A, SENTINEL_B])?;
        ensure_ids(&selected, &["sentinel_array", "sentinel_object"])
    }

    #[test]
    fn exact_filter_does_not_match_substrings() -> TestResult {
        let selection = BenchmarkSelection::from_values(Some("full"), Some("array"))?;
        let result = selection.select(vec![FULL_ONLY, SENTINEL_A]);
        if result.is_err() {
            return Ok(());
        }
        Err("bare substring unexpectedly matched a benchmark id".into())
    }

    #[test]
    fn explicit_prefix_filter_selects_matching_cases() -> TestResult {
        let selection = BenchmarkSelection::from_values(Some("sentinel"), Some("sentinel_a*"))?;
        let selected = selection.select(vec![FULL_ONLY, SENTINEL_A, SENTINEL_B])?;
        ensure_ids(&selected, &["sentinel_array"])
    }

    fn ensure_ids(cases: &[BenchmarkCase], expected: &[&str]) -> TestResult {
        let actual: Vec<_> = cases.iter().map(|case| case.id).collect();
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected {expected:?}, got {actual:?}").into())
    }
}
