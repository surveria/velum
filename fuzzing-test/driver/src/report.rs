use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, bail};
use serde_json::{Map, Value};
use tabled::{Table, Tabled};

const LATEST_FINDING_LIMIT: usize = 10;

#[derive(Debug, Default)]
pub struct SessionSnapshot {
    unique_crashes: BTreeSet<PathBuf>,
    duplicate_crashes: BTreeSet<PathBuf>,
    timeouts: BTreeSet<PathBuf>,
    statistics: BTreeSet<PathBuf>,
}

impl SessionSnapshot {
    /// Captures the saved findings and exported statistics currently in a session.
    ///
    /// # Errors
    ///
    /// Returns an error when an existing session directory cannot be read.
    pub fn capture(run_dir: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            unique_crashes: javascript_files(&run_dir.join("crashes"))?,
            duplicate_crashes: javascript_files(&run_dir.join("crashes/duplicates"))?,
            timeouts: javascript_files(&run_dir.join("timeouts"))?,
            statistics: files_with_extension(&run_dir.join("stats"), "json")?,
        })
    }
}

#[derive(Debug)]
pub struct SessionReport {
    table: String,
    problem_files: Vec<PathBuf>,
    log_path: PathBuf,
}

impl SessionReport {
    /// Renders the summary table, detailed log path, and bounded finding list.
    #[must_use]
    pub fn render(&self) -> String {
        let mut output = format!("{}\nDetailed log: {}", self.table, self.log_path.display());
        if self.problem_files.is_empty() {
            output.push_str("\nNo new problem files were saved.");
            return output;
        }
        output.push_str("\nLatest saved problem files (showing ");
        output.push_str(
            &self
                .problem_files
                .len()
                .min(LATEST_FINDING_LIMIT)
                .to_string(),
        );
        output.push_str(" of ");
        output.push_str(&self.problem_files.len().to_string());
        output.push_str("):");
        for path in self.problem_files.iter().take(LATEST_FINDING_LIMIT) {
            output.push_str("\n- ");
            output.push_str(&path.display().to_string());
        }
        output
    }

    /// Appends the rendered summary to the detailed session log.
    ///
    /// # Errors
    ///
    /// Returns an error when the log cannot be opened or written.
    pub fn append_to_log(&self) -> anyhow::Result<()> {
        let mut log = OpenOptions::new()
            .append(true)
            .open(&self.log_path)
            .with_context(|| format!("failed to reopen log '{}'", self.log_path.display()))?;
        writeln!(
            log,
            "\n===== Velum fuzzing summary =====\n{}",
            self.render()
        )
        .with_context(|| format!("failed to append summary to '{}'", self.log_path.display()))?;
        if self.problem_files.is_empty() {
            return Ok(());
        }
        writeln!(log, "\n===== All new saved problem files =====")
            .context("failed to write the complete problem file heading")?;
        for path in &self.problem_files {
            writeln!(log, "- {}", path.display())
                .context("failed to write a complete problem file path")?;
        }
        Ok(())
    }
}

#[derive(Tabled)]
struct SummaryRow {
    #[tabled(rename = "Metric")]
    metric: &'static str,
    #[tabled(rename = "Value")]
    value: String,
}

#[derive(Debug, Default)]
struct FuzzilliStatistics {
    total_samples: u64,
    valid_samples: u64,
    interesting_samples: u64,
    timed_out_samples: u64,
    crashing_samples: u64,
    total_execs: u64,
}

