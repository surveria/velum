use std::{
    fs::{File, OpenOptions, TryLockError},
    path::Path,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, bail};

const LOCK_NAME: &str = ".case-writers.lock";
const POLL_INTERVAL: Duration = Duration::from_millis(25);
pub const REPORT_DRAIN_TIMEOUT: Duration = Duration::from_mins(2);

/// An OS-managed session lock, released even if its owning process crashes.
pub struct SessionLock {
    _file: File,
}

impl SessionLock {
    /// Holds a shared lock for the complete artifact-recorder lifetime.
    ///
    /// # Errors
    /// Returns an error if the lock file cannot be opened or locked.
    pub fn writer(session: &Path) -> anyhow::Result<Self> {
        let file = open_lock(session)?;
        file.lock_shared()
            .with_context(|| format!("failed to lock case writer in '{}'", session.display()))?;
        Ok(Self { _file: file })
    }

    /// Waits for case writers to exit and prevents writes during report creation.
    ///
    /// # Errors
    /// Returns an error on I/O failure or if case writers outlive the drain timeout.
    pub fn report(session: &Path, timeout: Duration) -> anyhow::Result<Self> {
        let file = open_lock(session)?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(TryLockError::WouldBlock) => {
                    let remaining = timeout.saturating_sub(started.elapsed());
                    if remaining.is_zero() {
                        bail!(
                            "case writers are still active in '{}' after {}; refusing an incomplete report",
                            session.display(),
                            humantime::format_duration(timeout)
                        );
                    }
                    thread::sleep(POLL_INTERVAL.min(remaining));
                }
                Err(TryLockError::Error(error)) => {
                    return Err(error).with_context(|| {
                        format!("failed to lock report in '{}'", session.display())
                    });
                }
            }
        }
    }
}

fn open_lock(session: &Path) -> anyhow::Result<File> {
    let path = session.join(LOCK_NAME);
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .with_context(|| format!("failed to open session lock '{}'", path.display()))
}
