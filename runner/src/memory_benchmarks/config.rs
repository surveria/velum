use std::env;

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};

use super::model::{PROTOCOL_VERSION, Scenario, ScenarioKind, WorkerConfig};

pub const MAX_VMS: usize = 50;
pub const MAX_NODES: usize = 4_096;
pub const MAX_BYTES_PER_NODE: usize = 1_024;
pub const MAX_ROUNDS: usize = 5;
pub const MAX_REPETITIONS: usize = 10;
pub const HELLO_CHECKSUM: u64 = 42;
pub const BYTE_MODULUS: usize = 251;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub repetitions: usize,
    pub child_timeout_ms: u64,
    pub scenarios: Vec<Scenario>,
    pub quickjs_compiled: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let repetitions = env_usize("VELUM_MEMORY_REPETITIONS", 3, 1, MAX_REPETITIONS)?;
        let timeout = env_usize("VELUM_MEMORY_CHILD_TIMEOUT_MS", 120_000, 1, 3_600_000)?;
        let nodes = env_usize("VELUM_MEMORY_NODES", 1_024, 1, MAX_NODES)?;
        let bytes = env_usize("VELUM_MEMORY_BYTES_PER_NODE", 256, 1, MAX_BYTES_PER_NODE)?;
        let rounds = env_usize("VELUM_MEMORY_CHURN_ROUNDS", 3, 1, MAX_ROUNDS)?;
        let mut scenarios = vec![
            scenario("hello-world", ScenarioKind::HelloWorld, 1, 0, 0, 1),
            scenario(
                "retained-graph",
                ScenarioKind::RetainedGraph,
                1,
                nodes,
                bytes,
                1,
            ),
            scenario(
                "cyclic-churn",
                ScenarioKind::CyclicChurn,
                1,
                nodes,
                bytes,
                rounds,
            ),
        ];
        for count in [1, 10, 50] {
            scenarios.push(scenario(
                &format!("independent-vms-{count}"),
                ScenarioKind::IndependentVms,
                count,
                nodes.min(64),
                bytes,
                1,
            ));
        }
        if let Ok(filter) = env::var("VELUM_MEMORY_FILTER") {
            let selected: Vec<_> = filter.split(',').map(str::trim).collect();
            for id in &selected {
                if id.is_empty() || !scenarios.iter().any(|case| case.id == *id) {
                    bail!("unknown memory scenario filter '{id}'");
                }
            }
            scenarios.retain(|case| selected.contains(&case.id.as_str()));
        }
        Ok(Self {
            repetitions,
            child_timeout_ms: u64::try_from(timeout)?,
            scenarios,
            quickjs_compiled: cfg!(feature = "reference-quickjs"),
        })
    }
}

fn scenario(
    id: &str,
    kind: ScenarioKind,
    vm_count: usize,
    nodes_per_vm: usize,
    bytes_per_node: usize,
    rounds: usize,
) -> Scenario {
    Scenario {
        id: id.to_owned(),
        kind,
        vm_count,
        nodes_per_vm,
        bytes_per_node,
        rounds,
    }
}

fn env_usize(name: &str, default: usize, min: usize, max: usize) -> anyhow::Result<usize> {
    let value = match env::var(name) {
        Ok(value) => value
            .parse()
            .with_context(|| format!("{name} must be an integer"))?,
        Err(env::VarError::NotPresent) => default,
        Err(error) => return Err(error).with_context(|| format!("failed to read {name}")),
    };
    if value < min || value > max {
        bail!("{name} must be between {min} and {max}, got {value}");
    }
    Ok(value)
}

pub fn validate_worker(config: &WorkerConfig) -> anyhow::Result<()> {
    let case = &config.scenario;
    if config.protocol_version != PROTOCOL_VERSION {
        bail!(
            "unsupported memory worker protocol {}",
            config.protocol_version
        );
    }
    if !(1..=3_600_000).contains(&config.timeout_ms) {
        bail!("memory worker timeout must be between 1 and 3600000 milliseconds");
    }
    if case.id.is_empty()
        || case.id.len() > 64
        || !case
            .id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!("invalid memory scenario id");
    }
    if !(1..=MAX_VMS).contains(&case.vm_count)
        || !(1..=MAX_ROUNDS).contains(&case.rounds)
        || case.nodes_per_vm > MAX_NODES
        || case.bytes_per_node > MAX_BYTES_PER_NODE
    {
        bail!("memory worker scenario exceeds its bounded resource envelope");
    }
    if case.kind != ScenarioKind::HelloWorld && (case.nodes_per_vm == 0 || case.bytes_per_node == 0)
    {
        bail!("allocating memory scenario requires nonzero nodes and payload bytes");
    }
    Ok(())
}

pub fn expected_checksum(case: &Scenario) -> anyhow::Result<u64> {
    if case.kind == ScenarioKind::HelloWorld {
        return Ok(HELLO_CHECKSUM);
    }
    (0..case.nodes_per_vm).try_fold(0_u64, |sum, index| {
        sum.checked_add(u64::try_from(index % BYTE_MODULUS)?)
            .context("memory scenario checksum overflowed")
    })
}

pub fn source(case: &Scenario) -> String {
    if case.kind == ScenarioKind::HelloWorld {
        return "function __memoryAllocate() { return 42; }\nfunction __memoryVerify() { return 42; }\nfunction __memoryRelease() { return 1; }\n".to_owned();
    }
    let cycle = if case.kind == ScenarioKind::CyclicChurn {
        "for (var j = 0; j < nodes.length; j++) { nodes[j].next = nodes[(j + 1) % nodes.length]; }"
    } else {
        ""
    };
    format!(
        "var __memoryRoots = null;\nfunction __memoryAllocate() {{\n  var nodes = [];\n  for (var i = 0; i < {}; i++) {{\n    var item = {{ index: i, text: 'memory-' + i, payload: new Uint8Array({}) }};\n    item.payload[0] = i % {};\n    nodes.push(item);\n  }}\n  {}\n  __memoryRoots = nodes;\n  return __memoryVerify();\n}}\nfunction __memoryVerify() {{\n  var sum = 0;\n  for (var i = 0; i < __memoryRoots.length; i++) {{ sum += __memoryRoots[i].payload[0]; }}\n  return sum;\n}}\nfunction __memoryRelease() {{ __memoryRoots = null; return 1; }}\n",
        case.nodes_per_vm, case.bytes_per_node, BYTE_MODULUS, cycle,
    )
}
