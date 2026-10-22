use std::{
    fs,
    path::PathBuf,
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, ensure};
use velum_differential_fuzz::{report::build_report, session_lock::SessionLock};

#[test]
fn reporting_requires_all_writers_to_release_their_locks() -> anyhow::Result<()> {
    let session = session_directory("all-writers")?;
    let first = SessionLock::writer(&session)?;
    let second = SessionLock::writer(&session)?;
    let error = SessionLock::report(&session, Duration::ZERO)
        .err()
        .context("report lock unexpectedly ignored active writers")?;
    ensure!(error.to_string().contains("refusing an incomplete report"));
    drop(first);
    ensure!(SessionLock::report(&session, Duration::ZERO).is_err());
    drop(second);
    let report = SessionLock::report(&session, Duration::ZERO)?;
    ensure!(SessionLock::report(&session, Duration::ZERO).is_err());
    drop(report);
    drop(SessionLock::writer(&session)?);
    fs::remove_dir_all(session)?;
    Ok(())
}

#[test]
fn report_reads_case_files_only_after_the_writer_finishes() -> anyhow::Result<()> {
    let session = session_directory("late-case")?;
    let writer = SessionLock::writer(&session)?;
    let report_session = session.clone();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let reporter = thread::spawn(move || -> anyhow::Result<()> {
        started_tx.send(())?;
        let result = build_report(&report_session, Duration::from_secs(1), "completed");
        done_tx.send(result.map(|report| report.render()))?;
        Ok(())
    });
    started_rx.recv_timeout(Duration::from_secs(5))?;
    let premature = done_rx.recv_timeout(Duration::from_millis(100));
    let cases = session.join("cases");
    fs::create_dir_all(&cases)?;
    // A late corrupt record must fail reporting, not be silently omitted.
    fs::write(cases.join("late.jsonl"), "{incomplete record}\n")?;
    drop(writer);
    let result = match premature {
        Err(RecvTimeoutError::Timeout) => done_rx.recv_timeout(Duration::from_secs(5))?,
        other => {
            reporter
                .join()
                .map_err(|_| anyhow::anyhow!("report thread failed"))??;
            anyhow::bail!("report did not wait for the live writer: {other:?}");
        }
    };
    reporter
        .join()
        .map_err(|_| anyhow::anyhow!("report thread failed"))??;
    let error = result
        .err()
        .context("report missed the late corrupt record")?;
    ensure!(format!("{error:#}").contains("late.jsonl"));
    fs::remove_dir_all(session)?;
    Ok(())
}

fn session_directory(label: &str) -> anyhow::Result<PathBuf> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!(
        "velum-session-lock-{label}-{}-{timestamp}",
        std::process::id()
    ));
    fs::create_dir(&path)?;
    Ok(path)
}
