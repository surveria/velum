use std::{
    io::{BufRead as _, BufReader, Read as _, Write as _},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context as _, bail};

use super::{
    model::{
        ACK, MAX_PROTOCOL_EVENTS, MAX_PROTOCOL_LINE, Outcome, PhaseMeasurement, RunMeasurement,
        WorkerConfig, WorkerEvent, phases,
    },
    proc_metrics, protocol,
};

const MAX_STDERR_BYTES: usize = 16_384;
const CHILD_FLAG: &str = "--memory-benchmark-worker";

enum Output {
    Event(WorkerEvent),
    End,
    Error(String),
}

struct ManagedChild {
    child: Child,
}

impl ManagedChild {
    fn stop(&mut self) -> anyhow::Result<()> {
        if self
            .child
            .try_wait()
            .context("failed to inspect memory worker")?
            .is_none()
        {
            self.child.kill().context("failed to kill memory worker")?;
        }
        self.child.wait().context("failed to reap memory worker")?;
        Ok(())
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("memory worker cleanup failed: {error:#}");
        }
    }
}

pub fn run(executable: &Path, config: &WorkerConfig, repetition: usize) -> RunMeasurement {
    let started = Instant::now();
    let mut measurements = Vec::new();
    let result = execute(executable, config, &mut measurements);
    let elapsed_ns = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let (outcome, detail) = match result {
        Ok(()) => (
            Outcome::Passed,
            "all phases, checksums and teardown boundaries verified".to_owned(),
        ),
        Err(error) => (Outcome::Failed, format!("{error:#}")),
    };
    RunMeasurement {
        scenario_id: config.scenario.id.clone(),
        engine: config.engine,
        repetition,
        outcome,
        detail,
        elapsed_ns,
        phases: measurements,
    }
}

fn execute(
    executable: &Path,
    config: &WorkerConfig,
    measurements: &mut Vec<PhaseMeasurement>,
) -> anyhow::Result<()> {
    let started = Instant::now();
    let timeout = Duration::from_millis(config.timeout_ms);
    let encoded =
        serde_json::to_string(config).context("failed to encode memory worker configuration")?;
    let child = Command::new(executable)
        .args([CHILD_FLAG, &encoded])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start memory worker {}", executable.display()))?;
    let mut child = ManagedChild { child };
    let stdout = child
        .child
        .stdout
        .take()
        .context("memory worker stdout missing")?;
    let stderr = child
        .child
        .stderr
        .take()
        .context("memory worker stderr missing")?;
    let (sender, receiver) = mpsc::sync_channel(2);
    let output_thread = thread::Builder::new()
        .name("memory-worker-output".to_owned())
        .spawn(move || read_events(stdout, &sender))
        .context("failed to start memory stdout reader")?;
    let error_thread = thread::Builder::new()
        .name("memory-worker-errors".to_owned())
        .spawn(move || read_errors(stderr))
        .context("failed to start memory stderr reader")?;
    let observed = observe(
        &mut child,
        &receiver,
        config,
        started,
        timeout,
        measurements,
    );
    let cleanup = child.stop();
    // Dropping the receiver releases a reader blocked on its bounded channel.
    drop(receiver);
    let output_result = join_reader(output_thread, "stdout");
    let errors_result = join_reader(error_thread, "stderr");
    let errors = errors_result.unwrap_or_else(|error| format!("stderr reader failed: {error:#}"));
    match (observed, cleanup, output_result) {
        (Ok(()), Ok(()), Ok(())) if errors.is_empty() => Ok(()),
        (Ok(()), Ok(()), Ok(())) => bail!("memory worker unexpectedly wrote stderr: {errors}"),
        (observed, cleanup, output) => bail!(
            "memory worker failed: observation={}; cleanup={}; output={}; stderr={errors}",
            describe(observed),
            describe(cleanup),
            describe(output),
        ),
    }
}

fn describe(result: anyhow::Result<()>) -> String {
    match result {
        Ok(()) => "ok".to_owned(),
        Err(error) => format!("{error:#}"),
    }
}

