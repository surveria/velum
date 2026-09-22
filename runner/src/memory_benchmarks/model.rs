use serde::{Deserialize, Serialize};

use super::proc_metrics::ProcessMemory;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_PROTOCOL_LINE: usize = 1_048_576;
pub const MAX_PROTOCOL_EVENTS: usize = 64;
pub const ACK: &str = "continue\n";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineKind {
    Velum,
    Quickjs,
}

impl EngineKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Velum => "velum",
            Self::Quickjs => "quickjs",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioKind {
    HelloWorld,
    RetainedGraph,
    CyclicChurn,
    IndependentVms,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    pub kind: ScenarioKind,
    pub vm_count: usize,
    pub nodes_per_vm: usize,
    pub bytes_per_node: usize,
    pub rounds: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerConfig {
    pub protocol_version: u32,
    pub engine: EngineKind,
    pub scenario: Scenario,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case", deny_unknown_fields)]
pub enum Phase {
    ProcessBaseline,
    EmptyVms,
    VmsReady,
    Live { round: usize },
    RootsReleased { round: usize },
    AfterGc { round: usize },
    VmsDropped,
    OwnersDropped,
}

pub fn phases(scenario: &Scenario) -> Vec<Phase> {
    let mut result = vec![Phase::ProcessBaseline, Phase::EmptyVms, Phase::VmsReady];
    for round in 0..scenario.rounds {
        result.extend([
            Phase::Live { round },
            Phase::RootsReleased { round },
            Phase::AfterGc { round },
        ]);
    }
    result.extend([Phase::VmsDropped, Phase::OwnersDropped]);
    result
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageCategory {
    pub category: String,
    pub logical_records: usize,
    pub logical_payload_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case", deny_unknown_fields)]
pub enum VmCounters {
    Velum {
        vm_index: usize,
        logical_records: usize,
        logical_payload_bytes: usize,
        runtime_steps: usize,
        categories: Vec<StorageCategory>,
    },
    Quickjs {
        vm_index: usize,
        allocator_bytes: i64,
        memory_used_bytes: i64,
        allocator_blocks: i64,
        memory_used_blocks: i64,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Diagnostics {
    pub vms: Vec<VmCounters>,
    pub checksum: Option<u64>,
    pub forced_gc_duration_ns: Option<u64>,
    pub velum_reclaimed_records: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerEvent {
    Ready {
        protocol_version: u32,
        engine: EngineKind,
        scenario_id: String,
        pid: u32,
    },
    Phase {
        index: usize,
        phase: Phase,
    },
    Diagnostics {
        index: usize,
        details: Diagnostics,
    },
    Finished {
        phase_count: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseMeasurement {
    pub phase: Phase,
    pub process: ProcessMemory,
    pub diagnostics: Diagnostics,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMeasurement {
    pub scenario_id: String,
    pub engine: EngineKind,
    pub repetition: usize,
    pub outcome: Outcome,
    pub detail: String,
    pub elapsed_ns: u64,
    pub phases: Vec<PhaseMeasurement>,
}
