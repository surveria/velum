use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};

use crate::cases;

const TEST262_ACTIVE_DIR: &str = "tests/corpora/test262/active";
const QUICKJS_ACTIVE_DIR: &str = "tests/corpora/quickjs_differential/active";

pub fn validate() -> anyhow::Result<()> {
    let test262 = cases::test262_cases()
        .into_iter()
        .map(|case| (case.id, case.path))
        .collect::<Vec<_>>();
    validate_registry("Test262 active", TEST262_ACTIVE_DIR, &test262)?;

    let quickjs = cases::quickjs_differential_cases()
        .into_iter()
        .map(|case| (case.id, case.path))
        .collect::<Vec<_>>();
    validate_registry("QuickJS differential", QUICKJS_ACTIVE_DIR, &quickjs)
}

fn validate_registry(
    name: &str,
    directory: &str,
    registered: &[(&str, &str)],
) -> anyhow::Result<()> {
    let mut ids = BTreeSet::new();
    let mut paths = BTreeMap::<String, String>::new();
    for (id, path) in registered {
        if !ids.insert((*id).to_owned()) {
            bail!("{name} registry contains duplicate id '{id}'");
        }
        if let Some(previous) = paths.insert((*path).to_owned(), (*id).to_owned()) {
            bail!("{name} registry contains duplicate path '{path}' for '{previous}' and '{id}'");
        }
    }

    let discovered = discover_js_files(directory)?;
    let registered_paths = paths.into_keys().collect::<BTreeSet<_>>();
    if registered_paths == discovered {
        return Ok(());
    }
    let missing = discovered
        .difference(&registered_paths)
        .cloned()
        .collect::<Vec<_>>();
    let stale = registered_paths
        .difference(&discovered)
        .cloned()
        .collect::<Vec<_>>();
    bail!(
        "{name} registry does not match checked-in fixtures; missing registrations: {missing:?}; stale registrations: {stale:?}"
    )
}

fn discover_js_files(directory: &str) -> anyhow::Result<BTreeSet<String>> {
    let repo_root = repo_root()?;
    let root = repo_root.join(directory);
    let mut paths = BTreeSet::new();
    collect_js_files(&repo_root, &root, &mut paths)?;
    Ok(paths)
}

fn collect_js_files(
    repo_root: &Path,
    directory: &Path,
    paths: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("failed to read fixture directory '{}'", directory.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_js_files(repo_root, &path, paths)?;
        } else if path.extension().is_some_and(|extension| extension == "js") {
            let relative = path.strip_prefix(repo_root).with_context(|| {
                format!(
                    "fixture '{}' is outside repository root '{}'",
                    path.display(),
                    repo_root.display()
                )
            })?;
            paths.insert(relative.to_string_lossy().into_owned());
        }
    }
    Ok(())
}

fn repo_root() -> anyhow::Result<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .context("runner manifest directory has no repository parent")
}
