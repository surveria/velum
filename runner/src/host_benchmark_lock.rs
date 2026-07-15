use std::{
    env, fs,
    fs::{File, OpenOptions},
    io::Write as _,
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, anyhow, bail};
use fs2::FileExt as _;

const HOST_LOCK_PATH_ENV: &str = "VELUM_HOST_LOCK_PATH";
const DEFAULT_HOST_LOCK_PATH: &str = "/run/lock/velum/host-performance.lock";

pub fn with_exclusive<T>(
    label: &str,
    operation: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let lock_path = env::var_os(HOST_LOCK_PATH_ENV)
        .map_or_else(|| PathBuf::from(DEFAULT_HOST_LOCK_PATH), PathBuf::from);
    with_exclusive_at(&lock_path, label, operation)
}

fn with_exclusive_at<T>(
    lock_path: &Path,
    label: &str,
    operation: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let lock = BenchmarkLock::acquire(lock_path, label)?;
    let operation_result = operation();
    let release_result = lock.release();
    match (operation_result, release_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(operation_error), Err(release_error)) => Err(anyhow!(
            "{operation_error:#}; additionally failed to release benchmark lock: {release_error:#}"
        )),
    }
}

struct BenchmarkLock {
    file: File,
    lock_path: PathBuf,
    owner_path: PathBuf,
}

impl BenchmarkLock {
    fn acquire(lock_path: &Path, label: &str) -> anyhow::Result<Self> {
        let parent = lock_path
            .parent()
            .context("benchmark lock path has no parent")?;
        reject_symlink(parent, "benchmark lock directory")?;
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create benchmark lock directory {}",
                parent.display()
            )
        })?;
        reject_symlink(lock_path, "benchmark lock file")?;
        let owner_path = owner_path(lock_path)?;
        reject_symlink(&owner_path, "benchmark owner metadata")?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o666)
            .open(lock_path)
            .with_context(|| format!("failed to open benchmark lock {}", lock_path.display()))?;
        if let Err(error) = fs::set_permissions(lock_path, fs::Permissions::from_mode(0o666)) {
            eprintln!(
                "benchmark lock: could not widen permissions for existing writable lock {}: {error}",
                lock_path.display()
            );
        }
        println!(
            "benchmark lock: waiting for exclusive measured execution slot on {}",
            lock_path.display()
        );
        file.lock_exclusive()
            .with_context(|| format!("failed to lock benchmark slot at {}", lock_path.display()))?;
        if let Err(error) = write_owner(&owner_path, label) {
            if owner_path.exists() {
                fs::remove_file(&owner_path).with_context(|| {
                    format!(
                        "failed to remove incomplete benchmark owner metadata {}",
                        owner_path.display()
                    )
                })?;
            }
            fs2::FileExt::unlock(&file).with_context(|| {
                format!(
                    "failed to release benchmark lock {} after metadata error",
                    lock_path.display()
                )
            })?;
            return Err(error);
        }
        println!(
            "benchmark lock: acquired exclusive slot for {label}; owner metadata: {}",
            owner_path.display()
        );
        Ok(Self {
            file,
            lock_path: lock_path.to_path_buf(),
            owner_path,
        })
    }

    fn release(self) -> anyhow::Result<()> {
        if self.owner_path.exists() {
            fs::remove_file(&self.owner_path).with_context(|| {
                format!(
                    "failed to remove benchmark owner metadata {}",
                    self.owner_path.display()
                )
            })?;
        }
        fs2::FileExt::unlock(&self.file).with_context(|| {
            format!(
                "failed to release benchmark lock {}",
                self.lock_path.display()
            )
        })?;
        println!("benchmark lock: released exclusive measured execution slot");
        Ok(())
    }
}

fn owner_path(lock_path: &Path) -> anyhow::Result<PathBuf> {
    let file_name = lock_path
        .file_name()
        .and_then(|value| value.to_str())
        .context("benchmark lock filename is not valid UTF-8")?;
    Ok(lock_path.with_file_name(format!("{file_name}.owner")))
}

fn reject_symlink(path: &Path, label: &str) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("{label} must not be a symlink: {}", path.display())
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn write_owner(path: &Path, label: &str) -> anyhow::Result<()> {
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    let cwd = env::current_dir().context("failed to resolve benchmark working directory")?;
    let uid = env::var("UID").unwrap_or_else(|_| String::from("unknown"));
    let host = env::var("HOSTNAME").unwrap_or_else(|_| String::from("unknown"));
    let mut file = File::create(path).with_context(|| {
        format!(
            "failed to create benchmark owner metadata {}",
            path.display()
        )
    })?;
    writeln!(file, "pid={}", process::id())?;
    writeln!(file, "uid={uid}")?;
    writeln!(file, "host={host}")?;
    writeln!(file, "started_unix={started}")?;
    writeln!(file, "cwd={}", cwd.display())?;
    writeln!(file, "benchmark={label}")?;
    Ok(())
}

#[cfg(test)]
#[path = "host_benchmark_lock_tests.rs"]
mod tests;
