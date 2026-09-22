#[path = "../src/memory_benchmarks/proc_metrics.rs"]
mod proc_metrics;

use anyhow::{Result, ensure};
use proc_metrics::{Metric, ProcessMemory, parse_kib_field};

#[test]
fn converts_kib_fields_exactly() -> Result<()> {
    let source = "Name:\tworker\nVmRSS:\t123 kB\nVmHWM: 456 kB\n";
    ensure!(parse_kib_field(source, "VmRSS")? == Some(125_952));
    ensure!(parse_kib_field(source, "VmHWM")? == Some(466_944));
    ensure!(parse_kib_field("Pss: 0 kB\n", "Pss")? == Some(0));
    ensure!(
        parse_kib_field("Rss: 18014398509481983 kB\n", "Rss")? == Some(18_446_744_073_709_550_592)
    );
    Ok(())
}

#[test]
fn missing_fields_are_not_zero() -> Result<()> {
    ensure!(parse_kib_field("Rss: 0 kB\nSwapPss: 4 kB\n", "Pss")?.is_none());
    ensure!(parse_kib_field("", "VmHWM")?.is_none());
    Ok(())
}

#[test]
fn malformed_expected_fields_are_errors() -> Result<()> {
    for source in [
        "VmRSS:",
        "VmRSS: 1",
        "VmRSS: 1 KB",
        "VmRSS: 1 KiB",
        "VmRSS: 1 bytes",
        "VmRSS: -1 kB",
        "VmRSS: +1 kB",
        "VmRSS: 1.5 kB",
        "VmRSS: one kB",
        "VmRSS: 1 kB extra",
        "VmRSS 1 kB",
        "VmRSS",
    ] {
        ensure!(
            parse_kib_field(source, "VmRSS").is_err(),
            "malformed expected field was accepted: {source:?}"
        );
    }
    Ok(())
}

#[test]
fn duplicates_and_overflows_are_errors() -> Result<()> {
    for source in [
        "VmRSS: 0 kB\nVmRSS: 1 kB\n",
        "VmRSS: 18014398509481984 kB\n",
        "VmRSS: 18446744073709551616 kB\n",
    ] {
        ensure!(parse_kib_field(source, "VmRSS").is_err());
    }
    Ok(())
}

#[test]
fn bounds_input_and_validates_requested_key() -> Result<()> {
    let oversized = " ".repeat(128 * 1024 + 1);
    ensure!(parse_kib_field(&oversized, "Rss").is_err());
    for key in ["", "VmRSS:", "Vm RSS", "VmRSS\n"] {
        ensure!(parse_kib_field("VmRSS: 1 kB\n", key).is_err());
    }
    Ok(())
}

#[test]
fn unavailable_metrics_round_trip_without_becoming_zero() -> Result<()> {
    let memory = ProcessMemory {
        pid: 42,
        rss_bytes: Metric::Available { bytes: 0 },
        pss_bytes: Metric::Unavailable {
            reason: "permission denied".into(),
        },
        peak_rss_bytes: Metric::Available { bytes: 1024 },
    };
    let json = serde_json::to_string(&memory)?;
    ensure!(json.contains("\"status\":\"unavailable\""));
    ensure!(json.contains("permission denied"));
    ensure!(serde_json::from_str::<ProcessMemory>(&json)? == memory);
    ensure!(memory.rss_bytes.bytes() == Some(0));
    ensure!(memory.pss_bytes.bytes().is_none());
    ensure!(memory.peak_rss_bytes.bytes() == Some(1024));
    Ok(())
}

#[test]
fn absent_process_has_unavailable_metrics() -> Result<()> {
    // Linux process identifiers cannot reach this unsigned value.
    let memory = proc_metrics::sample(u32::MAX)?;
    for metric in [memory.rss_bytes, memory.pss_bytes, memory.peak_rss_bytes] {
        let Metric::Unavailable { reason } = metric else {
            anyhow::bail!("absent process unexpectedly produced a memory measurement");
        };
        ensure!(!reason.is_empty());
    }
    Ok(())
}
