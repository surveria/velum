use std::{env, fs, path::PathBuf, process::Command};

const UNKNOWN_VALUE: &str = "unknown";

fn main() {
    emit_rerun_hints();

    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| UNKNOWN_VALUE.to_owned());
    let commit = build_commit_sha();

    println!("cargo:rustc-env=VELUM_ENGINE_VERSION={version}");
    println!("cargo:rustc-env=VELUM_ENGINE_COMMIT_SHA={commit}");
}

fn emit_rerun_hints() {
    println!("cargo:rerun-if-env-changed=VELUM_BUILD_COMMIT_SHA");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    emit_git_path_hint("HEAD");
    if let Some(reference) = git_stdout(&["symbolic-ref", "-q", "HEAD"]) {
        emit_git_path_hint(&reference);
    }
}

fn emit_git_path_hint(git_path_name: &str) {
    let Some(path) = git_path(git_path_name) else {
        return;
    };
    let path = PathBuf::from(path);
    let path = fs::canonicalize(&path).unwrap_or(path);
    println!("cargo:rerun-if-changed={}", path.display());
}

fn build_commit_sha() -> String {
    env_commit("VELUM_BUILD_COMMIT_SHA")
        .or_else(|| git_stdout(&["rev-parse", "HEAD"]))
        .or_else(|| env_commit("GITHUB_SHA"))
        .unwrap_or_else(|| UNKNOWN_VALUE.to_owned())
}

fn env_commit(name: &str) -> Option<String> {
    let value = env::var(name).ok()?;
    non_empty(value)
}

fn git_stdout(args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    if let Some(repo_root) = git_work_dir() {
        command.current_dir(repo_root);
    }
    let output = command.args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    non_empty(stdout.trim().to_owned())
}

fn git_path(git_path_name: &str) -> Option<String> {
    git_stdout(&[
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        git_path_name,
    ])
    .or_else(|| git_stdout(&["rev-parse", "--git-path", git_path_name]))
}

fn git_work_dir() -> Option<PathBuf> {
    env_path("VELUM_BUILD_REPO_ROOT")
        .or_else(|| env_path("GITHUB_WORKSPACE"))
        .or_else(manifest_repo_root)
}

fn manifest_repo_root() -> Option<PathBuf> {
    let manifest_dir = env_path("CARGO_MANIFEST_DIR")?;
    let file_name = manifest_dir.file_name().and_then(|name| name.to_str());
    if file_name == Some("runner")
        && let Some(parent) = manifest_dir.parent()
    {
        return Some(parent.to_path_buf());
    }
    Some(manifest_dir)
}

fn env_path(name: &str) -> Option<PathBuf> {
    let value = env::var(name).ok()?;
    non_empty(value).map(PathBuf::from)
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}
