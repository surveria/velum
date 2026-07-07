use rs_quickjs::engine_build_info;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_engine_build_info() -> TestResult {
    let info = engine_build_info();

    ensure_text(info.package_name, "rs-quickjs")?;
    ensure_text(info.version, env!("CARGO_PKG_VERSION"))?;
    ensure_non_empty(info.commit_sha, "commit sha")
}

fn ensure_text(actual: &str, expected: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected '{expected}', got '{actual}'").into())
}

fn ensure_non_empty(actual: &str, label: &str) -> TestResult {
    if !actual.is_empty() {
        return Ok(());
    }
    Err(format!("expected non-empty {label}").into())
}
