use std::{
    io::{self, BufRead as _, Read as _, Write as _},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, bail};

use super::{
    config,
    engines::{ALLOCATE, RELEASE, Sessions, VERIFY},
    model::{
        ACK, Diagnostics, MAX_PROTOCOL_LINE, PROTOCOL_VERSION, Phase, WorkerConfig, WorkerEvent,
        phases,
    },
};

pub fn run(encoded_config: &str) -> anyhow::Result<()> {
    if encoded_config.len() > MAX_PROTOCOL_LINE {
        bail!("memory worker configuration exceeds the protocol limit");
    }
    let config: WorkerConfig =
        serde_json::from_str(encoded_config).context("invalid memory worker configuration")?;
    config::validate_worker(&config)?;
    let (finished, receiver) = mpsc::sync_channel::<()>(1);
    let timeout = Duration::from_millis(config.timeout_ms);
    let watchdog = thread::Builder::new()
        .name("memory-worker-deadline".to_owned())
        .spawn(move || {
            if receiver.recv_timeout(timeout) == Err(mpsc::RecvTimeoutError::Timeout) {
                eprintln!("memory worker exceeded its independent wall deadline");
                std::process::exit(124);
            }
        })
        .context("failed to start memory worker watchdog")?;
    let result = execute(&config);
    finished
        .send(())
        .context("memory worker watchdog disconnected unexpectedly")?;
    watchdog
        .join()
        .map_err(|_| anyhow::anyhow!("memory worker watchdog panicked"))?;
    result
}

fn execute(config: &WorkerConfig) -> anyhow::Result<()> {
    emit(&WorkerEvent::Ready {
        protocol_version: PROTOCOL_VERSION,
        engine: config.engine,
        scenario_id: config.scenario.id.clone(),
        pid: std::process::id(),
    })?;
    let mut sessions = None;
    let expected = config::expected_checksum(&config.scenario)?;
    let plan = phases(&config.scenario);
    for (index, phase) in plan.iter().copied().enumerate() {
        let mut details = apply_phase(phase, config, &mut sessions, expected)?;
        emit(&WorkerEvent::Phase { index, phase })?;
        wait_for_sample()?;
        if let Some(sessions) = &sessions {
            details.vms = sessions.counters()?;
        }
        emit(&WorkerEvent::Diagnostics { index, details })?;
    }
    emit(&WorkerEvent::Finished {
        phase_count: plan.len(),
    })
}

fn apply_phase(
    phase: Phase,
    config: &WorkerConfig,
    sessions: &mut Option<Sessions>,
    expected: u64,
) -> anyhow::Result<Diagnostics> {
    let mut details = Diagnostics::default();
    match phase {
        Phase::ProcessBaseline => {}
        Phase::EmptyVms => *sessions = Some(Sessions::create(config.engine, &config.scenario)?),
        Phase::VmsReady => sessions
            .as_mut()
            .context("memory sessions missing for preparation")?
            .prepare(&config.scenario)?,
        Phase::Live { .. } => {
            let sessions = sessions
                .as_mut()
                .context("memory sessions missing for allocation")?;
            let checksum = sessions.eval_verified(ALLOCATE, expected)?;
            let verified = sessions.eval_verified(VERIFY, expected)?;
            if checksum != verified {
                bail!("memory phase result changed during checksum verification");
            }
            details.checksum = Some(checksum);
        }
        Phase::RootsReleased { .. } => {
            sessions
                .as_mut()
                .context("memory sessions missing for release")?
                .eval_verified(RELEASE, 1)?;
        }
        Phase::AfterGc { .. } => {
            let started = Instant::now();
            details.velum_reclaimed_records = sessions
                .as_mut()
                .context("memory sessions missing for collection")?
                .collect()?;
            details.forced_gc_duration_ns = Some(u64::try_from(started.elapsed().as_nanos())?);
        }
        Phase::VmsDropped => sessions
            .as_mut()
            .context("memory sessions missing for teardown")?
            .drop_vms(),
        Phase::OwnersDropped => sessions
            .as_mut()
            .context("memory sessions missing for owner teardown")?
            .drop_owners(),
    }
    Ok(details)
}

fn emit(event: &WorkerEvent) -> anyhow::Result<()> {
    let mut output = io::stdout().lock();
    serde_json::to_writer(&mut output, event).context("failed to encode memory worker event")?;
    output
        .write_all(b"\n")
        .context("failed to delimit memory worker event")?;
    output
        .flush()
        .context("failed to flush memory worker event")
}

fn wait_for_sample() -> anyhow::Result<()> {
    let mut input = io::stdin().lock();
    let mut acknowledgement = Vec::new();
    let count = input
        .by_ref()
        .take(32)
        .read_until(b'\n', &mut acknowledgement)
        .context("failed to read memory sampling acknowledgement")?;
    if count == 0 || acknowledgement.as_slice() != ACK.as_bytes() {
        bail!("invalid or missing memory sampling acknowledgement");
    }
    Ok(())
}
