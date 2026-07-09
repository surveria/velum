use crate::benchmark_protocol::{BenchmarkInput, BenchmarkMode};

#[derive(Debug, Clone, Copy)]
pub struct BenchmarkCase {
    pub id: &'static str,
    pub path: &'static str,
    pub mode: BenchmarkMode,
    pub input: BenchmarkInput,
    pub sentinel: bool,
}

impl BenchmarkCase {
    pub const fn cold(id: &'static str, path: &'static str) -> Self {
        Self {
            id,
            path,
            mode: BenchmarkMode::ColdEval,
            input: BenchmarkInput::Standard,
            sentinel: false,
        }
    }

    pub const fn cold_host_image(id: &'static str, path: &'static str, byte_len: usize) -> Self {
        Self {
            id,
            path,
            mode: BenchmarkMode::ColdEval,
            input: BenchmarkInput::HostImage { byte_len },
            sentinel: false,
        }
    }

    pub const fn prepared_sentinel(id: &'static str, path: &'static str) -> Self {
        Self {
            id,
            path,
            mode: BenchmarkMode::PreparedExecution,
            input: BenchmarkInput::Standard,
            sentinel: true,
        }
    }
}
