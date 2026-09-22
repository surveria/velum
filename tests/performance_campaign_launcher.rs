use std::{path::Path, process::Command};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn launch(arguments: &[&str]) -> std::io::Result<std::process::Output> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/run-performance-campaign.sh");
    Command::new("bash").arg(script).args(arguments).output()
}

#[test]
fn campaign_help_does_not_start_measurement_or_compilation() -> TestResult {
    let output = launch(&["--help"])?;
    let stdout = String::from_utf8(output.stdout)?;
    if !output.status.success()
        || !stdout.contains("representative|holdout|embedding|jetstream|memory")
        || stdout.contains("Starting ")
        || stdout.contains("Rebuilding ")
    {
        return Err(format!(
            "unexpected campaign help response: {} {stdout}",
            output.status
        )
        .into());
    }
    Ok(())
}

#[test]
fn invalid_campaign_arguments_fail_before_any_work() -> TestResult {
    for (arguments, expected) in [
        (vec!["--lane", "unknown"], "unknown lane"),
        (vec!["--lane"], "missing value"),
        (vec!["--lane", "--help"], "missing value"),
        (vec!["--artifact-root", "relative"], "absolute path"),
        (vec!["--artifact-root"], "missing value"),
        (vec!["--unknown"], "unknown argument"),
    ] {
        let output = launch(&arguments)?;
        let stderr = String::from_utf8(output.stderr)?;
        if output.status.success() || !stderr.contains(expected) || !output.stdout.is_empty() {
            return Err(format!(
                "invalid campaign arguments {arguments:?} did not fail before work: {} {stderr}",
                output.status
            )
            .into());
        }
    }
    Ok(())
}
