use velum::{OptimizationMode, RuntimeLimits, Value, Vm, VmConfig, VmStorageKind};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];

#[test]
fn packed_shift_releases_property_storage() -> TestResult {
    check_case(
        "packed shift",
        "var values = [1, 2, 3]; values.shift() === 1 && values.length === 2 && \
         values[0] === 2 && values[1] === 3 && Object.keys(values).join(',') === '0,1';",
    )
}

#[test]
fn packed_unshift_reserves_property_storage() -> TestResult {
    check_case(
        "packed unshift",
        "var values = [3]; values.unshift(1, 2) === 3 && \
         values.join(',') === '1,2,3' && Object.keys(values).join(',') === '0,1,2';",
    )
}

#[test]
fn holey_shift_distinguishes_absent_and_present_properties() -> TestResult {
    check_case(
        "holey shift",
        "var values = [, 2, , 4]; values.note = 9; \
         var absent = values.shift(); var present = values.shift(); \
         absent === undefined && present === 2 && values.length === 2 && \
         !(0 in values) && values[1] === 4 && values.note === 9 && \
         Object.keys(values).join(',') === '1,note';",
    )
}

#[test]
fn empty_and_holey_unshift_preserve_enumeration() -> TestResult {
    check_case(
        "empty and holey unshift",
        "var empty = []; var values = [, 4]; \
         var zero = empty.unshift(); var added = empty.unshift(1, 2); \
         var holey = values.unshift(1, 2); var keys = ''; \
         for (var key in empty) keys += key; \
         zero === 0 && added === 2 && holey === 4 && keys === '01' && \
         !(2 in values) && values[3] === 4 && \
         Object.keys(values).join(',') === '0,1,3';",
    )
}

#[test]
fn front_mutation_fallback_keeps_descriptor_and_prototype_semantics() -> TestResult {
    check_case(
        "front mutation fallback",
        "var values = [1, 2, 3]; \
         Object.defineProperty(values, '0', {enumerable: false}); \
         var first = values.shift(); \
         var prototype = Object.create(Array.prototype); prototype[0] = 8; \
         var inherited = [, 9]; Object.setPrototypeOf(inherited, prototype); \
         var inheritedFirst = inherited.shift(); \
         first === 1 && values[0] === 2 && values.length === 2 && \
         Object.keys(values).join(',') === '1' && \
         inheritedFirst === 8 && inherited[0] === 9 && inherited.length === 1;",
    )
}

#[test]
fn repeated_front_mutations_reconcile_after_explicit_collection() -> TestResult {
    check_case(
        "repeated front mutations",
        "var total = 0; for (var i = 0; i < 64; i++) { \
             var fields = 'zone;camera;42'.split(';'); \
             fields.shift(); fields.unshift('ready', 'active'); \
             total += fields.length; \
         } total === 256;",
    )
}

#[test]
fn front_mutations_reconcile_during_automatic_collection() -> TestResult {
    const OBJECT_LIMIT: usize = 128;
    for source in [
        "var total = 0; for (var i = 0; i < 512; i++) { \
             var row = [i, i + 1]; row.shift(); total += row.length; \
         } total === 512;",
        "var total = 0; for (var i = 0; i < 512; i++) { \
             var row = []; row.unshift(i); total += row.length; \
         } total === 512;",
    ] {
        for mode in MODES {
            let config = VmConfig::with_limits(RuntimeLimits {
                max_objects: OBJECT_LIMIT,
                ..RuntimeLimits::default()
            })
            .with_optimization_mode(mode);
            let mut vm = Vm::with_config(config);
            let value = vm.eval(source)?;
            let snapshot = vm.storage_snapshot()?;
            if value != Value::Bool(true) || snapshot.count(VmStorageKind::Object) >= OBJECT_LIMIT {
                return Err(format!(
                    "automatic collection in {mode:?}: unexpected result {value:?} \
                     or object retention {snapshot:?} for {source}"
                )
                .into());
            }
        }
    }
    Ok(())
}

fn check_case(label: &str, source: &str) -> TestResult {
    for mode in MODES {
        let mut vm = Vm::with_config(VmConfig::default().with_optimization_mode(mode));
        let actual = vm.eval(source)?;
        if actual != Value::Bool(true) {
            return Err(format!("{label} in {mode:?}: expected true, got {actual:?}").into());
        }
        vm.storage_snapshot()
            .map_err(|error| format!("{label} in {mode:?}, before GC: {error}"))?;
        vm.collect_garbage()?;
        vm.storage_snapshot()
            .map_err(|error| format!("{label} in {mode:?}, after GC: {error}"))?;
    }
    Ok(())
}
