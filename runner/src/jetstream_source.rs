use std::fs;

use anyhow::Context as _;

use super::jetstream_model::{QUICKJS_PERFORMANCE_PRELUDE, SHELL_PRELUDE, SYNC_HARNESS};

pub fn workload_source(files: &[&str]) -> anyhow::Result<String> {
    let mut script = String::new();
    for file in files {
        let source = fs::read_to_string(file)
            .with_context(|| format!("failed to read JetStream source '{file}'"))?;
        script.push_str("// JetStream source: ");
        script.push_str(file);
        script.push('\n');
        script.push_str(&source);
        script.push('\n');
    }
    Ok(script)
}

pub fn benchmark_source_from_workload(workload: &str) -> String {
    format!("{SHELL_PRELUDE}\n{workload}{SYNC_HARNESS}")
}

pub fn quickjs_source_from_workload(workload: &str) -> String {
    format!("{QUICKJS_PERFORMANCE_PRELUDE}\n{SHELL_PRELUDE}\n{workload}{SYNC_HARNESS}")
}

pub fn harness_descriptor() -> String {
    format!(
        "timer=rust-instant\nreference-performance-prelude:\n{QUICKJS_PERFORMANCE_PRELUDE}\nprelude:\n{SHELL_PRELUDE}\nharness:\n{SYNC_HARNESS}"
    )
}

#[cfg(test)]
pub fn benchmark_source(files: &[&str]) -> anyhow::Result<String> {
    workload_source(files).map(|workload| benchmark_source_from_workload(&workload))
}
