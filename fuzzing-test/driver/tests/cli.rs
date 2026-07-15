use std::{error::Error, process::Command};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn help_describes_manual_and_bounded_campaigns() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_velum-fuzz"))
        .arg("--help")
        .output()?;
    if !output.status.success() {
        return Err(format!("help command failed with {}", output.status).into());
    }
    let stdout = String::from_utf8(output.stdout)?;
    if stdout.contains("Ctrl-C") && stdout.contains("--iterations") && stdout.contains("--output") {
        return Ok(());
    }
    Err(format!("help output is incomplete: {stdout}").into())
}

#[test]
fn rejects_zero_jobs() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_velum-fuzz"))
        .args(["--jobs", "0"])
        .output()?;
    if !output.status.success() {
        return Ok(());
    }
    Err("zero jobs unexpectedly succeeded".into())
}
