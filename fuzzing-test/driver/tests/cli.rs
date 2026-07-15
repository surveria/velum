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
    if stdout.contains("Ctrl-C")
        && stdout.contains("--duration")
        && stdout.contains("--iterations")
        && stdout.contains("--output")
        && stdout.contains("--resume")
        && stdout.contains("--reproduce")
    {
        return Ok(());
    }
    Err(format!("help output is incomplete: {stdout}").into())
}

#[test]
fn rejects_zero_and_malformed_durations() -> TestResult {
    for value in ["0s", "later"] {
        let output = Command::new(env!("CARGO_BIN_EXE_velum-fuzz"))
            .args(["--duration", value])
            .output()?;
        if output.status.success() {
            return Err(format!("invalid duration '{value}' unexpectedly succeeded").into());
        }
    }
    Ok(())
}

#[test]
fn rejects_conflicting_output_modes() -> TestResult {
    let output = Command::new(env!("CARGO_BIN_EXE_velum-fuzz"))
        .args(["--output", "new", "--resume", "old"])
        .output()?;
    if !output.status.success() {
        return Ok(());
    }
    Err("conflicting output modes unexpectedly succeeded".into())
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
