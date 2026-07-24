use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, ensure};

/// Parses a human-readable positive duration.
///
/// # Errors
///
/// Returns an error when the value is not a valid positive duration.
pub fn parse_duration(value: &str) -> anyhow::Result<Duration> {
    let duration = humantime::parse_duration(value)
        .with_context(|| format!("invalid duration '{value}'; examples: 30s, 2m, 1h"))?;
    ensure!(!duration.is_zero(), "duration must be greater than zero");
    Ok(duration)
}

/// Returns the current Unix timestamp in milliseconds.
///
/// # Errors
///
/// Returns an error when the system clock is before the Unix epoch.
pub fn unix_timestamp_millis() -> anyhow::Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_millis())
}

#[must_use]
pub fn duration_nanos_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

#[must_use]
pub fn duration_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
