use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn accepts_the_standard_exec_command() -> TestResult {
    let directory = temporary_directory()?;
    fs::create_dir(&directory)?;
    let result = exercise_empty_program(&directory);
    let cleanup = fs::remove_dir_all(&directory);
    match (result, cleanup) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error.into()),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn temporary_directory() -> Result<PathBuf, Box<dyn Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "velum-reprl-protocol-{}-{timestamp}",
        std::process::id()
    )))
}

fn exercise_empty_program(directory: &Path) -> TestResult {
    let control_input = directory.join("control-input");
    let control_output = directory.join("control-output");
    let data_input = directory.join("data-input");
    let fuzz_output = directory.join("fuzz-output");

    let mut commands = Vec::from(*b"HELOexec");
    commands.extend_from_slice(&0_u64.to_ne_bytes());
    fs::write(&control_input, commands)?;
    fs::write(&data_input, [])?;

    let status = Command::new("bash")
        .args([
            "-c",
            "exec 100<\"$1\" 101>\"$2\" 102<\"$3\" 103>\"$4\"; exec \"$5\" --reprl",
            "velum-reprl-test",
        ])
        .arg(&control_input)
        .arg(&control_output)
        .arg(&data_input)
        .arg(&fuzz_output)
        .arg(env!("CARGO_BIN_EXE_velum-fuzzilli"))
        .status()?;
    if !status.success() {
        return Err(format!("REPRL target exited with {status}").into());
    }

    let mut expected = Vec::from(*b"HELO");
    expected.extend_from_slice(&0_u32.to_ne_bytes());
    let actual = fs::read(&control_output)?;
    if actual != expected {
        return Err(
            format!("unexpected REPRL response: expected {expected:?}, got {actual:?}").into(),
        );
    }
    Ok(())
}
