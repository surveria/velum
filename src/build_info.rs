#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct BuildInfo {
    pub package_name: &'static str,
    pub version: &'static str,
    pub commit_sha: &'static str,
}

#[must_use]
pub const fn engine_build_info() -> BuildInfo {
    BuildInfo {
        package_name: env!("CARGO_PKG_NAME"),
        version: env!("VELUM_ENGINE_VERSION"),
        commit_sha: env!("VELUM_ENGINE_COMMIT_SHA"),
    }
}
