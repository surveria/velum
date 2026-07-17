use std::{cell::Cell, rc::Rc, time::Duration};

use velum::{
    Engine, EngineConfig, OwnedValue, RuntimeLimits, VmConfig, VmStorageKind, VmStorageLimits,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let storage = VmStorageLimits::unlimited()
        .with_max_count(VmStorageKind::OutputEntry, 2)
        .with_max_payload_bytes(VmStorageKind::OutputEntry, 32);
    let limits = RuntimeLimits {
        max_source_len: 1_024,
        max_call_depth: 16,
        max_call_stack_bytes: 128 * 1_024,
        max_runtime_steps: 200,
        max_string_len: 128,
        max_byte_buffer_len: 64,
        max_objects: 128,
        max_object_properties: 256,
        storage,
        ..RuntimeLimits::default()
    };
    let engine = Engine::with_config(EngineConfig::with_default_vm_config(VmConfig::with_limits(
        limits,
    )));
    let clock = Rc::new(Cell::new(Duration::ZERO));
    let clock_source = Rc::clone(&clock);
    let mut vm = engine.create_vm_with_clock(move || clock_source.get());

    let start = vm.eval_owned("performance.now()")?;
    clock.set(Duration::from_millis(25));
    let later = vm.eval_owned("performance.now()")?;
    if start != OwnedValue::Number(0.0) || later != OwnedValue::Number(25.0) {
        return Err(format!("deterministic clock mismatch: {start:?}, {later:?}").into());
    }
    println!("Deterministic clock: {start:?} -> {later:?}");

    vm.eval("print('first'); print('second');")?;
    let output_error = vm
        .eval("print('third');")
        .err()
        .ok_or("output storage limit did not reject a third entry")?;
    println!("Output limit: {output_error}");
    println!("Accepted output: {:?}", vm.take_output());

    let step_error = vm
        .eval("while (true) {}")
        .err()
        .ok_or("runtime step limit did not stop an infinite loop")?;
    println!("Step limit: {step_error}");
    Ok(())
}
