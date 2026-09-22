//! Isolated physical-memory measurements and separately labeled VM accounting.

use std::{env, path::Path, time::Instant};

use anyhow::{Context as _, bail};

mod config;
mod engines;
mod model;
mod proc_metrics;
mod process;
mod protocol;
mod report;
mod worker;

use config::Config;
use model::{EngineKind, Outcome, PROTOCOL_VERSION, RunMeasurement, WorkerConfig};

pub fn run(report_path: &Path) -> anyhow::Result<()> {
    let config = Config::from_env()?;
    let executable = env::current_exe().context("failed to locate memory worker executable")?;
    let mut report = report::MemoryReport::new(config.clone(), &executable)?;
    // Publish a recoverable empty report before waiting for the measured host slot.
    report.write(report_path)?;
    let started = Instant::now();
    for scenario in &config.scenarios {
        for repetition in 0..config.repetitions {
            let engines = if repetition % 2 == 0 {
                [EngineKind::Velum, EngineKind::Quickjs]
            } else {
                [EngineKind::Quickjs, EngineKind::Velum]
            };
            for engine in engines {
                let worker = WorkerConfig {
                    protocol_version: PROTOCOL_VERSION,
                    engine,
                    scenario: scenario.clone(),
                    timeout_ms: config.child_timeout_ms,
                };
                config::validate_worker(&worker)?;
                let result = if engine == EngineKind::Quickjs && !config.quickjs_compiled {
                    RunMeasurement {
                            scenario_id: scenario.id.clone(), engine, repetition,
                            outcome: Outcome::Skipped,
                            detail: "QuickJS memory reference was not compiled; rebuild runner with --features reference-quickjs".to_owned(),
                            elapsed_ns: 0, phases: Vec::new(),
                        }
                } else {
                    crate::host_benchmark_lock::with_exclusive(
                        "process-isolated memory worker",
                        || Ok(process::run(&executable, &worker, repetition)),
                    )?
                };
                println!(
                    "memory worker: scenario={} engine={} repetition={} outcome={:?}",
                    scenario.id,
                    engine.label(),
                    repetition,
                    result.outcome
                );
                report.runs.push(result);
                report.set_elapsed(started.elapsed())?;
                report.write(report_path)?;
            }
        }
    }
    report.set_elapsed(started.elapsed())?;
    report.write(report_path)?;
    report.print_summary(report_path);
    let failures = report
        .runs
        .iter()
        .filter(|run| run.outcome == Outcome::Failed)
        .count();
    if failures > 0 {
        bail!(
            "memory campaign recorded {failures} failed workers; see {}",
            report_path.display()
        );
    }
    Ok(())
}

pub fn run_worker(encoded_config: &str) -> anyhow::Result<()> {
    worker::run(encoded_config)
}
