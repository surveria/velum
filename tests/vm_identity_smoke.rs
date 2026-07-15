use velum::{Engine, Runtime};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn assigns_distinct_identity_to_each_vm() -> TestResult {
    let engine = Engine::new();
    let first = engine.create_vm();
    let second = engine.create_vm();

    if first.identity() == second.identity() {
        return Err("independent VMs shared an owner identity".into());
    }
    if first.identity().generation() != second.identity().generation() {
        return Err("new VMs did not start in the same initial generation".into());
    }
    Ok(())
}

#[test]
fn vm_and_context_expose_the_same_identity() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let vm_identity = vm.identity().clone();
    let context_identity = vm.context().identity().clone();

    if vm_identity != context_identity {
        return Err("Vm and its Context exposed different identities".into());
    }
    if vm_identity != vm_identity.clone() {
        return Err("an identity clone did not retain its owner capability".into());
    }
    Ok(())
}

#[test]
fn runtime_contexts_receive_distinct_identity() -> TestResult {
    let runtime = Runtime::new();
    let first = runtime.context();
    let second = runtime.context();

    if first.identity() == second.identity() {
        return Err("independent runtime contexts shared an owner identity".into());
    }
    Ok(())
}
