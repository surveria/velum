use anyhow::{Context as _, bail};
use rquickjs::CatchResultExt as _;

use super::{QUICKJS_MEMORY_LIMIT_BYTES, Scenario, VmCounters, config};

pub struct QuickjsSessions {
    contexts: Vec<rquickjs::Context>,
    runtimes: Vec<rquickjs::Runtime>,
}

impl QuickjsSessions {
    pub fn create(case: &Scenario) -> anyhow::Result<Self> {
        let mut sessions = Self {
            contexts: Vec::with_capacity(case.vm_count),
            runtimes: Vec::with_capacity(case.vm_count),
        };
        for _index in 0..case.vm_count {
            let runtime =
                rquickjs::Runtime::new().context("QuickJS memory runtime initialization failed")?;
            runtime.set_memory_limit(QUICKJS_MEMORY_LIMIT_BYTES);
            let context = rquickjs::Context::full(&runtime)
                .context("QuickJS full context initialization failed")?;
            sessions.contexts.push(context);
            sessions.runtimes.push(runtime);
        }
        Ok(sessions)
    }

    pub fn prepare(&self, case: &Scenario) -> anyhow::Result<()> {
        let source = config::source(case);
        for (index, context) in self.contexts.iter().enumerate() {
            context.with(|ctx| {
                ctx.eval::<rquickjs::Value, _>(source.as_bytes())
                    .catch(&ctx)
                    .map(drop)
                    .map_err(|error| {
                        anyhow::anyhow!(
                            "QuickJS memory workload setup failed in VM {index}: {error}"
                        )
                    })
            })?;
        }
        Ok(())
    }

    pub fn eval_verified(&self, source: &str, expected: u64) -> anyhow::Result<u64> {
        let expected_number = f64::from(u32::try_from(expected)?);
        let mut total = 0_u64;
        for (index, context) in self.contexts.iter().enumerate() {
            let value = context.with(|ctx| {
                ctx.eval::<f64, _>(source.as_bytes())
                    .catch(&ctx)
                    .map_err(|error| {
                        anyhow::anyhow!(
                            "QuickJS memory phase {source} failed in VM {index}: {error}"
                        )
                    })
            })?;
            if value.to_bits() != expected_number.to_bits() {
                bail!(
                    "QuickJS memory checksum mismatch in VM {index}: expected {expected}, got {value}"
                );
            }
            total = total
                .checked_add(expected)
                .context("QuickJS aggregate memory checksum overflowed")?;
        }
        Ok(total)
    }

    pub fn collect(&self) {
        for runtime in &self.runtimes {
            runtime.run_gc();
        }
    }

    pub fn counters(&self) -> Vec<VmCounters> {
        self.runtimes
            .iter()
            .enumerate()
            .map(|(vm_index, runtime)| {
                let usage = runtime.memory_usage();
                VmCounters::Quickjs {
                    vm_index,
                    allocator_bytes: usage.malloc_size,
                    memory_used_bytes: usage.memory_used_size,
                    allocator_blocks: usage.malloc_count,
                    memory_used_blocks: usage.memory_used_count,
                }
            })
            .collect()
    }

    pub fn drop_contexts(&mut self) {
        drop(std::mem::take(&mut self.contexts));
    }

    pub fn drop_runtimes(&mut self) {
        drop(std::mem::take(&mut self.runtimes));
    }
}
