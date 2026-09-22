use std::{fs, path::Path};

use anyhow::Result;
use serde_json::{Value, json};

pub fn report(executable: &Path, metadata: &Value, environment: &Value) -> Result<Value> {
    let specs = [
        ("hello-world", "hello_world", 1_u32, 0_u32, 0_u32, 1_u32),
        ("retained-graph", "retained_graph", 1, 1024, 256, 1),
        ("cyclic-churn", "cyclic_churn", 1, 1024, 256, 3),
        ("independent-vms-1", "independent_vms", 1, 64, 256, 1),
        ("independent-vms-10", "independent_vms", 10, 64, 256, 1),
        ("independent-vms-50", "independent_vms", 50, 64, 256, 1),
    ];
    let mut scenarios = Vec::new();
    let mut runs = Vec::new();
    for (id, kind, vm_count, nodes, bytes, rounds) in specs {
        scenarios.push(json!({"id":id, "kind":kind, "vm_count":vm_count,
            "nodes_per_vm":nodes, "bytes_per_node":bytes, "rounds":rounds}));
        let checksum = if id == "hello-world" {
            42
        } else {
            (0..nodes).map(|index| u64::from(index % 251)).sum::<u64>() * u64::from(vm_count)
        };
        for engine in ["velum", "quickjs"] {
            for repetition in 0..3 {
                let phases = phases(rounds).into_iter().map(|phase| {
                    let name = phase.get("name").and_then(Value::as_str);
                    let dropped = matches!(name, Some("process_baseline" | "owners_dropped"))
                        || (engine == "velum" && name == Some("vms_dropped"));
                    let count = if dropped { 0 } else { vm_count };
                    let vms = (0..count).map(|index| counters(engine, index)).collect::<Vec<_>>();
                    let live = name == Some("live");
                    let collected = name == Some("after_gc");
                    json!({"phase":phase,
                        "process":{"pid":42,"rss_bytes":{"status":"available","bytes":4096},
                            "pss_bytes":{"status":"available","bytes":2048},
                            "peak_rss_bytes":{"status":"available","bytes":8192}},
                        "diagnostics":{"vms":vms,"checksum":live.then_some(checksum),
                            "forced_gc_duration_ns":collected.then_some(100),
                            "velum_reclaimed_records":(collected && engine == "velum").then_some(1)}})
                }).collect::<Vec<_>>();
                runs.push(json!({"scenario_id":id,"engine":engine,"repetition":repetition,
                    "outcome":"passed","detail":"synthetic memory fixture","elapsed_ns":1,"phases":phases}));
            }
        }
    }
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for byte in fs::read(executable)? {
        digest ^= u64::from(byte);
        digest = digest.overflowing_mul(0x0000_0100_0000_01b3).0;
    }
    Ok(
        json!({"schema_version":1,"artifact_kind":"local_process_isolated_memory_campaign",
        "metadata":metadata,"environment":environment,"executable_path":executable,
        "executable_digest":format!("fnv1a64-{digest:016x}"),
        "configuration":{"repetitions":3,"child_timeout_ms":120_000,"quickjs_compiled":true,"scenarios":scenarios},
        "expected_worker_count":36,"campaign_complete":true,"runs":runs}),
    )
}

fn phases(rounds: u32) -> Vec<Value> {
    let mut result = vec![
        json!({"name":"process_baseline"}),
        json!({"name":"empty_vms"}),
        json!({"name":"vms_ready"}),
    ];
    for round in 0..rounds {
        for name in ["live", "roots_released", "after_gc"] {
            result.push(json!({"name":name,"round":round}));
        }
    }
    result.extend([
        json!({"name":"vms_dropped"}),
        json!({"name":"owners_dropped"}),
    ]);
    result
}

fn counters(engine: &str, index: u32) -> Value {
    if engine == "quickjs" {
        return json!({"engine":engine,"vm_index":index,"allocator_bytes":100,
            "memory_used_bytes":80,"allocator_blocks":5,"memory_used_blocks":4});
    }
    let categories = velum::VmStorageKind::all()
        .iter()
        .enumerate()
        .map(|(position, kind)| {
            json!({"category":format!("{kind:?}"),"logical_records":u64::from(position == 0),
            "logical_payload_bytes":0})
        })
        .collect::<Vec<_>>();
    json!({"engine":engine,"vm_index":index,"logical_records":1,
        "logical_payload_bytes":0,"runtime_steps":10,"categories":categories})
}
