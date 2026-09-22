use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

const MAX_PROC_BYTES: u64 = 128 * 1024;
const KIB_BYTES: u64 = 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessMemory {
    pub pid: u32,
    pub rss_bytes: Metric,
    pub pss_bytes: Metric,
    pub peak_rss_bytes: Metric,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Metric {
    Available { bytes: u64 },
    Unavailable { reason: String },
}

impl Metric {
    pub const fn bytes(&self) -> Option<u64> {
        match self {
            Self::Available { bytes } => Some(*bytes),
            Self::Unavailable { .. } => None,
        }
    }
}

enum ProcText {
    Available(String),
    Unavailable(String),
}

/// Samples resident memory without confusing missing proc data with zero bytes.
///
/// `peak_rss_bytes` is the process-lifetime high-water RSS, not a phase peak.
/// The proc files are read independently; the sample is not an atomic snapshot.
pub fn sample(pid: u32) -> Result<ProcessMemory> {
    if !cfg!(target_os = "linux") {
        let unavailable = Metric::Unavailable {
            reason: "Linux /proc memory sampling is unavailable on this platform".into(),
        };
        return Ok(ProcessMemory {
            pid,
            rss_bytes: unavailable.clone(),
            pss_bytes: unavailable.clone(),
            peak_rss_bytes: unavailable,
        });
    }

    let status_path = format!("/proc/{pid}/status");
    let smaps_path = format!("/proc/{pid}/smaps_rollup");
    let status = read_proc(Path::new(&status_path))?;
    let smaps = read_proc(Path::new(&smaps_path))?;
    let status_rss = field(&status, &status_path, "VmRSS")?;
    let smaps_rss = field(&smaps, &smaps_path, "Rss")?;

    Ok(ProcessMemory {
        pid,
        rss_bytes: prefer(smaps_rss, status_rss),
        pss_bytes: field(&smaps, &smaps_path, "Pss")?,
        peak_rss_bytes: field(&status, &status_path, "VmHWM")?,
    })
}

/// Parses one optional proc memory field, whose `kB` unit means 1024 bytes.
///
/// Duplicate keys and malformed matching fields are errors, including invalid
/// units, extra tokens, negative quantities and byte-conversion overflow.
pub fn parse_kib_field(source: &str, key: &str) -> Result<Option<u64>> {
    ensure!(
        u64::try_from(source.len())? <= MAX_PROC_BYTES,
        "proc text exceeds the {MAX_PROC_BYTES}-byte limit"
    );
    ensure!(
        !key.is_empty()
            && !key.contains(|character: char| character.is_whitespace() || character == ':'),
        "proc field key must be a nonempty name without whitespace or a colon"
    );

    let mut bytes = None;
    for line in source.lines() {
        let Some((name, value)) = line.split_once(':') else {
            ensure!(
                line.split_whitespace().next() != Some(key),
                "proc field {key} is missing its colon"
            );
            continue;
        };
        if name.trim() != key {
            continue;
        }
        ensure!(bytes.is_none(), "duplicate proc field {key}");
        let mut tokens = value.split_whitespace();
        let quantity = tokens
            .next()
            .with_context(|| format!("proc field {key} has no quantity"))?;
        ensure!(
            quantity.bytes().all(|byte| byte.is_ascii_digit()),
            "proc field {key} has an invalid unsigned quantity: {quantity}"
        );
        ensure!(
            tokens.next() == Some("kB") && tokens.next().is_none(),
            "proc field {key} must contain exactly an unsigned quantity and kB"
        );
        let kib = quantity.parse::<u64>().with_context(|| {
            format!("proc field {key} quantity cannot be represented as u64: {quantity}")
        })?;
        bytes = Some(kib.checked_mul(KIB_BYTES).with_context(|| {
            format!("proc field {key} quantity {quantity} kB overflows byte accounting")
        })?);
    }
    Ok(bytes)
}

fn read_proc(path: &Path) -> Result<ProcText> {
    let mut contents = String::new();
    let read = File::open(path)
        .and_then(|file| file.take(MAX_PROC_BYTES + 1).read_to_string(&mut contents));
    if let Err(error) = read {
        return match error.kind() {
            ErrorKind::NotFound => Ok(ProcText::Unavailable(format!(
                "{} is unavailable: file not found",
                path.display()
            ))),
            ErrorKind::PermissionDenied => Ok(ProcText::Unavailable(format!(
                "{} is unavailable: permission denied",
                path.display()
            ))),
            _ => Err(error)
                .with_context(|| format!("reading bounded proc data from {}", path.display())),
        };
    }
    if u64::try_from(contents.len())? > MAX_PROC_BYTES {
        bail!(
            "{} exceeds the {MAX_PROC_BYTES}-byte proc read limit",
            path.display()
        );
    }
    Ok(ProcText::Available(contents))
}

fn field(source: &ProcText, path: &str, key: &str) -> Result<Metric> {
    match source {
        ProcText::Available(source) => parse_kib_field(source, key)
            .with_context(|| format!("parsing {path}"))?
            .map_or_else(
                || {
                    Ok(Metric::Unavailable {
                        reason: format!("{path} does not contain field {key}"),
                    })
                },
                |bytes| Ok(Metric::Available { bytes }),
            ),
        ProcText::Unavailable(reason) => Ok(Metric::Unavailable {
            reason: reason.clone(),
        }),
    }
}

fn prefer(primary: Metric, fallback: Metric) -> Metric {
    match (primary, fallback) {
        (available @ Metric::Available { .. }, _)
        | (Metric::Unavailable { .. }, available @ Metric::Available { .. }) => available,
        (Metric::Unavailable { reason }, Metric::Unavailable { reason: fallback }) => {
            Metric::Unavailable {
                reason: format!("{reason}; RSS fallback: {fallback}"),
            }
        }
    }
}
