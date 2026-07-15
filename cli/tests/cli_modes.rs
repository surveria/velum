use std::{
    io::Write as _,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context as _, Result, ensure};

const BINARY: &str = env!("CARGO_BIN_EXE_velum");

#[test]
fn eval_mode_prints_host_output_and_completion_value() -> Result<()> {
    let output = Command::new(BINARY)
        .args(["--eval", "print('hello'); 40 + 2"])
        .output()
        .context("failed to run eval mode")?;
    ensure!(output.status.success(), "eval mode failed");
    ensure!(
        String::from_utf8(output.stdout)? == "hello\n42\n",
        "unexpected eval stdout"
    );
    ensure!(output.stderr.is_empty(), "eval mode wrote stderr");
    Ok(())
}

#[test]
fn file_mode_runs_a_javascript_path() -> Result<()> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("file_mode.js");
    let output = Command::new(BINARY)
        .arg(fixture)
        .output()
        .context("failed to run file mode")?;
    ensure!(output.status.success(), "file mode failed");
    ensure!(
        String::from_utf8(output.stdout)? == "file mode\n42\n",
        "unexpected file stdout"
    );
    ensure!(output.stderr.is_empty(), "file mode wrote stderr");
    Ok(())
}

#[test]
fn piped_stdin_is_evaluated_as_one_multiline_script() -> Result<()> {
    let mut child = Command::new(BINARY)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start stdin mode")?;
    let mut stdin = child
        .stdin
        .take()
        .context("stdin mode did not expose a writable pipe")?;
    stdin.write_all(b"function answer() {\n  return 42;\n}\nanswer();\n")?;
    drop(stdin);

    let output = child
        .wait_with_output()
        .context("failed to wait for stdin mode")?;
    ensure!(output.status.success(), "stdin mode failed");
    ensure!(
        String::from_utf8(output.stdout)? == "42\n",
        "unexpected stdin stdout"
    );
    ensure!(output.stderr.is_empty(), "stdin mode wrote stderr");
    Ok(())
}

#[test]
fn invalid_source_returns_a_nonzero_status() -> Result<()> {
    let output = Command::new(BINARY)
        .args(["--eval", "throw new Error('failed')"])
        .output()
        .context("failed to run invalid eval mode")?;
    ensure!(
        !output.status.success(),
        "invalid source unexpectedly passed"
    );
    let stderr = String::from_utf8(output.stderr)?;
    ensure!(
        stderr.contains("failed"),
        "missing JavaScript error context"
    );
    Ok(())
}
