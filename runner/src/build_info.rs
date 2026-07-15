#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RunnerBuildInfo {
    pub package_name: &'static str,
    pub version: &'static str,
    pub commit_sha: &'static str,
}

#[must_use]
pub const fn runner_build_info() -> RunnerBuildInfo {
    RunnerBuildInfo {
        package_name: env!("CARGO_PKG_NAME"),
        version: env!("VELUM_RUNNER_VERSION"),
        commit_sha: env!("VELUM_RUNNER_COMMIT_SHA"),
    }
}