fn observe(
    child: &mut ManagedChild,
    receiver: &Receiver<Output>,
    config: &WorkerConfig,
    started: Instant,
    timeout: Duration,
    measurements: &mut Vec<PhaseMeasurement>,
) -> anyhow::Result<()> {
    protocol::ready(event(receiver, started, timeout)?, config, child.child.id())?;
    let plan = phases(&config.scenario);
    for (index, phase) in plan.iter().copied().enumerate() {
        protocol::phase(event(receiver, started, timeout)?, index, phase)?;
        let process =
            proc_metrics::sample(child.child.id()).context("memory process snapshot failed")?;
        let input = child
            .child
            .stdin
            .as_mut()
            .context("memory worker stdin missing")?;
        input
            .write_all(ACK.as_bytes())
            .context("memory worker acknowledgement failed")?;
        input
            .flush()
            .context("memory worker acknowledgement flush failed")?;
        let diagnostics =
            protocol::diagnostics(event(receiver, started, timeout)?, index, phase, config)?;
        measurements.push(PhaseMeasurement {
            phase,
            process,
            diagnostics,
        });
    }
    protocol::finished(event(receiver, started, timeout)?, plan.len())?;
    match receive(receiver, started, timeout)? {
        Output::End => {}
        Output::Error(error) => bail!("memory output failed after completion: {error}"),
        Output::Event(event) => bail!("unexpected memory event after completion: {event:?}"),
    }
    loop {
        if let Some(status) = child
            .child
            .try_wait()
            .context("memory worker status read failed")?
        {
            if !status.success() {
                bail!("memory worker exited with {status}");
            }
            return Ok(());
        }
        remaining(started, timeout)?;
        thread::sleep(Duration::from_millis(1));
    }
}

fn event(
    receiver: &Receiver<Output>,
    started: Instant,
    timeout: Duration,
) -> anyhow::Result<WorkerEvent> {
    match receive(receiver, started, timeout)? {
        Output::Event(event) => Ok(event),
        Output::End => bail!("memory worker ended before all required phases completed"),
        Output::Error(error) => bail!("invalid memory worker output: {error}"),
    }
}

fn receive(
    receiver: &Receiver<Output>,
    started: Instant,
    timeout: Duration,
) -> anyhow::Result<Output> {
    match receiver.recv_timeout(remaining(started, timeout)?) {
        Ok(output) => Ok(output),
        Err(RecvTimeoutError::Timeout) => bail!(
            "memory worker exceeded its {} ms wall deadline",
            timeout.as_millis()
        ),
        Err(RecvTimeoutError::Disconnected) => bail!("memory worker output channel disconnected"),
    }
}

fn remaining(started: Instant, timeout: Duration) -> anyhow::Result<Duration> {
    timeout
        .checked_sub(started.elapsed())
        .context("memory worker wall deadline expired")
}

fn read_events(reader: impl std::io::Read, sender: &SyncSender<Output>) -> anyhow::Result<()> {
    let result = read_event_stream(reader, sender);
    if let Err(error) = &result
        && sender.send(Output::Error(format!("{error:#}"))).is_err()
    {
        return result;
    }
    result
}

fn read_event_stream(
    reader: impl std::io::Read,
    sender: &SyncSender<Output>,
) -> anyhow::Result<()> {
    let mut reader = BufReader::new(reader);
    for _event_index in 0..MAX_PROTOCOL_EVENTS {
        let mut bytes = Vec::new();
        let limit = u64::try_from(MAX_PROTOCOL_LINE)?
            .checked_add(1)
            .context("memory line bound overflowed")?;
        let count = reader
            .by_ref()
            .take(limit)
            .read_until(b'\n', &mut bytes)
            .context("failed to read memory event")?;
        if count == 0 {
            sender
                .send(Output::End)
                .context("memory output receiver disappeared")?;
            return Ok(());
        }
        if count > MAX_PROTOCOL_LINE || bytes.last() != Some(&b'\n') {
            bail!("memory event exceeds the line bound or lacks its delimiter");
        }
        let event =
            serde_json::from_slice(&bytes).context("memory event is not valid typed JSON")?;
        sender
            .send(Output::Event(event))
            .context("memory output receiver disappeared")?;
    }
    bail!("memory worker emitted too many protocol events")
}

fn read_errors(mut reader: impl std::io::Read) -> anyhow::Result<String> {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 4_096];
    loop {
        let count = reader
            .read(&mut buffer)
            .context("failed to read memory worker stderr")?;
        if count == 0 {
            break;
        }
        let available = MAX_STDERR_BYTES.saturating_sub(retained.len());
        if let Some(bytes) = buffer.get(..count.min(available)) {
            retained.extend_from_slice(bytes);
        }
    }
    Ok(String::from_utf8_lossy(&retained).into_owned())
}

fn join_reader<T>(handle: JoinHandle<anyhow::Result<T>>, label: &str) -> anyhow::Result<T> {
    handle
        .join()
        .map_err(|_| anyhow::anyhow!("memory {label} reader panicked"))?
}
