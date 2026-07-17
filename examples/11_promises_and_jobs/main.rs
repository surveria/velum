use std::task::{Context as TaskContext, Waker};

use velum::{Engine, OwnedValue};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Engine::new().create_vm();
    vm.register_async_host_function_typed("rustDeferred", |_call| Ok(async { Ok(20.0) }))?;
    vm.eval(
        r#"
        globalThis.trace = [];
        globalThis.result = "pending";
        rustDeferred()
            .then(value => {
                trace.push("first");
                return value + 1;
            })
            .then(value => {
                trace.push("second");
                result = value * 2;
            });
        "#,
    )?;
    println!(
        "Pending Rust futures before the outer loop: {}",
        vm.pending_host_future_count()
    );
    let mut context = TaskContext::from_waker(Waker::noop());
    let polled = vm.poll_host_futures(&mut context)?;
    println!(
        "Future poll completed {}; Promise jobs ready: {}",
        polled.completed(),
        vm.pending_job_count()
    );
    let completed = vm.run_jobs()?;
    let result = vm.eval_owned("`${trace.join(',')}:${globalThis.result}`")?;
    if result != OwnedValue::String("first,second:42".to_owned()) {
        return Err(format!("unexpected Promise result: {result:?}").into());
    }
    println!("Application drained {completed} jobs: {result:?}");

    vm.eval(
        r"
        globalThis.cancelledReactionRan = false;
        rustDeferred().then(() => { cancelledReactionRan = true; });
        ",
    )?;
    vm.poll_host_futures(&mut context)?;
    let cancelled = vm.cancel_jobs()?;
    let ran = vm.eval_owned("cancelledReactionRan")?;
    if ran != OwnedValue::Bool(false) {
        return Err("a cancelled Promise reaction still ran".into());
    }
    println!("Application cancelled {cancelled} queued Promise job(s)");
    Ok(())
}
