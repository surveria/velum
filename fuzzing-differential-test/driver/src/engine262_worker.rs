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

const ENGINE262_PARENT_SOURCE: &str = r"
const readline = require('node:readline');
const { Worker } = require('node:worker_threads');

const WORKER_SOURCE = String.raw`
import { parentPort, workerData } from 'node:worker_threads';
import {
  Agent,
  Call,
  CreateBuiltinFunction,
  CreateNonEnumerableDataPropertyOrThrow,
  ManagedRealm,
  ModuleCache,
  setSurroundingAgent,
  ThrowCompletion,
  Value,
  inspect,
  X,
} from '@engine262/engine262';

function errorNameFromText(message) {
  const match = String(message).match(/^([A-Za-z]*Error)(?::|$)/);
  return match ? match[1] : 'Error';
}

function makeErrorOutcome(status, started, error) {
  const message = error && error.stack ? String(error.stack) : String(error);
  return {
    status,
    stdout: '',
    errorName: errorNameFromText(message),
    errorMessage: message,
    elapsedNanos: String(process.hrtime.bigint() - started),
  };
}

function installFuzzilliHook(realm, output) {
  const pop = realm.pushTopContext();
  const stringFunction = realm.Intrinsics['%String%'];
  const fuzzilli = CreateBuiltinFunction.from(function* fuzzilli(
    operation = Value.undefined,
    value = Value.undefined,
  ) {
    const operationCompletion = yield* Call(stringFunction, Value.undefined, [operation]);
    if (operationCompletion instanceof ThrowCompletion) {
      return operationCompletion;
    }
    if (operationCompletion.Value.stringValue() !== 'FUZZILLI_PRINT') {
      return Value.undefined;
    }
    const valueCompletion = yield* Call(stringFunction, Value.undefined, [value]);
    if (valueCompletion instanceof ThrowCompletion) {
      return valueCompletion;
    }
    output.push(valueCompletion.Value.stringValue());
    return Value.undefined;
  }, 'fuzzilli');
  X(CreateNonEnumerableDataPropertyOrThrow(realm.GlobalObject, Value('fuzzilli'), fuzzilli));
  pop?.();
}

function evaluate(source) {
  const started = process.hrtime.bigint();
  const output = [];
  const agent = new Agent({
    uncaughtExceptionTrackers: new Set(),
    hostHooks: {},
  });
  setSurroundingAgent(agent);
  const realm = new ManagedRealm({
    resolverCache: new ModuleCache(),
    name: 'velum-differential-fuzz',
    specifier: process.cwd(),
  });
  installFuzzilliHook(realm, output);
  const completion = realm.evaluateScriptSkipDebugger(source, { specifier: '<fuzzilli>' });
  const elapsedNanos = String(process.hrtime.bigint() - started);
  const stdout = output.length === 0 ? '' : output.join('\n') + '\n';
  if (completion instanceof ThrowCompletion) {
    const pop = realm.pushTopContext();
    const message = inspect(completion.Value);
    pop?.();
    return {
      status: 'js_error',
      stdout,
      errorName: errorNameFromText(message),
      errorMessage: message,
      elapsedNanos,
    };
  }
  return {
    status: 'ok',
    stdout,
    errorName: null,
    errorMessage: null,
    elapsedNanos,
  };
}

try {
  const source = Buffer.from(workerData.sourceBase64, 'base64').toString('utf8');
  parentPort.postMessage(evaluate(source));
} catch (error) {
  parentPort.postMessage(makeErrorOutcome('crash', process.hrtime.bigint(), error));
}
`;

const rl = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

function writeResponse(response) {
  process.stdout.write(`${JSON.stringify(response)}\n`);
}

function timeoutOutcome(started) {
  return {
    status: 'timeout',
    stdout: '',
    errorName: 'TimeoutError',
    errorMessage: 'Engine262 worker timed out',
    elapsedNanos: String(process.hrtime.bigint() - started),
  };
}

function crashOutcome(started, error) {
  const message = error && error.stack ? String(error.stack) : String(error);
  return {
    status: 'crash',
    stdout: '',
    errorName: 'WorkerCrash',
    errorMessage: message,
    elapsedNanos: String(process.hrtime.bigint() - started),
  };
}

function evaluateInWorker(request) {
  return new Promise((resolve) => {
    const started = process.hrtime.bigint();
    let settled = false;
    const worker = new Worker(WORKER_SOURCE, {
      eval: true,
      type: 'module',
      workerData: { sourceBase64: request.sourceBase64 },
    });
    const timer = setTimeout(() => {
      if (settled) {
        return;
      }
      settled = true;
      worker.terminate().finally(() => resolve(timeoutOutcome(started)));
    }, Math.max(1, request.timeoutMs));

    worker.once('message', (response) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      resolve(response);
    });
    worker.once('error', (error) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      resolve(crashOutcome(started, error));
    });
    worker.once('exit', (code) => {
      if (settled || code === 0) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      resolve(crashOutcome(started, `Engine262 worker exited with code ${code}`));
    });
  });
}

