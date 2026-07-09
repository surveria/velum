//! Shared benchmark protocol types.
//!
//! Canonical benchmark timing is owned by the Rust runner. JavaScript sources
//! expose deterministic setup, run, and verify functions, but never decide the
//! measured interval themselves.

use std::{fmt, time::Duration};

use anyhow::{Context as _, bail};
use rs_quickjs::Value;

pub const PREPARED_PROTOCOL_VERSION: &str = "prepared-v1";
pub const SETUP_FUNCTION: &str = "__rsqjsBenchSetup";
pub const RUN_FUNCTION: &str = "__rsqjsBenchRun";
pub const VERIFY_FUNCTION: &str = "__rsqjsBenchVerify";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BenchmarkMode {
    ColdEval,
    PreparedExecution,
}

impl BenchmarkMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ColdEval => "cold_eval",
            Self::PreparedExecution => "prepared_execution",
        }
    }

    pub const fn uses_prepared_protocol(self) -> bool {
        matches!(self, Self::PreparedExecution)
    }
}

impl fmt::Display for BenchmarkMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BenchmarkInput {
    Standard,
    HostImage { byte_len: usize },
}

impl BenchmarkInput {
    pub fn descriptor(self) -> String {
        match self {
            Self::Standard => "standard".to_owned(),
            Self::HostImage { byte_len } => format!("host_image:{byte_len}"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BenchmarkChecksum {
    Undefined,
    Null,
    Boolean(bool),
    Number(u64),
    String(String),
}

impl BenchmarkChecksum {
    pub fn from_rsqjs(value: Value) -> anyhow::Result<Self> {
        match value {
            Value::Undefined => Ok(Self::Undefined),
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::boolean(value)),
            Value::Number(value) => Ok(Self::number(value)),
            Value::String(value) => Ok(Self::string(value)),
            Value::HeapString(value) => Ok(Self::string(value.to_string())),
            unsupported => bail!(
                "benchmark checksum must be a primitive value, got {}",
                unsupported.type_name()
            ),
        }
    }

    pub const fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    pub fn number(value: f64) -> Self {
        let normalized = if value.is_nan() { f64::NAN } else { value };
        Self::Number(normalized.to_bits())
    }

    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    pub fn storage_text(&self) -> String {
        match self {
            Self::Undefined => "undefined".to_owned(),
            Self::Null => "null".to_owned(),
            Self::Boolean(false) => "bool:0".to_owned(),
            Self::Boolean(true) => "bool:1".to_owned(),
            Self::Number(bits) => format!("number:{bits:016x}"),
            Self::String(value) => format!("string:{}", encode_hex(value.as_bytes())),
        }
    }

    pub fn from_storage_text(value: &str) -> anyhow::Result<Self> {
        match value {
            "undefined" => return Ok(Self::Undefined),
            "null" => return Ok(Self::Null),
            "bool:0" => return Ok(Self::Boolean(false)),
            "bool:1" => return Ok(Self::Boolean(true)),
            _ => {}
        }
        if let Some(raw) = value.strip_prefix("number:") {
            let bits = u64::from_str_radix(raw, 16)
                .with_context(|| format!("invalid benchmark number checksum '{value}'"))?;
            return Ok(Self::Number(bits));
        }
        if let Some(raw) = value.strip_prefix("string:") {
            let bytes = decode_hex(raw)?;
            let text = String::from_utf8(bytes)
                .with_context(|| format!("benchmark string checksum is not UTF-8: '{value}'"))?;
            return Ok(Self::String(text));
        }
        bail!("unknown benchmark checksum encoding '{value}'")
    }
}

impl fmt::Display for BenchmarkChecksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Undefined => formatter.write_str("undefined"),
            Self::Null => formatter.write_str("null"),
            Self::Boolean(value) => write!(formatter, "{value}"),
            Self::Number(bits) => write!(formatter, "{}", f64::from_bits(*bits)),
            Self::String(value) => write!(formatter, "{:?}", value),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct BenchmarkLifecycle {
    pub load: Duration,
    pub compile: Option<Duration>,
    pub setup: Option<Duration>,
    pub warmup: Duration,
    pub timed_run: Duration,
    pub verify: Option<Duration>,
    pub teardown: Option<Duration>,
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(hex_digit(byte >> 4));
        encoded.push(hex_digit(byte & 0x0f));
    }
    encoded
}

fn decode_hex(value: &str) -> anyhow::Result<Vec<u8>> {
    let chunks = value.as_bytes().chunks_exact(2);
    if !chunks.remainder().is_empty() {
        bail!("benchmark checksum hex string has odd length")
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in chunks {
        let Some(high) = pair.first().and_then(|digit| hex_value(*digit)) else {
            bail!("benchmark checksum contains a non-hex digit")
        };
        let Some(low) = pair.get(1).and_then(|digit| hex_value(*digit)) else {
            bail!("benchmark checksum contains a non-hex digit")
        };
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_digit(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0'.saturating_add(value)),
        _ => char::from(b'a'.saturating_add(value.saturating_sub(10))),
    }
}

const fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::BenchmarkChecksum;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn checksum_storage_round_trip_preserves_types() -> TestResult {
        let values = [
            BenchmarkChecksum::Undefined,
            BenchmarkChecksum::Null,
            BenchmarkChecksum::boolean(false),
            BenchmarkChecksum::boolean(true),
            BenchmarkChecksum::number(-0.0),
            BenchmarkChecksum::number(f64::NAN),
            BenchmarkChecksum::string("tab\tand unicode: да"),
        ];
        for value in values {
            let decoded = BenchmarkChecksum::from_storage_text(&value.storage_text())?;
            if decoded != value {
                return Err(format!("checksum round trip changed {value}").into());
            }
        }
        Ok(())
    }
}
