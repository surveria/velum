use std::{
    fs::{self, OpenOptions},
    io::{BufRead as _, BufReader, Write as _},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use anyhow::{Context as _, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::compare::{EngineOutcome, OutcomeStatus, error_name_from_text, outcome};

const NODE_WORKER_SOURCE: &str = r"
const readline = require('readline');
const vm = require('vm');

process.on('unhandledRejection', (reason) => {
  const message = reason && reason.stack ? reason.stack : String(reason);
  process.stderr.write(`[unhandledRejection] ${message}\n`);
});

process.on('uncaughtException', (error) => {
  const message = error && error.stack ? error.stack : String(error);
  process.stderr.write(`[uncaughtException] ${message}\n`);
});

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

function writeResponse(response) {
  process.stdout.write(`${JSON.stringify(response)}\n`);
}

function makeSandbox(output) {
  return {
    fuzzilli(operation, value) {
      if (operation === 'FUZZILLI_PRINT') {
        output.push(String(value));
      }
    },
  };
}

rl.on('line', (line) => {
  try {
    const request = JSON.parse(line);
    const source = Buffer.from(request.sourceBase64, 'base64').toString('utf8');
    const output = [];
    const started = process.hrtime.bigint();
    let status = 'ok';
    let errorName = null;
    let errorMessage = null;
    try {
      vm.runInNewContext(source, makeSandbox(output), {
        timeout: request.timeoutMs,
        displayErrors: false,
      });
    } catch (error) {
      errorName = error && error.name ? String(error.name) : 'Error';
      errorMessage = error && error.message ? String(error.message) : String(error);
      status = errorMessage.includes('Script execution timed out') ? 'timeout' : 'js_error';
    }
    const elapsedNanos = process.hrtime.bigint() - started;
    writeResponse({
      status,
      stdout: output.length === 0 ? '' : `${output.join('\n')}\n`,
      errorName,
      errorMessage,
      elapsedNanos: elapsedNanos.toString(),
    });
  } catch (error) {
    writeResponse({
      status: 'crash',
      stdout: '',
      errorName: 'WorkerError',
      errorMessage: error && error.message ? String(error.message) : String(error),
      elapsedNanos: '0',
    });
  }
});
";

pub struct NodeWorker {
    node_binary: String,
    stderr_dir: PathBuf,
    owner_pid: u32,
    restart_count: u64,
    current_stderr: PathBuf,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl NodeWorker {
    /// Starts a persistent Node/V8 worker.
    ///
    /// # Errors
    ///
    /// Returns an error when Node cannot be spawned or its pipes are missing.
    pub fn start(node_binary: impl Into<String>, stderr_dir: PathBuf) -> anyhow::Result<Self> {
        let node_binary = node_binary.into();
        fs::create_dir_all(&stderr_dir)
            .with_context(|| format!("failed to create '{}'", stderr_dir.display()))?;
        let owner_pid = std::process::id();
        let restart_count = 0;
        let current_stderr = stderr_path(&stderr_dir, owner_pid, restart_count);
        let (child, stdin, stdout) = spawn_worker(&node_binary, &current_stderr)?;
        Ok(Self {
            node_binary,
            stderr_dir,
            owner_pid,
            restart_count,
            current_stderr,
            child,
            stdin,
            stdout,
        })
    }

    /// Executes one JavaScript source string in the persistent V8 worker.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker protocol returns invalid JSON.
    pub fn execute(&mut self, source: &str, timeout_ms: u64) -> anyhow::Result<EngineOutcome> {
        let request = NodeRequest {
            source_base64: STANDARD.encode(source.as_bytes()),
            timeout_ms,
        };
        if let Err(error) = self.write_request(&request) {
            let reason = format!("failed to write to Node worker: {error:#}");
            let message = self.restart_after_failure(&reason);
            return Ok(worker_crash(message));
        }
        let mut line = String::new();
        let read = self
            .stdout
            .read_line(&mut line)
            .context("failed to read from Node worker")?;
        if read == 0 {
            let message = self.restart_after_failure("Node worker exited without a response");
            return Ok(worker_crash(message));
        }
        let response: NodeResponse =
            serde_json::from_str(&line).context("Node worker returned invalid JSON")?;
        Ok(response.into_outcome())
    }

    fn write_request(&mut self, request: &NodeRequest) -> anyhow::Result<()> {
        serde_json::to_writer(&mut self.stdin, request)
            .context("failed to serialize Node worker request")?;
        self.stdin
            .write_all(b"\n")
            .context("failed to terminate Node worker request")?;
        self.stdin
            .flush()
            .context("failed to flush Node worker request")
    }

    fn restart_after_failure(&mut self, reason: &str) -> String {
        let old_stderr = self.current_stderr.clone();
        let status = match self.child.try_wait() {
            Ok(Some(status)) => Some(status.to_string()),
            Ok(None) => {
                if let Err(_error) = self.child.kill() {}
                self.child.wait().ok().map(|value| value.to_string())
            }
            Err(error) => Some(format!("failed to poll Node worker: {error}")),
        };
        self.restart_count = self.restart_count.saturating_add(1);
        let next_stderr = stderr_path(&self.stderr_dir, self.owner_pid, self.restart_count);
        if let Ok((child, stdin, stdout)) = spawn_worker(&self.node_binary, &next_stderr) {
            self.child = child;
            self.stdin = stdin;
            self.stdout = stdout;
            self.current_stderr = next_stderr;
        }
        format!(
            "{reason}; status: {}; stderr: {}",
            status.as_deref().unwrap_or("unavailable"),
            old_stderr.display()
        )
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeRequest {
    source_base64: String,
    timeout_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeResponse {
    status: String,
    stdout: String,
    error_name: Option<String>,
    error_message: Option<String>,
    elapsed_nanos: String,
}

impl NodeResponse {
    fn into_outcome(self) -> EngineOutcome {
        let status = match self.status.as_str() {
            "ok" => OutcomeStatus::Ok,
            "js_error" => OutcomeStatus::JsError,
            "timeout" => OutcomeStatus::Timeout,
            _ => OutcomeStatus::Crash,
        };
        let elapsed_nanos = self.elapsed_nanos.parse::<u64>().unwrap_or(u64::MAX);
        let error_name = self
            .error_name
            .or_else(|| self.error_message.as_deref().map(error_name_from_text));
        outcome(
            status,
            elapsed_nanos,
            &self.stdout,
            error_name,
            self.error_message,
        )
    }
}

fn spawn_worker(
    node_binary: &str,
    stderr_path: &Path,
) -> anyhow::Result<(Child, ChildStdin, BufReader<ChildStdout>)> {
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(stderr_path)
        .with_context(|| format!("failed to open '{}'", stderr_path.display()))?;
    let mut child = Command::new(node_binary)
        .arg("--eval")
        .arg(NODE_WORKER_SOURCE)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("failed to start Node/V8 worker '{node_binary}'"))?;
    let stdin = child.stdin.take().context("Node worker stdin is missing")?;
    let stdout = child
        .stdout
        .take()
        .context("Node worker stdout is missing")?;
    ensure!(
        child
            .try_wait()
            .context("failed to poll Node worker")?
            .is_none(),
        "Node worker exited during startup"
    );
    Ok((child, stdin, BufReader::new(stdout)))
}

fn stderr_path(directory: &Path, owner_pid: u32, restart_count: u64) -> PathBuf {
    directory.join(format!(
        "node-worker-{owner_pid}-{restart_count}.stderr.log"
    ))
}

fn worker_crash(message: String) -> EngineOutcome {
    outcome(
        OutcomeStatus::Crash,
        0,
        "",
        Some("WorkerCrash".to_owned()),
        Some(message),
    )
}
