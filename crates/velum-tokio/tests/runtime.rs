use std::{cell::Cell, mem::size_of, sync::Arc};

use tokio::sync::Barrier;
use velum::{Engine, HostInstance, OwnedValue};
use velum_tokio::VmRuntime;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[velum::host_class(name = "LocalCounter", rename_all = "camelCase")]
struct LocalCounter {
    #[js(get)]
    label: String,
    private_count: Cell<u32>,
}

#[velum::host_methods]
impl LocalCounter {
    #[js(constructor)]
    fn new(label: String) -> velum::Result<HostInstance<Self>> {
        let logical_bytes = size_of::<Self>()
            .checked_add(label.len())
            .ok_or_else(|| velum::Error::limit("local counter payload size overflowed"))?;
        Ok(HostInstance::new(
            Self {
                label,
                private_count: Cell::new(0),
            },
            logical_bytes,
        ))
    }

    #[js(method)]
    // This deliberately verifies that VM-local async methods may be !Send.
    #[allow(clippy::future_not_send)]
    async fn increment(&self) -> velum::Result<f64> {
        tokio::task::yield_now().await;
        let next = self
            .private_count
            .get()
            .checked_add(1)
            .ok_or_else(|| velum::Error::limit("local counter overflowed"))?;
        self.private_count.set(next);
        Ok(f64::from(next))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn commands_for_one_vm_remain_on_one_owner_thread() -> TestResult {
    let runtime = VmRuntime::builder(Engine::new())
        .worker_threads(2)
        .build()?;
    let vm = runtime.spawn_vm().await?;
    let expected = vm.run(|_vm| owner_thread_name()).await?;

    for _ in 0..16 {
        let actual = vm.run(|_vm| owner_thread_name()).await?;
        if actual != expected {
            return Err(format!("VM moved from worker '{expected}' to '{actual}'").into());
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn independent_vms_make_async_progress_on_different_workers() -> TestResult {
    let runtime = VmRuntime::builder(Engine::new())
        .worker_threads(2)
        .build()?;
    let barrier = Arc::new(Barrier::new(2));
    let left = spawn_barrier_vm(&runtime, Arc::clone(&barrier)).await?;
    let right = spawn_barrier_vm(&runtime, barrier).await?;

    left.run(start_barrier_script).await?;
    right.run(start_barrier_script).await?;
    let (left_idle, right_idle) = tokio::join!(left.wait_idle(), right.wait_idle());
    left_idle?;
    right_idle?;

    let left_worker = left.run(read_barrier_result).await?;
    let right_worker = right.run(read_barrier_result).await?;
    if left_worker == right_worker {
        return Err(
            format!("independent VMs unexpectedly used the same worker '{left_worker}'").into(),
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_macro_method_can_use_hidden_vm_local_state() -> TestResult {
    let runtime = VmRuntime::builder(Engine::new())
        .worker_threads(1)
        .build()?;
    let vm = runtime
        .spawn_vm_with(velum::Vm::register_host_type::<LocalCounter>)
        .await?;
    vm.run(|vm| {
        vm.eval(
            r#"
            const counter = new LocalCounter("owned");
            globalThis.counterResult = "pending";
            globalThis.hiddenVisible = "private_count" in counter;
            counter.increment().then(value => {
                globalThis.counterResult = `${counter.label}|${value}`;
            });
            "#,
        )?;
        Ok(())
    })
    .await?;
    vm.wait_idle().await?;

    let result = vm.run(read_counter_result).await?;
    let expected = ("owned|1".to_owned(), false);
    if result != expected {
        return Err(format!("unexpected macro-backed counter result: {result:?}").into());
    }
    Ok(())
}

async fn spawn_barrier_vm(
    runtime: &VmRuntime,
    barrier: Arc<Barrier>,
) -> Result<velum_tokio::VmHandle, velum_tokio::RuntimeError> {
    runtime
        .spawn_vm_with(move |vm| {
            vm.register_async_host_function_typed("meetBarrier", move |_call| {
                let barrier = Arc::clone(&barrier);
                Ok(async move {
                    barrier.wait().await;
                    owner_thread_name()
                })
            })
        })
        .await
}

fn start_barrier_script(vm: &mut velum::Vm) -> velum::Result<()> {
    vm.eval(
        r#"
        globalThis.barrierResult = "pending";
        meetBarrier().then(value => {
            globalThis.barrierResult = value;
        });
        "#,
    )?;
    Ok(())
}

fn read_barrier_result(vm: &mut velum::Vm) -> velum::Result<String> {
    let OwnedValue::String(value) = vm.eval_owned("barrierResult")? else {
        return Err(velum::Error::runtime(
            "barrier result did not become a string",
        ));
    };
    Ok(value)
}

fn read_counter_result(vm: &mut velum::Vm) -> velum::Result<(String, bool)> {
    let OwnedValue::String(summary) = vm.eval_owned("counterResult")? else {
        return Err(velum::Error::runtime(
            "counter result did not become a string",
        ));
    };
    let OwnedValue::Bool(hidden_visible) = vm.eval_owned("hiddenVisible")? else {
        return Err(velum::Error::runtime(
            "hidden-field check did not become a boolean",
        ));
    };
    Ok((summary, hidden_visible))
}

fn owner_thread_name() -> velum::Result<String> {
    let thread = std::thread::current();
    let Some(name) = thread.name() else {
        return Err(velum::Error::runtime("VM worker thread has no name"));
    };
    Ok(name.to_owned())
}
