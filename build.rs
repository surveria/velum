use std::{env, fs, path::PathBuf, process::Command};

const UNKNOWN_COMMIT: &str = "unknown";

fn main() {
    emit_rerun_hints();

    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| UNKNOWN_COMMIT.to_owned());
    let commit = build_commit_sha();

    println!("cargo:rustc-env=RSQJS_ENGINE_VERSION={version}");
    println!("cargo:rustc-env=RSQJS_ENGINE_COMMIT_SHA={commit}");
}

fn emit_rerun_hints() {
    println!("cargo:rerun-if-env-changed=RSQJS_BUILD_COMMIT_SHA");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    emit_git_path_hint("HEAD");
    if let Some(reference) = git_stdout(&["symbolic-ref", "-q", "HEAD"]) {
        emit_git_path_hint(&reference);
    }
}

fn emit_git_path_hint(git_path: &str) {
    let Some(path) = git_stdout(&["rev-parse", "--git-path", git_path]) else {
        return;
    };
    let path = PathBuf::from(path);
    let path = fs::canonicalize(&path).unwrap_or(path);
    println!("cargo:rerun-if-changed={}", path.display());
}

fn build_commit_sha() -> String {
    env_commit("RSQJS_BUILD_COMMIT_SHA")
        .or_else(|| git_stdout(&["rev-parse", "HEAD"]))
        .or_else(|| env_commit("GITHUB_SHA"))
        .unwrap_or_else(|| UNKNOWN_COMMIT.to_owned())
}

fn env_commit(name: &str) -> Option<String> {
    let value = env::var(name).ok()?;
    non_empty(value)
}

fn git_stdout(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    non_empty(stdout.trim().to_owned())
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}