rl.on('line', async (line) => {
  try {
    const request = JSON.parse(line);
    writeResponse(await evaluateInWorker(request));
  } catch (error) {
    writeResponse(crashOutcome(process.hrtime.bigint(), error));
  }
});
";

pub struct Engine262Worker {
    node_binary: String,
    package_dir: PathBuf,
    stderr_dir: PathBuf,
    owner_pid: u32,
    restart_count: u64,
    current_stderr: PathBuf,
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Engine262Worker {
    /// Starts a persistent Node parent process for Engine262 comparisons.
    ///
    /// # Errors
    ///
    /// Returns an error when Node cannot be spawned or Engine262 is not
    /// available from the configured package directory.
    pub fn start(
        node_binary: impl Into<String>,
        package_dir: PathBuf,
        stderr_dir: PathBuf,
    ) -> anyhow::Result<Self> {
        let node_binary = node_binary.into();
        fs::create_dir_all(&stderr_dir)
            .with_context(|| format!("failed to create '{}'", stderr_dir.display()))?;
        ensure!(
            package_dir
                .join("node_modules/@engine262/engine262")
                .is_dir(),
            "Engine262 package is missing; run npm ci in {}",
            package_dir.display()
        );
        let owner_pid = std::process::id();
        let restart_count = 0;
        let current_stderr = stderr_path(&stderr_dir, owner_pid, restart_count);
        let (child, stdin, stdout) = spawn_worker(&node_binary, &package_dir, &current_stderr)?;
        Ok(Self {
            node_binary,
            package_dir,
            stderr_dir,
            owner_pid,
            restart_count,
            current_stderr,
            child,
            stdin,
            stdout,
        })
    }

    /// Executes one JavaScript source string through Engine262.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker protocol returns invalid JSON.
    pub fn execute(&mut self, source: &str, timeout_ms: u64) -> anyhow::Result<EngineOutcome> {
        let request = Engine262Request {
            source_base64: STANDARD.encode(source.as_bytes()),
            timeout_ms,
        };
        if let Err(error) = self.write_request(&request) {
            let reason = format!("failed to write to Engine262 worker: {error:#}");
            let message = self.restart_after_failure(&reason);
            return Ok(worker_crash(message));
        }
        let mut line = String::new();
        let read = self
            .stdout
            .read_line(&mut line)
            .context("failed to read from Engine262 worker")?;
        if read == 0 {
            let message = self.restart_after_failure("Engine262 worker exited without a response");
            return Ok(worker_crash(message));
        }
        let response: Engine262Response =
            serde_json::from_str(&line).context("Engine262 worker returned invalid JSON")?;
        Ok(response.into_outcome())
    }

    fn write_request(&mut self, request: &Engine262Request) -> anyhow::Result<()> {
        serde_json::to_writer(&mut self.stdin, request)
            .context("failed to serialize Engine262 worker request")?;
        self.stdin
            .write_all(b"\n")
            .context("failed to terminate Engine262 worker request")?;
        self.stdin
            .flush()
            .context("failed to flush Engine262 worker request")
    }

    fn restart_after_failure(&mut self, reason: &str) -> String {
        let old_stderr = self.current_stderr.clone();
        let status = match self.child.try_wait() {
            Ok(Some(status)) => Some(status.to_string()),
            Ok(None) => {
                if let Err(_error) = self.child.kill() {}
                self.child.wait().ok().map(|value| value.to_string())
            }
            Err(error) => Some(format!("failed to poll Engine262 worker: {error}")),
        };
        self.restart_count = self.restart_count.saturating_add(1);
        let next_stderr = stderr_path(&self.stderr_dir, self.owner_pid, self.restart_count);
        if let Ok((child, stdin, stdout)) =
            spawn_worker(&self.node_binary, &self.package_dir, &next_stderr)
        {
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
struct Engine262Request {
    source_base64: String,
    timeout_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Engine262Response {
    status: String,
    stdout: String,
    error_name: Option<String>,
    error_message: Option<String>,
    elapsed_nanos: String,
}

impl Engine262Response {
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
    package_dir: &Path,
    stderr_path: &Path,
) -> anyhow::Result<(Child, ChildStdin, BufReader<ChildStdout>)> {
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(stderr_path)
        .with_context(|| format!("failed to open '{}'", stderr_path.display()))?;
    let mut child = Command::new(node_binary)
        .arg("--eval")
        .arg(ENGINE262_PARENT_SOURCE)
        .current_dir(package_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("failed to start Engine262 worker '{node_binary}'"))?;
    let stdin = child
        .stdin
        .take()
        .context("Engine262 worker stdin is missing")?;
    let stdout = child
        .stdout
        .take()
        .context("Engine262 worker stdout is missing")?;
    ensure!(
        child
            .try_wait()
            .context("failed to poll Engine262 worker")?
            .is_none(),
        "Engine262 worker exited during startup"
    );
    Ok((child, stdin, BufReader::new(stdout)))
}

fn stderr_path(directory: &Path, owner_pid: u32, restart_count: u64) -> PathBuf {
    directory.join(format!(
        "engine262-worker-{owner_pid}-{restart_count}.stderr.log"
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
