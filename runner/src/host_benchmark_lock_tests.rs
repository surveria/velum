use std::{fs, process};

use super::{owner_path, with_exclusive_at};

type TestResult = anyhow::Result<()>;

#[test]
fn removes_owner_metadata_after_measured_operation() -> TestResult {
    let root = std::env::temp_dir().join(format!("rsqjs-benchmark-lock-test-{}", process::id()));
    fs::create_dir_all(&root)?;
    let lock_path = root.join("host-performance.lock");
    let result = with_exclusive_at(&lock_path, "unit benchmark", || Ok(42_u8))?;
    let metadata_path = owner_path(&lock_path)?;
    if result != 42 || metadata_path.exists() {
        anyhow::bail!("benchmark lock did not return its value or remove owner metadata");
    }
    if lock_path.exists() {
        fs::remove_file(&lock_path)?;
    }
    fs::remove_dir(&root)?;
    Ok(())
}
