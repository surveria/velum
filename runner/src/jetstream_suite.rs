use std::{env, time::Duration};

use anyhow::Context as _;

const ENV_SUITE_MAX_SECONDS: &str = "VELUM_JETSTREAM_SUITE_MAX_SECONDS";
const DEFAULT_READ_SUITE_MAX_SECONDS: u64 = 120;
const DEFAULT_REFRESH_SUITE_MAX_SECONDS: u64 = 900;

pub fn budget(refresh: bool) -> anyhow::Result<Duration> {
    let value = match env::var(ENV_SUITE_MAX_SECONDS) {
        Ok(value) => Some(value),
        Err(env::VarError::NotPresent) => None,
        Err(error) => return Err(error).context(format!("failed to read {ENV_SUITE_MAX_SECONDS}")),
    };
    budget_from_value(refresh, value.as_deref())
}

fn budget_from_value(refresh: bool, value: Option<&str>) -> anyhow::Result<Duration> {
    let default = if refresh {
        DEFAULT_REFRESH_SUITE_MAX_SECONDS
    } else {
        DEFAULT_READ_SUITE_MAX_SECONDS
    };
    let seconds = value.map_or(Ok(default), |value| {
        value.trim().parse::<u64>().with_context(|| {
            format!("{ENV_SUITE_MAX_SECONDS} must be a non-negative integer number of seconds")
        })
    })?;
    Ok(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::budget_from_value;
    use std::time::Duration;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn read_and_refresh_have_separate_bounded_defaults() -> TestResult {
        if budget_from_value(false, None)? != Duration::from_mins(2) {
            return Err("read suite budget changed unexpectedly".into());
        }
        if budget_from_value(true, None)? != Duration::from_mins(15) {
            return Err("refresh suite budget changed unexpectedly".into());
        }
        Ok(())
    }

    #[test]
    fn explicit_suite_budget_is_validated() -> TestResult {
        if budget_from_value(false, Some("7"))? != Duration::from_secs(7) {
            return Err("suite budget override was ignored".into());
        }
        if budget_from_value(false, Some("invalid")).is_err() {
            return Ok(());
        }
        Err("invalid suite budget unexpectedly succeeded".into())
    }
}
