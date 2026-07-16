use std::{
    error::Error,
    fs,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

type TestResult = Result<(), Box<dyn Error>>;

fn cache_script() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fuzzing_dir = manifest_dir
        .parent()
        .ok_or("driver manifest has no parent directory")?;
    Ok(fuzzing_dir.join("scripts/fuzzilli-cache.sh"))
}

fn temporary_directory() -> Result<PathBuf, Box<dyn Error>> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "velum-fuzzilli-cache-test-{}-{timestamp}",
        std::process::id()
    )))
}

fn run_cache(cache_root: &Path, arguments: &[&Path]) -> Result<Output, Box<dyn Error>> {
    let mut command = Command::new(cache_script()?);
    command.env("VELUM_FUZZILLI_CACHE_DIR", cache_root);
    for argument in arguments {
        command.arg(argument);
    }
    Ok(command.output()?)
}

#[test]
fn stores_and_restores_machine_cached_fuzzilli() -> TestResult {
    let directory = temporary_directory()?;
    let cache_root = directory.join("cache");
    let source = directory.join("source-FuzzilliCli");
    let first_link = directory.join("first/FuzzilliCli");
    let second_link = directory.join("second/FuzzilliCli");
    fs::create_dir_all(&directory)?;
    fs::write(&source, b"cached fuzzilli fixture")?;
    let mut permissions = fs::metadata(&source)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&source, permissions)?;

    let miss = run_cache(&cache_root, &[Path::new("restore"), second_link.as_path()])?;
    if miss.status.success() {
        return Err("an empty cache unexpectedly restored a binary".into());
    }

    let stored = run_cache(
        &cache_root,
        &[Path::new("store"), source.as_path(), first_link.as_path()],
    )?;
    if !stored.status.success() {
        return Err(format!(
            "cache store failed: {}",
            String::from_utf8_lossy(&stored.stderr)
        )
        .into());
    }

    let restored = run_cache(&cache_root, &[Path::new("restore"), second_link.as_path()])?;
    if !restored.status.success() {
        return Err(format!(
            "cache restore failed: {}",
            String::from_utf8_lossy(&restored.stderr)
        )
        .into());
    }

    let first_contents = fs::read(&first_link)?;
    let second_contents = fs::read(&second_link)?;
    if first_contents != b"cached fuzzilli fixture" || second_contents != b"cached fuzzilli fixture"
    {
        return Err("restored Fuzzilli links do not use the cached binary".into());
    }
    if fs::canonicalize(&first_link)? != fs::canonicalize(&second_link)? {
        return Err("cache restore linked different binaries".into());
    }

    fs::remove_dir_all(directory)?;
    Ok(())
}
