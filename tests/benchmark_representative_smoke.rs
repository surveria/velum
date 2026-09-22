//! Semantic checks for the opt-in workloads, independent of timing quality.

use velum::{OptimizationMode, OwnedValue, RuntimeLimits, Vm, VmConfig};

type TestResult = Result<(), Box<dyn std::error::Error>>;

// Debug Rust frames are much larger than release benchmark frames. Semantic
// replay uses a dedicated native stack; measured runner limits stay unchanged.
const TEST_NATIVE_STACK_BYTES: usize = 32 * 1_024 * 1_024;
const TEST_GUARDED_STACK_BYTES: usize = 8 * 1_024 * 1_024;

#[derive(Clone, Copy)]
struct Workload {
    id: &'static str,
    source: &'static str,
    expected: &'static str,
}

macro_rules! workload {
    ($id:literal, $expected:literal) => {
        Workload {
            id: $id,
            source: include_str!(concat!("corpora/benchmarks/prepared/", $id, ".js")),
            expected: $expected,
        }
    };
}

// Constants were independently calculated from input indexes, not from Velum
// or the benchmark verification functions. Node also confirmed every value.
#[test]
fn object_transform_workloads_are_repeatable_in_both_modes() -> TestResult {
    check_pair([
        workload!("representative_object_transform", "3264:5156768:20032"),
        workload!("holdout_object_transform", "4384:6526912:40864"),
    ])
}

#[test]
fn method_dispatch_workloads_are_repeatable_in_both_modes() -> TestResult {
    check_pair([
        workload!("representative_method_dispatch", "8192:14996:62767175"),
        workload!("holdout_method_dispatch", "8192:131367:50723067"),
    ])
}

#[test]
fn json_ingestion_workloads_are_repeatable_in_both_modes() -> TestResult {
    check_pair([
        workload!("representative_json_ingestion", "1536:605580:71908"),
        workload!("holdout_json_ingestion", "1636:207612:82064"),
    ])
}

#[test]
fn string_processing_workloads_are_repeatable_in_both_modes() -> TestResult {
    check_pair([
        workload!("representative_string_processing", "101520:54625736:807104"),
        workload!("holdout_string_processing", "88432:43410968:977048"),
    ])
}

#[test]
fn collection_index_workloads_are_repeatable_in_both_modes() -> TestResult {
    check_pair([
        workload!("representative_collection_index", "242:1896614:86:81"),
        workload!("holdout_collection_index", "165:591214:96:82"),
    ])
}

#[test]
fn tree_allocation_workloads_are_repeatable_in_both_modes() -> TestResult {
    check_pair([
        workload!("representative_tree_allocation", "4088:2263400:31840"),
        workload!("holdout_tree_allocation", "4092:1226676:39624"),
    ])
}

fn check_pair(workloads: [Workload; 2]) -> TestResult {
    for workload in workloads {
        for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
            std::thread::Builder::new()
                .stack_size(TEST_NATIVE_STACK_BYTES)
                .spawn(move || {
                    check_workload(&workload, mode).map_err(|error| {
                        format!(
                            "benchmark fixture '{}' in {mode:?} mode: {error}",
                            workload.id
                        )
                    })
                })?
                .join()
                .map_err(|_| "benchmark semantic-replay thread panicked")??;
        }
    }
    Ok(())
}

fn check_workload(workload: &Workload, mode: OptimizationMode) -> TestResult {
    let limits = RuntimeLimits {
        max_runtime_steps: 20_000_000,
        max_call_stack_bytes: TEST_GUARDED_STACK_BYTES,
        max_bindings: 65_536,
        max_objects: 1_000_000,
        max_object_properties: 1_000_000,
        ..RuntimeLimits::default()
    };
    let mut vm = Vm::with_config(VmConfig::with_limits(limits).with_optimization_mode(mode));
    let source = vm.compile(workload.source)?;
    let setup = vm.compile("__velumBenchSetup()")?;
    let run = vm.compile("__velumBenchRun()")?;
    let verify = vm.compile("__velumBenchVerify()")?;
    vm.eval_compiled(&source)?;
    vm.eval_compiled(&setup)?;
    for iteration in 0..2 {
        let value = vm.eval_compiled_owned(&run)?;
        ensure_checksum(&value, workload.expected, &format!("run {iteration}"))?;
    }
    ensure_checksum(
        &vm.eval_compiled_owned(&verify)?,
        workload.expected,
        "verify",
    )?;
    vm.eval_compiled(&setup)?;
    ensure_checksum(
        &vm.eval_compiled_owned(&run)?,
        workload.expected,
        "reset run",
    )
}

fn ensure_checksum(actual: &OwnedValue, expected: &str, phase: &str) -> TestResult {
    if matches!(actual, OwnedValue::String(value) if value == expected) {
        return Ok(());
    }
    Err(format!("{phase}: expected checksum {expected:?}, got {actual:?}").into())
}
