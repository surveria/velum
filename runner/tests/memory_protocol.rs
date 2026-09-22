// Import the exact production protocol model; unrelated report/worker entrypoints
// in these shared source files are intentionally not exercised by this test crate.
#[allow(dead_code)]
#[path = "../src/memory_benchmarks/config.rs"]
mod config;
#[allow(dead_code)]
#[path = "../src/memory_benchmarks/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../src/memory_benchmarks/proc_metrics.rs"]
mod proc_metrics;
#[path = "../src/memory_benchmarks/protocol.rs"]
mod protocol;

use anyhow::{Result, ensure};
use model::{Diagnostics, EngineKind, Phase, Scenario, ScenarioKind, WorkerConfig, WorkerEvent};

fn worker() -> WorkerConfig {
    WorkerConfig {
        protocol_version: model::PROTOCOL_VERSION,
        engine: EngineKind::Velum,
        scenario: Scenario {
            id: "hello-world".to_owned(),
            kind: ScenarioKind::HelloWorld,
            vm_count: 1,
            nodes_per_vm: 0,
            bytes_per_node: 0,
            rounds: 1,
        },
        timeout_ms: 1_000,
    }
}

fn ready(pid: u32) -> WorkerEvent {
    WorkerEvent::Ready {
        protocol_version: model::PROTOCOL_VERSION,
        engine: EngineKind::Velum,
        scenario_id: "hello-world".to_owned(),
        pid,
    }
}

#[test]
fn binds_protocol_to_identity_and_exact_phase_order() -> Result<()> {
    let config = worker();
    protocol::ready(ready(100), &config, 100)?;
    ensure!(protocol::ready(ready(101), &config, 100).is_err());
    let event = WorkerEvent::Phase {
        index: 0,
        phase: Phase::ProcessBaseline,
    };
    protocol::phase(event, 0, Phase::ProcessBaseline)?;
    ensure!(
        protocol::phase(
            WorkerEvent::Phase {
                index: 1,
                phase: Phase::VmsReady
            },
            0,
            Phase::ProcessBaseline
        )
        .is_err()
    );
    ensure!(protocol::finished(WorkerEvent::Finished { phase_count: 6 }, 7).is_err());
    protocol::finished(WorkerEvent::Finished { phase_count: 7 }, 7)?;
    Ok(())
}

#[test]
fn refuses_missing_vm_counters_and_checksum_evidence() -> Result<()> {
    let config = worker();
    let event = WorkerEvent::Diagnostics {
        index: 0,
        details: Diagnostics::default(),
    };
    protocol::diagnostics(event, 0, Phase::ProcessBaseline, &config)?;
    let event = WorkerEvent::Diagnostics {
        index: 1,
        details: Diagnostics::default(),
    };
    ensure!(protocol::diagnostics(event, 1, Phase::VmsReady, &config).is_err());
    let event = WorkerEvent::Diagnostics {
        index: 2,
        details: Diagnostics {
            checksum: Some(42),
            ..Diagnostics::default()
        },
    };
    ensure!(protocol::diagnostics(event, 2, Phase::OwnersDropped, &config).is_err());
    let event = WorkerEvent::Diagnostics {
        index: 2,
        details: Diagnostics {
            forced_gc_duration_ns: Some(1),
            ..Diagnostics::default()
        },
    };
    ensure!(protocol::diagnostics(event, 2, Phase::OwnersDropped, &config).is_err());
    Ok(())
}

fn velum_counters(vm_index: usize) -> model::VmCounters {
    model::VmCounters::Velum {
        vm_index,
        logical_records: 0,
        logical_payload_bytes: 0,
        runtime_steps: 0,
        categories: velum::VmStorageKind::all()
            .iter()
            .map(|kind| model::StorageCategory {
                category: format!("{kind:?}"),
                logical_records: 0,
                logical_payload_bytes: 0,
            })
            .collect(),
    }
}

#[test]
fn live_diagnostics_require_exact_vm_set_and_checksum() -> Result<()> {
    let config = worker();
    let valid = Diagnostics {
        vms: vec![velum_counters(0)],
        checksum: Some(42),
        ..Diagnostics::default()
    };
    protocol::diagnostics(
        WorkerEvent::Diagnostics {
            index: 3,
            details: valid.clone(),
        },
        3,
        Phase::Live { round: 0 },
        &config,
    )?;
    let mut bad_checksum = valid.clone();
    bad_checksum.checksum = Some(43);
    let mut extra_vm = valid.clone();
    extra_vm.vms.push(velum_counters(1));
    let mut missing_checksum = valid.clone();
    missing_checksum.checksum = None;
    let mut wrong_index = valid;
    wrong_index.vms = vec![velum_counters(1)];
    for details in [bad_checksum, extra_vm, missing_checksum, wrong_index] {
        ensure!(
            protocol::diagnostics(
                WorkerEvent::Diagnostics { index: 3, details },
                3,
                Phase::Live { round: 0 },
                &config
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn workload_generation_and_checksum_are_deterministic() -> Result<()> {
    let mut config = worker();
    config::validate_worker(&config)?;
    ensure!(config::expected_checksum(&config.scenario)? == 42);
    config.scenario.kind = ScenarioKind::CyclicChurn;
    config.scenario.nodes_per_vm = 3;
    config.scenario.bytes_per_node = 8;
    config.scenario.rounds = 3;
    config::validate_worker(&config)?;
    ensure!(config::expected_checksum(&config.scenario)? == 3);
    let source = config::source(&config.scenario);
    ensure!(source == config::source(&config.scenario));
    ensure!(source.contains("nodes[j].next"));
    let plan = model::phases(&config.scenario);
    ensure!(plan.len() == 14);
    ensure!(plan.first() == Some(&Phase::ProcessBaseline));
    ensure!(plan.last() == Some(&Phase::OwnersDropped));
    config.scenario.vm_count = 51;
    ensure!(config::validate_worker(&config).is_err());
    Ok(())
}

#[test]
fn typed_events_reject_unknown_fields_and_variants() -> Result<()> {
    for event in [
        r#"{"event":"finished","phase_count":7,"ignored":true}"#,
        r#"{"event":"phase","index":0,"phase":{"name":"unknown"}}"#,
        r#"{"event":"diagnostics","index":0,"details":{"vms":[],"checksum":null,"forced_gc_duration_ns":null,"velum_reclaimed_records":null,"extra":1}}"#,
    ] {
        ensure!(serde_json::from_str::<WorkerEvent>(event).is_err());
    }
    Ok(())
}
