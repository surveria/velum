use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
    process,
};

use anyhow::{Context as _, bail};
use rs_quickjs::{Runtime, Value};
use tabled::{Table, Tabled};

const USAGE: &str = "usage: rsqjs-test-runner --report <path>";
const STATUS_PASSED: &str = "passed";
const STATUS_FAILED: &str = "failed";
const STATUS_SKIPPED: &str = "skipped";
const SUITE_ENGINE: &str = "engine";
const SUITE_TEST262: &str = "test262";
const SUITE_QUICKJS: &str = "quickjs-reference";
const REPORT_TITLE: &str = "# rs-quickjs Test Report";
const RUNNER_NAME: &str = "`rsqjs-test-runner`";
const BASIS_POINTS_SCALE: usize = 10_000;
const PERCENT_SCALE: usize = 100;

const CASE_ARITHMETIC: &str = "arithmetic_precedence";
const CASE_HOST_PRINT: &str = "host_print";
const CASE_CONST_ASSIGNMENT: &str = "const_assignment_error";
const CASE_SHORT_CIRCUIT: &str = "short_circuit";
const CASE_TEST262: &str = "test262_corpus";
const CASE_QUICKJS: &str = "quickjs_differential";

const PATH_ARITHMETIC: &str = "tests/engine_cases/arithmetic_precedence.js";
const PATH_HOST_PRINT: &str = "tests/engine_cases/host_print.js";
const PATH_CONST_ASSIGNMENT: &str = "tests/engine_cases/const_assignment_error.js";
const PATH_SHORT_CIRCUIT: &str = "tests/engine_cases/short_circuit.js";

const EXPECTED_ARITHMETIC_VALUE: &str = "5";
const EXPECTED_HOST_PRINT_VALUE: &str = "id-7";
const EXPECTED_HOST_PRINT_OUTPUT: &[&str] = &["hello camera"];
const EXPECTED_SHORT_CIRCUIT_VALUE: &str = "ok";
const EXPECTED_CONST_ASSIGNMENT_ERROR: &str = "assignment to constant";

const REASON_MATCHED: &str = "matched expected behavior";
const REASON_TEST262_SKIPPED: &str = "Test262 corpus integration is not wired into the runner yet";
const REASON_QUICKJS_SKIPPED: &str =
    "QuickJS reference binary and differential harness are not wired yet";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let config = Config::from_args(env::args().skip(1))?;
    let report = run_all_cases();
    write_report(&config.report_path, &report)?;

    if report.failed == 0 {
        return Ok(());
    }

    bail!(
        "test runner recorded {} failed case(s); report written to {}",
        report.failed,
        config.report_path.display()
    )
}

#[derive(Debug)]
struct Config {
    report_path: PathBuf,
}

impl Config {
    fn from_args(mut args: impl Iterator<Item = String>) -> anyhow::Result<Self> {
        let Some(flag) = args.next() else {
            bail!("{USAGE}");
        };
        if flag != "--report" {
            bail!("unknown argument '{flag}'; {USAGE}");
        }

        let report_path = args.next().context("missing path after --report")?;
        if let Some(extra) = args.next() {
            bail!("unexpected argument '{extra}'; {USAGE}");
        }

        Ok(Self {
            report_path: PathBuf::from(report_path),
        })
    }
}

#[derive(Debug)]
struct EngineCase {
    id: &'static str,
    path: &'static str,
    expectation: Expectation,
}

