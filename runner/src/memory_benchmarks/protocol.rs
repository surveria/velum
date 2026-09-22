use anyhow::{Context as _, bail};

use super::{
    config,
    model::{
        Diagnostics, EngineKind, PROTOCOL_VERSION, Phase, VmCounters, WorkerConfig, WorkerEvent,
    },
};

pub fn ready(event: WorkerEvent, config: &WorkerConfig, pid: u32) -> anyhow::Result<()> {
    match event {
        WorkerEvent::Ready {
            protocol_version,
            engine,
            scenario_id,
            pid: actual_pid,
        } if protocol_version == PROTOCOL_VERSION
            && engine == config.engine
            && scenario_id == config.scenario.id
            && actual_pid == pid =>
        {
            Ok(())
        }
        event => bail!("invalid memory worker identity event: {event:?}"),
    }
}

pub fn phase(
    event: WorkerEvent,
    expected_index: usize,
    expected_phase: Phase,
) -> anyhow::Result<()> {
    match event {
        WorkerEvent::Phase { index, phase }
            if index == expected_index && phase == expected_phase =>
        {
            Ok(())
        }
        event => bail!("expected memory phase {expected_index} {expected_phase:?}, got {event:?}"),
    }
}

pub fn diagnostics(
    event: WorkerEvent,
    index: usize,
    phase: Phase,
    config: &WorkerConfig,
) -> anyhow::Result<Diagnostics> {
    let WorkerEvent::Diagnostics {
        index: actual_index,
        details,
    } = event
    else {
        bail!("expected memory diagnostics for phase {index}, got {event:?}");
    };
    if actual_index != index {
        bail!("memory diagnostics sequence mismatch: expected {index}, got {actual_index}");
    }
    let expected_vms = match phase {
        Phase::ProcessBaseline | Phase::OwnersDropped => 0,
        Phase::VmsDropped if config.engine == EngineKind::Velum => 0,
        _ => config.scenario.vm_count,
    };
    if details.vms.len() != expected_vms {
        bail!(
            "memory diagnostics have {} VM rows, expected {expected_vms}",
            details.vms.len()
        );
    }
    validate_counters(&details.vms, config.engine)?;
    let expected_checksum = if matches!(phase, Phase::Live { .. }) {
        Some(
            config::expected_checksum(&config.scenario)?
                .checked_mul(u64::try_from(config.scenario.vm_count)?)
                .context("memory expected checksum overflowed")?,
        )
    } else {
        None
    };
    if details.checksum != expected_checksum {
        bail!(
            "memory diagnostics checksum mismatch: expected {expected_checksum:?}, got {:?}",
            details.checksum
        );
    }
    let collected = matches!(phase, Phase::AfterGc { .. });
    if details.forced_gc_duration_ns.is_some() != collected
        || details.velum_reclaimed_records.is_some()
            != (collected && config.engine == EngineKind::Velum)
    {
        bail!("memory diagnostics contain invalid collection metadata for {phase:?}");
    }
    Ok(details)
}

fn validate_counters(counters: &[VmCounters], engine: EngineKind) -> anyhow::Result<()> {
    for (index, counter) in counters.iter().enumerate() {
        match counter {
            VmCounters::Velum {
                vm_index,
                logical_records,
                logical_payload_bytes,
                categories,
                ..
            } if engine == EngineKind::Velum && *vm_index == index => {
                if categories.len() != velum::VmStorageKind::all().len() {
                    bail!("memory storage category set is incomplete for VM {index}");
                }
                let mut records = 0_usize;
                let mut bytes = 0_usize;
                for (category, kind) in categories.iter().zip(velum::VmStorageKind::all()) {
                    if category.category != format!("{kind:?}") {
                        bail!("memory storage category order changed");
                    }
                    records = records
                        .checked_add(category.logical_records)
                        .context("memory record count overflowed")?;
                    bytes = bytes
                        .checked_add(category.logical_payload_bytes)
                        .context("memory payload sum overflowed")?;
                }
                if records != *logical_records || bytes != *logical_payload_bytes {
                    bail!("memory logical totals do not reconcile");
                }
            }
            VmCounters::Quickjs {
                vm_index,
                allocator_bytes,
                memory_used_bytes,
                allocator_blocks,
                memory_used_blocks,
            } if engine == EngineKind::Quickjs
                && *vm_index == index
                && [
                    *allocator_bytes,
                    *memory_used_bytes,
                    *allocator_blocks,
                    *memory_used_blocks,
                ]
                .iter()
                .all(|value| *value >= 0) => {}
            _ => bail!("invalid memory counter engine, index or value at VM {index}"),
        }
    }
    Ok(())
}

pub fn finished(event: WorkerEvent, expected_count: usize) -> anyhow::Result<()> {
    match event {
        WorkerEvent::Finished { phase_count } if phase_count == expected_count => Ok(()),
        event => bail!("expected memory completion after {expected_count} phases, got {event:?}"),
    }
}
