use velum::{Engine, VmGcKind, engine_build_info};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let build = engine_build_info();
    println!(
        "Engine build: {} {} ({})",
        build.package_name, build.version, build.commit_sha
    );

    let mut vm = Engine::new().create_vm();
    let storage_before = vm.storage_snapshot()?;
    let roots_before = vm.root_snapshot()?;
    let retained = vm.eval_retained("({ label: 'explicit root' })")?;
    vm.eval(
        r"
        (() => {
            const left = {};
            const right = {};
            left.right = right;
            right.left = left;
        })();
        let total = 0;
        for (let index = 0; index < 1000; index += 1) total += index;
        total;
        ",
    )?;

    let roots_during = vm.root_snapshot()?;
    let reachability = vm.heap_reachability_snapshot()?;
    let optimization = vm.optimization_snapshot();
    let usage = vm.resource_usage();
    println!(
        "Roots: {} -> {}; unreachable objects: {}; runtime steps: {}",
        roots_before.total(),
        roots_during.total(),
        reachability.unreachable(VmGcKind::Object),
        usage.runtime_steps
    );
    println!(
        "Optimized linear runs: segments={}, direct={}",
        optimization.bytecode_linear_segment_runs(),
        optimization.bytecode_linear_direct_runs()
    );

    retained.release()?;
    let collection = vm.collect_garbage()?;
    let storage_after_gc = vm.storage_snapshot()?;
    println!(
        "GC reclaimed {} records; storage {} -> {} records",
        collection.total_reclaimed(),
        storage_before.total(),
        storage_after_gc.total()
    );

    let preview = vm.teardown_report()?;
    println!(
        "Teardown will release {} records and {} payload bytes",
        preview.storage.total(),
        preview.storage.total_payload_bytes()
    );
    let finished = vm.finish()?;
    if finished != preview {
        return Err("finish report changed after the teardown preview".into());
    }
    Ok(())
}