/// Builds a report for the files and statistics added after `before`.
///
/// # Errors
///
/// Returns an error when session files cannot be read, statistics are invalid,
/// or aggregate counters overflow.
pub fn build_report(
    run_dir: &Path,
    before: &SessionSnapshot,
    elapsed: Duration,
    outcome: &str,
    log_path: &Path,
) -> anyhow::Result<SessionReport> {
    let after = SessionSnapshot::capture(run_dir)?;
    let unique_crashes = new_files(&after.unique_crashes, &before.unique_crashes);
    let duplicate_crashes = new_files(&after.duplicate_crashes, &before.duplicate_crashes);
    let timeouts = new_files(&after.timeouts, &before.timeouts);
    let statistics_path = new_files(&after.statistics, &before.statistics).pop();
    let statistics = statistics_path
        .as_deref()
        .map(read_statistics)
        .transpose()?;

    let observed_problems = statistics
        .as_ref()
        .map(|value| {
            value
                .crashing_samples
                .checked_add(value.timed_out_samples)
                .context("Fuzzilli problem count overflow")
        })
        .transpose()?;

    let rows = vec![
        SummaryRow {
            metric: "Fuzzilli outcome",
            value: outcome.to_owned(),
        },
        SummaryRow {
            metric: "Elapsed",
            value: humantime::format_duration(elapsed).to_string(),
        },
        statistics_row("Generated test cases", statistics.as_ref(), |value| {
            value.total_samples
        }),
        statistics_row("Valid test cases", statistics.as_ref(), |value| {
            value.valid_samples
        }),
        statistics_row("Engine executions", statistics.as_ref(), |value| {
            value.total_execs
        }),
        statistics_row("Coverage corpus additions", statistics.as_ref(), |value| {
            value.interesting_samples
        }),
        statistics_row("Crash events", statistics.as_ref(), |value| {
            value.crashing_samples
        }),
        statistics_row("Timeout events", statistics.as_ref(), |value| {
            value.timed_out_samples
        }),
        SummaryRow {
            metric: "Problems observed",
            value: optional_count(observed_problems),
        },
        SummaryRow {
            metric: "New unique crash files",
            value: unique_crashes.len().to_string(),
        },
        SummaryRow {
            metric: "New duplicate crash files",
            value: duplicate_crashes.len().to_string(),
        },
        SummaryRow {
            metric: "New timeout files",
            value: timeouts.len().to_string(),
        },
    ];

    let mut problem_files = unique_crashes;
    problem_files.extend(duplicate_crashes);
    problem_files.extend(timeouts);
    problem_files.sort();
    problem_files.reverse();

    Ok(SessionReport {
        table: Table::new(rows).to_string(),
        problem_files,
        log_path: log_path.to_path_buf(),
    })
}

fn statistics_row(
    metric: &'static str,
    statistics: Option<&FuzzilliStatistics>,
    value: impl FnOnce(&FuzzilliStatistics) -> u64,
) -> SummaryRow {
    SummaryRow {
        metric,
        value: statistics.map_or_else(
            || "unavailable".to_owned(),
            |stats| value(stats).to_string(),
        ),
    }
}

fn optional_count(value: Option<u64>) -> String {
    value.map_or_else(|| "unavailable".to_owned(), |count| count.to_string())
}

fn new_files(after: &BTreeSet<PathBuf>, before: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
    after.difference(before).cloned().collect()
}

fn javascript_files(directory: &Path) -> anyhow::Result<BTreeSet<PathBuf>> {
    files_with_extension(directory, "js")
}

fn files_with_extension(directory: &Path, extension: &str) -> anyhow::Result<BTreeSet<PathBuf>> {
    if !directory.is_dir() {
        return Ok(BTreeSet::new());
    }
    let mut files = BTreeSet::new();
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read '{}'", directory.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read '{}'", directory.display()))?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some(extension) {
            files.insert(path);
        }
    }
    Ok(files)
}

fn read_statistics(path: &Path) -> anyhow::Result<FuzzilliStatistics> {
    let data = fs::read(path)
        .with_context(|| format!("failed to read Fuzzilli statistics '{}'", path.display()))?;
    let value: Value = serde_json::from_slice(&data)
        .with_context(|| format!("failed to parse Fuzzilli statistics '{}'", path.display()))?;
    let object = value
        .as_object()
        .with_context(|| format!("Fuzzilli statistics '{}' are not an object", path.display()))?;
    Ok(FuzzilliStatistics {
        total_samples: json_u64(object, "totalSamples")?,
        valid_samples: json_u64(object, "validSamples")?,
        interesting_samples: json_u64(object, "interestingSamples")?,
        timed_out_samples: json_u64(object, "timedOutSamples")?,
        crashing_samples: json_u64(object, "crashingSamples")?,
        total_execs: json_u64(object, "totalExecs")?,
    })
}

fn json_u64(object: &Map<String, Value>, key: &str) -> anyhow::Result<u64> {
    let Some(value) = object.get(key) else {
        return Ok(0);
    };
    if let Some(number) = value.as_u64() {
        return Ok(number);
    }
    if let Some(number) = value.as_str() {
        return number
            .parse::<u64>()
            .with_context(|| format!("Fuzzilli statistic '{key}' is not an unsigned integer"));
    }
    bail!("Fuzzilli statistic '{key}' is not an unsigned integer")
}
