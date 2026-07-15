//! Deterministic exact and prefix selection for `JetStream` shell cases.

use std::{collections::BTreeSet, env};

use anyhow::bail;

use super::JetStreamCase;

pub const ENV_JETSTREAM_FILTER: &str = "VELUM_JETSTREAM_FILTER";

#[derive(Debug, Clone, Eq, PartialEq)]
enum Selector {
    Exact(String),
    Prefix(String),
}

impl Selector {
    fn parse(raw: &str) -> anyhow::Result<Self> {
        let value = raw.trim();
        if value.is_empty() {
            bail!("JetStream filter contains an empty selector")
        }
        if let Some(prefix) = value.strip_suffix('*') {
            if prefix.contains('*') {
                bail!("JetStream selector '{value}' may contain only one trailing '*'")
            }
            return Ok(Self::Prefix(prefix.to_owned()));
        }
        if value.contains('*') {
            bail!("JetStream selector '{value}' must use '*' only as a suffix")
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
pub struct JetStreamSelection {
    selectors: Vec<Selector>,
}

impl JetStreamSelection {
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_value(env::var(ENV_JETSTREAM_FILTER).ok().as_deref())
    }

    fn from_value(filter: Option<&str>) -> anyhow::Result<Self> {
        let selectors = filter
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.split(',').map(Selector::parse).collect())
            .transpose()?
            .unwrap_or_default();
        Ok(Self { selectors })
    }

    pub fn select<'a>(&self, cases: &'a [JetStreamCase]) -> anyhow::Result<Vec<&'a JetStreamCase>> {
        ensure_unique_ids(cases)?;
        if self.selectors.is_empty() {
            return Ok(cases.iter().collect());
        }
        let mut selected_ids = BTreeSet::new();
        for selector in &self.selectors {
            let matched: Vec<_> = cases
                .iter()
                .filter(|case| selector.matches(case.id))
                .map(|case| case.id)
                .collect();
            if matched.is_empty() {
                bail!(
                    "JetStream selector '{}' matched no cases",
                    selector.display()
                )
            }
            selected_ids.extend(matched);
        }
        Ok(cases
            .iter()
            .filter(|case| selected_ids.contains(case.id))
            .collect())
    }
}

fn ensure_unique_ids(cases: &[JetStreamCase]) -> anyhow::Result<()> {
    let mut ids = BTreeSet::new();
    for case in cases {
        if !ids.insert(case.id) {
            bail!("duplicate JetStream case id '{}'", case.id)
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::JetStreamSelection;
    use crate::jetstream::JetStreamCase;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    const CASES: [JetStreamCase; 3] = [
        JetStreamCase::timed("alpha", &["alpha.js"]),
        JetStreamCase::timed("sunspider-one", &["one.js"]),
        JetStreamCase::skipped("sunspider-two", "fixture"),
    ];

    #[test]
    fn absent_filter_keeps_official_coverage_rows() -> TestResult {
        let selection = JetStreamSelection::from_value(None)?;
        ensure_ids(
            &selection.select(&CASES)?,
            &["alpha", "sunspider-one", "sunspider-two"],
        )
    }

    #[test]
    fn exact_filter_selects_only_exact_id() -> TestResult {
        let selection = JetStreamSelection::from_value(Some("alpha"))?;
        ensure_ids(&selection.select(&CASES)?, &["alpha"])
    }

    #[test]
    fn explicit_prefix_selects_timed_and_skipped_rows() -> TestResult {
        let selection = JetStreamSelection::from_value(Some("sunspider-*"))?;
        ensure_ids(
            &selection.select(&CASES)?,
            &["sunspider-one", "sunspider-two"],
        )
    }

    #[test]
    fn unmatched_selector_is_an_error() -> TestResult {
        let selection = JetStreamSelection::from_value(Some("missing"))?;
        if selection.select(&CASES).is_err() {
            return Ok(());
        }
        Err("unmatched selector unexpectedly succeeded".into())
    }

    fn ensure_ids(cases: &[&JetStreamCase], expected: &[&str]) -> TestResult {
        let actual: Vec<_> = cases.iter().map(|case| case.id).collect();
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected {expected:?}, got {actual:?}").into())
    }
}