#[derive(Debug)]
enum Expectation {
    Value(&'static str),
    OutputAndValue {
        output: &'static [&'static str],
        value: &'static str,
    },
    ErrorContains(&'static str),
}

#[derive(Debug)]
struct SkipCase {
    suite: &'static str,
    id: &'static str,
    reason: &'static str,
}

#[derive(Debug)]
struct TestReport {
    rows: Vec<ReportRow>,
    passed: usize,
    failed: usize,
    skipped: usize,
}

impl TestReport {
    const fn total(&self) -> usize {
        self.passed
            .saturating_add(self.failed)
            .saturating_add(self.skipped)
    }
}

#[derive(Debug, Tabled)]
struct ReportRow {
    suite: String,
    case: String,
    status: String,
    source: String,
    detail: String,
}

fn run_all_cases() -> TestReport {
    let mut report = TestReport {
        rows: Vec::new(),
        passed: 0,
        failed: 0,
        skipped: 0,
    };

    for case in engine_cases() {
        let row = run_engine_case(&case);
        report.record(row);
    }

    for case in skipped_cases() {
        report.record(ReportRow {
            suite: case.suite.to_owned(),
            case: case.id.to_owned(),
            status: STATUS_SKIPPED.to_owned(),
            source: "-".to_owned(),
            detail: case.reason.to_owned(),
        });
    }

    report
}

impl TestReport {
    fn record(&mut self, row: ReportRow) {
        if row.status == STATUS_PASSED {
            self.passed = self.passed.saturating_add(1);
        } else if row.status == STATUS_SKIPPED {
            self.skipped = self.skipped.saturating_add(1);
        } else {
            self.failed = self.failed.saturating_add(1);
        }
        self.rows.push(row);
    }
}

fn engine_cases() -> Vec<EngineCase> {
    vec![
        EngineCase {
            id: CASE_ARITHMETIC,
            path: PATH_ARITHMETIC,
            expectation: Expectation::Value(EXPECTED_ARITHMETIC_VALUE),
        },
        EngineCase {
            id: CASE_HOST_PRINT,
            path: PATH_HOST_PRINT,
            expectation: Expectation::OutputAndValue {
                output: EXPECTED_HOST_PRINT_OUTPUT,
                value: EXPECTED_HOST_PRINT_VALUE,
            },
        },
        EngineCase {
            id: CASE_CONST_ASSIGNMENT,
            path: PATH_CONST_ASSIGNMENT,
            expectation: Expectation::ErrorContains(EXPECTED_CONST_ASSIGNMENT_ERROR),
        },
        EngineCase {
            id: CASE_SHORT_CIRCUIT,
            path: PATH_SHORT_CIRCUIT,
            expectation: Expectation::Value(EXPECTED_SHORT_CIRCUIT_VALUE),
        },
    ]
}

fn skipped_cases() -> Vec<SkipCase> {
    vec![
        SkipCase {
            suite: SUITE_TEST262,
            id: CASE_TEST262,
            reason: REASON_TEST262_SKIPPED,
        },
        SkipCase {
            suite: SUITE_QUICKJS,
            id: CASE_QUICKJS,
            reason: REASON_QUICKJS_SKIPPED,
        },
    ]
}

fn run_engine_case(case: &EngineCase) -> ReportRow {
    match execute_engine_case(case) {
        Ok(()) => ReportRow {
            suite: SUITE_ENGINE.to_owned(),
            case: case.id.to_owned(),
            status: STATUS_PASSED.to_owned(),
            source: case.path.to_owned(),
            detail: REASON_MATCHED.to_owned(),
        },
        Err(error) => ReportRow {
            suite: SUITE_ENGINE.to_owned(),
            case: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            detail: error.to_string(),
        },
    }
}

fn execute_engine_case(case: &EngineCase) -> anyhow::Result<()> {
    let source = fs::read_to_string(case.path)
        .with_context(|| format!("failed to read test source '{}'", case.path))?;

    let runtime = Runtime::new();
    let mut context = runtime.context();
    let result = context.eval(&source);

    match &case.expectation {
        Expectation::Value(expected) => {
            let value = result
                .with_context(|| format!("case '{}' failed while evaluating source", case.id))?;
            ensure_value(case.id, &value, expected)?;
            ensure_output(case.id, context.output(), &[])?;
        }
        Expectation::OutputAndValue { output, value } => {
            let actual = result
                .with_context(|| format!("case '{}' failed while evaluating source", case.id))?;
            ensure_value(case.id, &actual, value)?;
            ensure_output(case.id, context.output(), output)?;
        }
        Expectation::ErrorContains(expected) => {
            let Err(error) = result else {
                bail!("case '{}' expected error containing '{expected}'", case.id);
            };
            let message = error.to_string();
            if !message.contains(expected) {
                bail!(
                    "case '{}' expected error containing '{}', got '{}'",
                    case.id,
                    expected,
                    message
                );
            }
        }
    }

    Ok(())
}

fn ensure_value(case_id: &str, actual: &Value, expected: &str) -> anyhow::Result<()> {
    let actual_text = actual.to_string();
    if actual_text == expected {
        return Ok(());
    }

    bail!("case '{case_id}' expected value '{expected}', got '{actual_text}'")
}

fn ensure_output(case_id: &str, actual: &[String], expected: &[&str]) -> anyhow::Result<()> {
    if actual.len() != expected.len() {
        bail!(
            "case '{}' expected output {}, got {}",
            case_id,
            DisplaySlice(expected),
            DisplaySlice(actual)
        );
    }

    for (actual_line, expected_line) in actual.iter().zip(expected.iter()) {
        if actual_line != expected_line {
            bail!(
                "case '{}' expected output {}, got {}",
                case_id,
                DisplaySlice(expected),
                DisplaySlice(actual)
            );
        }
    }

    Ok(())
}

fn write_report(path: &Path, report: &TestReport) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create report directory '{}'", parent.display()))?;
    }

    let table = Table::new(&report.rows).to_string();
    let body = format!(
        "{REPORT_TITLE}\n\nGenerated by {RUNNER_NAME}.\n\n\
         ## Summary\n\n\
         - Total: {}\n\
         - Passed: {} ({})\n\
         - Failed: {} ({})\n\
         - Skipped: {} ({})\n\n\
         ## Cases\n\n```text\n{}\n```\n",
        report.total(),
        report.passed,
        percent(report.passed, report.total()),
        report.failed,
        percent(report.failed, report.total()),
        report.skipped,
        percent(report.skipped, report.total()),
        table
    );

    fs::write(path, body)
        .with_context(|| format!("failed to write test report '{}'", path.display()))
}

fn percent(part: usize, total: usize) -> String {
    if total == 0 {
        return "0.00%".to_owned();
    }

    let basis_points = part.saturating_mul(BASIS_POINTS_SCALE) / total;
    let major = basis_points / PERCENT_SCALE;
    let minor = basis_points % PERCENT_SCALE;
    format!("{major}.{minor:02}%")
}

struct DisplaySlice<'a, T>(&'a [T]);

impl<T: fmt::Display> fmt::Display for DisplaySlice<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[")?;
        let mut first = true;
        for item in self.0 {
            if first {
                first = false;
            } else {
                formatter.write_str(", ")?;
            }
            write!(formatter, "\"{item}\"")?;
        }
        formatter.write_str("]")
    }
}
