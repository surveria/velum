use std::{
    io::{Read as _, Write as _},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, anyhow, bail, ensure};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

const BINARY: &str = env!("CARGO_BIN_EXE_velum");
const CONTROL_C: u8 = 3;
const PRIMARY_PROMPT: &[u8] = b"> ";
const PTY_COLUMNS: u16 = 80;
const PTY_ROWS: u16 = 24;
const PTY_READY_DELAY: Duration = Duration::from_millis(50);
const PTY_TIMEOUT: Duration = Duration::from_secs(3);
const PTY_POLL_INTERVAL: Duration = Duration::from_millis(10);
const TEST_TERMINAL: &str = "xterm-256color";

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

#[test]
fn control_c_exits_the_interactive_shell() -> Result<()> {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: PTY_ROWS,
            cols: PTY_COLUMNS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("failed to open a pseudo-terminal")?;
    let mut command = CommandBuilder::new(BINARY);
    command.env("TERM", TEST_TERMINAL);
    let mut child = pair
        .slave
        .spawn_command(command)
        .context("failed to start interactive mode")?;
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .context("failed to clone the pseudo-terminal reader")?;
    let (output_sender, output_receiver) = mpsc::channel();
    let reader_thread = thread::spawn(move || -> Result<Vec<u8>> {
        let mut reader = reader;
        let mut output = Vec::new();
        let mut buffer = [0_u8; 1_024];
        loop {
            let byte_count = reader
                .read(&mut buffer)
                .context("failed to read pseudo-terminal output")?;
            if byte_count == 0 {
                break;
            }
            let chunk = buffer
                .get(..byte_count)
                .context("pseudo-terminal returned an invalid byte count")?
                .to_vec();
            output.extend_from_slice(&chunk);
            output_sender
                .send(chunk)
                .context("failed to forward pseudo-terminal output")?;
        }
        Ok(output)
    });

    wait_for_prompt(&output_receiver)?;
    thread::sleep(PTY_READY_DELAY);
    let mut writer = pair
        .master
        .take_writer()
        .context("failed to take the pseudo-terminal writer")?;
    writer
        .write_all(&[CONTROL_C])
        .context("failed to send Ctrl-C")?;
    writer.flush().context("failed to flush Ctrl-C")?;

    let status = wait_for_child_exit(child.as_mut())?;
    drop(writer);
    drop(pair.master);
    let reader_result = reader_thread
        .join()
        .map_err(|_| anyhow!("pseudo-terminal reader thread failed"))?;
    let output = reader_result?;

    ensure!(
        status.success(),
        "Ctrl-C exited with {status:?}; output: {}",
        String::from_utf8_lossy(&output)
    );
    ensure!(
        String::from_utf8_lossy(&output).contains("^C"),
        "interactive output did not acknowledge Ctrl-C"
    );
    Ok(())
}

fn wait_for_prompt(output_receiver: &Receiver<Vec<u8>>) -> Result<()> {
    let deadline = Instant::now()
        .checked_add(PTY_TIMEOUT)
        .context("pseudo-terminal prompt deadline overflowed")?;
    let mut output = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let chunk = output_receiver
            .recv_timeout(remaining)
            .context("interactive prompt was not displayed")?;
        output.extend_from_slice(&chunk);
        if output
            .windows(PRIMARY_PROMPT.len())
            .any(|window| window == PRIMARY_PROMPT)
        {
            return Ok(());
        }
    }
}

fn wait_for_child_exit(child: &mut dyn portable_pty::Child) -> Result<portable_pty::ExitStatus> {
    let deadline = Instant::now()
        .checked_add(PTY_TIMEOUT)
        .context("pseudo-terminal exit deadline overflowed")?;
    loop {
        if let Some(status) = child
            .try_wait()
            .context("failed to query interactive process status")?
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            child
                .kill()
                .context("failed to terminate unresponsive interactive process")?;
            let status = child
                .wait()
                .context("failed to wait for terminated interactive process")?;
            bail!("interactive process ignored Ctrl-C; terminated with {status:?}");
        }
        thread::sleep(PTY_POLL_INTERVAL);
    }
}
