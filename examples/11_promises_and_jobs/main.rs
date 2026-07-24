use std::{
    cell::Cell,
    mem::size_of,
    task::{Context as TaskContext, Waker},
};

use velum::{Engine, HostClass, HostInstance, OwnedValue};

struct ManualCounter {
    count: Cell<u32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Engine::new().create_vm();
    vm.register_host_class(manual_counter_class())?;
    vm.register_async_host_function_typed("rustDeferred", |_call| Ok(async { Ok(20.0) }))?;
    vm.eval(
        r#"
        globalThis.trace = [];
        globalThis.result = "pending";
        const counter = new ManualCounter();
        counter.increment();
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
    let result = vm.eval_owned("`${trace.join(',')}:${globalThis.result}:${counter.count}`")?;
    if result != OwnedValue::String("first,second:42:1".to_owned()) {
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

fn manual_counter_class() -> HostClass<ManualCounter> {
    HostClass::new("ManualCounter", |_call| {
        Ok(HostInstance::new(
            ManualCounter {
                count: Cell::new(0),
            },
            size_of::<ManualCounter>(),
        ))
    })
    .getter("count", |counter, _call| Ok(f64::from(counter.count.get())))
    .method("increment", |counter, _call| {
        let next = counter
            .count
            .get()
            .checked_add(1)
            .ok_or_else(|| velum::Error::limit("manual counter overflowed"))?;
        counter.count.set(next);
        Ok(f64::from(next))
    })
}
