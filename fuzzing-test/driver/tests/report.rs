use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use velum_fuzz_driver::report::{SessionSnapshot, build_report};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn renders_statistics_and_bounded_problem_paths() -> TestResult {
    let directory = unique_test_directory()?;
    fs::create_dir_all(&directory)?;
    let outcome = render_test_session(&directory);
    fs::remove_dir_all(&directory)?;
    outcome
}

fn render_test_session(directory: &Path) -> TestResult {
    let before = SessionSnapshot::capture(directory)?;
    fs::create_dir_all(directory.join("stats"))?;
    fs::create_dir_all(directory.join("crashes/duplicates"))?;
    fs::create_dir_all(directory.join("timeouts"))?;
    fs::write(
        directory.join("stats/20260716010000.json"),
        r#"{
            "totalSamples": "100",
            "validSamples": "80",
            "interestingSamples": "7",
            "timedOutSamples": "2",
            "crashingSamples": "3",
            "totalExecs": "120"
        }"#,
    )?;
    let oldest_crash = directory.join("crashes/program_20260716010000_00.js");
    let newest_crash = directory.join("crashes/program_20260716010000_11.js");
    let timeout = directory.join("timeouts/program_20260716010001_timeout.js");
    for index in 0_u8..12 {
        fs::write(
            directory.join(format!("crashes/program_20260716010000_{index:02}.js")),
            "let value = 1;",
        )?;
    }
    fs::write(&timeout, "while (true) {}")?;
    let log_path = directory.join("fuzzilli.log");
    fs::write(&log_path, "Fuzzilli output")?;

    let report = build_report(
        directory,
        &before,
        Duration::from_secs(30),
        "exit status 0",
        &log_path,
    )?;
    let rendered = report.render();
    report.append_to_log()?;
    let detailed_log = fs::read_to_string(&log_path)?;
    if rendered.contains("Valid test cases")
        && rendered.contains("80")
        && rendered.contains("Problems observed")
        && rendered.contains("| 5")
        && rendered.contains(&newest_crash.display().to_string())
        && rendered.contains(&timeout.display().to_string())
        && !rendered.contains(&oldest_crash.display().to_string())
        && rendered.contains("showing 10 of 13")
        && detailed_log.contains("===== Velum fuzzing summary =====")
        && detailed_log.contains("===== All new saved problem files =====")
        && detailed_log.contains("Valid test cases")
        && detailed_log.contains(&oldest_crash.display().to_string())
    {
        return Ok(());
    }
    Err(format!("report output is incomplete:\n{rendered}").into())
}

fn unique_test_directory() -> Result<PathBuf, Box<dyn Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "velum-fuzz-report-test-{}-{timestamp}",
        std::process::id()
    )))
}
