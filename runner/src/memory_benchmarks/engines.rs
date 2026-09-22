use anyhow::{Context as _, bail};
use velum::{
    CompiledScript, Engine, EngineConfig, Runtime, RuntimeLimits, Value, Vm, VmConfig,
    VmStorageKind,
};

use super::{
    config,
    model::{EngineKind, Scenario, StorageCategory, VmCounters},
};

#[cfg(feature = "reference-quickjs")]
#[path = "quickjs.rs"]
mod quickjs;

pub const ALLOCATE: &str = "__memoryAllocate()";
pub const VERIFY: &str = "__memoryVerify()";
pub const RELEASE: &str = "__memoryRelease()";
pub const QUICKJS_MEMORY_LIMIT_BYTES: usize = 64 * 1_024 * 1_024;

pub fn limits() -> RuntimeLimits {
    RuntimeLimits {
        max_runtime_steps: 100_000_000,
        max_objects: 1_000_000,
        max_object_properties: 1_000_000,
        max_byte_buffer_len: 1_048_576,
        ..RuntimeLimits::default()
    }
}

pub enum Sessions {
    Velum(Box<VelumSessions>),
    #[cfg(feature = "reference-quickjs")]
    Quickjs(quickjs::QuickjsSessions),
}

impl Sessions {
    pub fn create(engine: EngineKind, case: &Scenario) -> anyhow::Result<Self> {
        match engine {
            EngineKind::Velum => Ok(Self::Velum(Box::new(VelumSessions::create(case)))),
            EngineKind::Quickjs => {
                #[cfg(feature = "reference-quickjs")]
                {
                    Ok(Self::Quickjs(quickjs::QuickjsSessions::create(case)?))
                }
                #[cfg(not(feature = "reference-quickjs"))]
                {
                    bail!("memory reference requires --features reference-quickjs")
                }
            }
        }
    }

    pub fn prepare(&mut self, case: &Scenario) -> anyhow::Result<()> {
        match self {
            Self::Velum(sessions) => sessions.prepare(case),
            #[cfg(feature = "reference-quickjs")]
            Self::Quickjs(sessions) => sessions.prepare(case),
        }
    }

    pub fn eval_verified(&mut self, source: &str, expected: u64) -> anyhow::Result<u64> {
        match self {
            Self::Velum(sessions) => sessions.eval_verified(source, expected),
            #[cfg(feature = "reference-quickjs")]
            Self::Quickjs(sessions) => sessions.eval_verified(source, expected),
        }
    }

    pub fn collect(&mut self) -> anyhow::Result<Option<usize>> {
        match self {
            Self::Velum(sessions) => sessions.collect().map(Some),
            #[cfg(feature = "reference-quickjs")]
            Self::Quickjs(sessions) => {
                sessions.collect();
                Ok(None)
            }
        }
    }

    pub fn counters(&self) -> anyhow::Result<Vec<VmCounters>> {
        match self {
            Self::Velum(sessions) => sessions.counters(),
            #[cfg(feature = "reference-quickjs")]
            Self::Quickjs(sessions) => Ok(sessions.counters()),
        }
    }

    pub fn drop_vms(&mut self) {
        match self {
            Self::Velum(sessions) => drop(std::mem::take(&mut sessions.vms)),
            #[cfg(feature = "reference-quickjs")]
            Self::Quickjs(sessions) => sessions.drop_contexts(),
        }
    }

    pub fn drop_owners(&mut self) {
        match self {
            Self::Velum(sessions) => {
                drop(sessions.script.take());
                drop(sessions.runtime.take());
            }
            #[cfg(feature = "reference-quickjs")]
            Self::Quickjs(sessions) => sessions.drop_runtimes(),
        }
    }
}

pub struct VelumSessions {
    vms: Vec<Vm>,
    script: Option<CompiledScript>,
    runtime: Option<Runtime>,
}

impl VelumSessions {
    fn create(case: &Scenario) -> Self {
        let engine = Engine::with_config(EngineConfig::with_default_vm_config(
            VmConfig::with_limits(limits()),
        ));
        let mut vms = Vec::with_capacity(case.vm_count);
        for _index in 0..case.vm_count {
            vms.push(engine.create_vm());
        }
        Self {
            vms,
            script: None,
            runtime: None,
        }
    }

    fn prepare(&mut self, case: &Scenario) -> anyhow::Result<()> {
        let runtime = Runtime::with_limits(limits());
        let source = config::source(case);
        let script = runtime.compile(&source).map_err(|error| {
            anyhow::anyhow!("Velum memory workload compilation failed: {error}")
        })?;
        for (index, vm) in self.vms.iter_mut().enumerate() {
            vm.eval_compiled(&script).map_err(|error| {
                anyhow::anyhow!("Velum memory workload setup failed in VM {index}: {error}")
            })?;
        }
        self.script = Some(script);
        self.runtime = Some(runtime);
        Ok(())
    }

    fn eval_verified(&mut self, source: &str, expected: u64) -> anyhow::Result<u64> {
        let expected_number = f64::from(u32::try_from(expected)?);
        let mut total = 0_u64;
        for (index, vm) in self.vms.iter_mut().enumerate() {
            let value = vm.eval(source).map_err(|error| {
                anyhow::anyhow!("Velum memory phase {source} failed in VM {index}: {error}")
            })?;
            if !matches!(value, Value::Number(number) if number.to_bits() == expected_number.to_bits())
            {
                bail!(
                    "Velum memory checksum mismatch in VM {index}: expected {expected}, got {value}"
                );
            }
            total = total
                .checked_add(expected)
                .context("Velum aggregate memory checksum overflowed")?;
        }
        Ok(total)
    }

    fn collect(&mut self) -> anyhow::Result<usize> {
        self.vms.iter_mut().try_fold(0_usize, |sum, vm| {
            let report = vm.collect_garbage().map_err(|error| {
                anyhow::anyhow!("Velum forced memory collection failed: {error}")
            })?;
            sum.checked_add(report.total_reclaimed())
                .context("Velum reclaimed-record count overflowed")
        })
    }

    fn counters(&self) -> anyhow::Result<Vec<VmCounters>> {
        self.vms
            .iter()
            .enumerate()
            .map(|(vm_index, vm)| {
                let snapshot = vm.storage_snapshot().map_err(|error| {
                    anyhow::anyhow!("Velum VM {vm_index} storage snapshot failed: {error}")
                })?;
                let categories = VmStorageKind::all()
                    .iter()
                    .map(|kind| StorageCategory {
                        category: format!("{kind:?}"),
                        logical_records: snapshot.count(*kind),
                        logical_payload_bytes: snapshot.payload_bytes(*kind),
                    })
                    .collect();
                Ok(VmCounters::Velum {
                    vm_index,
                    logical_records: snapshot.total(),
                    logical_payload_bytes: snapshot.total_payload_bytes(),
                    runtime_steps: vm.resource_usage().runtime_steps,
                    categories,
                })
            })
            .collect()
    }
}
