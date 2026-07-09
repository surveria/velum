use std::path::PathBuf;

use anyhow::{Context as _, bail};

use crate::report_rollup;

const USAGE: &str = "usage: rsqjs-test-runner --report <path> | --correctness <path> | --benchmarks <path> | --aggregate-reports <dir>";

#[derive(Debug)]
pub enum Config {
    Run { report_path: PathBuf },
    Correctness { report_path: PathBuf },
    Benchmarks { report_path: PathBuf },
    AggregateReports { report_dir: PathBuf },
}

impl Config {
    pub fn from_args(mut args: impl Iterator<Item = String>) -> anyhow::Result<Self> {
        let Some(flag) = args.next() else {
            bail!("{USAGE}");
        };
        if flag == "--aggregate-reports" {
            let report_dir = args
                .next()
                .context("missing directory after --aggregate-reports")?;
            ensure_no_extra_arg(args)?;
            return Ok(Self::AggregateReports {
                report_dir: PathBuf::from(report_dir),
            });
        }
        if flag == "--benchmarks" {
            let report_path = args.next().context("missing path after --benchmarks")?;
            ensure_no_extra_arg(args)?;
            return Ok(Self::Benchmarks {
                report_path: PathBuf::from(report_path),
            });
        }
        if flag == "--correctness" {
            let report_path = args.next().context("missing path after --correctness")?;
            ensure_no_extra_arg(args)?;
            return Ok(Self::Correctness {
                report_path: PathBuf::from(report_path),
            });
        }
        if flag != "--report" {
            bail!("unknown argument '{flag}'; {USAGE}");
        }

        let report_path = args.next().context("missing path after --report")?;
        ensure_no_extra_arg(args)?;
        Ok(Self::Run {
            report_path: PathBuf::from(report_path),
        })
    }
}

pub fn print_rollup_outputs(outputs: &report_rollup::RollupOutputs) {
    println!("benchmark rollup: {}", outputs.markdown.display());
    println!(
        "benchmark summary chart: {}",
        outputs.summary_chart.display()
    );
}

fn ensure_no_extra_arg(mut args: impl Iterator<Item = String>) -> anyhow::Result<()> {
    if let Some(extra) = args.next() {
        bail!("unexpected argument '{extra}'; {USAGE}");
    }
    Ok(())
}
