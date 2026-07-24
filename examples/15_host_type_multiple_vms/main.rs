use core::mem::size_of;

use tokio::sync::Mutex;
use velum::{Engine, HostInstance, OwnedValue};
use velum_tokio::{VmHandle, VmRuntime};

#[velum::host_class(name = "AppCounter", rename_all = "camelCase")]
struct AppCounter {
    #[js(get)]
    label: String,
    private_count: Mutex<u32>,
}

#[velum::host_methods]
impl AppCounter {
    #[js(constructor)]
    fn new(label: String) -> velum::Result<HostInstance<Self>> {
        let logical_bytes = size_of::<Self>()
            .checked_add(label.len())
            .ok_or_else(|| velum::Error::limit("counter payload size overflowed"))?;
        Ok(HostInstance::new(
            Self {
                label,
                private_count: Mutex::new(0),
            },
            logical_bytes,
        ))
    }

    #[js(method)]
    async fn increment(&self) -> velum::Result<f64> {
        tokio::task::yield_now().await;
        let mut count = self.private_count.lock().await;
        *count = count
            .checked_add(1)
            .ok_or_else(|| velum::Error::limit("counter value overflowed"))?;
        Ok(f64::from(*count))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::builder(Engine::new())
        .worker_threads(3)
        .build()?;
    let alpha = spawn_counter_vm(&runtime, "alpha", 1).await?;
    let beta = spawn_counter_vm(&runtime, "beta", 2).await?;
    let gamma = spawn_counter_vm(&runtime, "gamma", 3).await?;

    let (alpha_result, beta_result, gamma_result) =
        tokio::join!(run_counter(&alpha), run_counter(&beta), run_counter(&gamma));
    let actual = [alpha_result?, beta_result?, gamma_result?];
    let expected = ["alpha:1:false", "beta:2:false", "gamma:3:false"];
    if actual != expected {
        return Err(format!("independent counter results were {actual:?}").into());
    }
    println!("One macro-defined host type, three isolated instances: {actual:?}");
    Ok(())
}

async fn spawn_counter_vm(
    runtime: &VmRuntime,
    label: &'static str,
    steps: u32,
) -> Result<VmHandle, velum_tokio::RuntimeError> {
    runtime
        .spawn_vm_with(move |vm| {
            vm.register_host_type::<AppCounter>()?;
            vm.register_host_function_typed("instanceLabel", move |_call| Ok(label))?;
            vm.register_host_function_typed("instanceSteps", move |_call| Ok(f64::from(steps)))
        })
        .await
}

async fn run_counter(vm: &VmHandle) -> Result<String, velum_tokio::RuntimeError> {
    vm.run(|vm| {
        vm.eval(
            r#"
            const counter = new AppCounter(instanceLabel());
            globalThis.summary = "pending";
            (async () => {
                for (let step = 0; step < instanceSteps(); step += 1) {
                    await counter.increment();
                }
                globalThis.summary = [
                    counter.label,
                    await counter.increment() - 1,
                    "private_count" in counter
                ].join(":");
            })();
            "#,
        )?;
        Ok(())
    })
    .await?;
    vm.wait_idle().await?;
    vm.run(|vm| {
        let OwnedValue::String(summary) = vm.eval_owned("summary")? else {
            return Err(velum::Error::runtime("counter summary was not a string"));
        };
        Ok(summary)
    })
    .await
}
