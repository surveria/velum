use velum::{
    HostOperation, ModuleLoader, ModuleSource, OptimizationMode, Value, Vm, VmConfig, VmGcKind,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];

#[test]
fn rotating_own_data_and_methods_read_the_current_receiver() -> TestResult {
    check_script(
        "rotating own data and methods",
        r"
        function read(object) { return object.slot; }
        function invoke(object) { return object.method(2); }
        var first = { slot: 3, method: function (n) { return this.slot + n; } };
        var second = { slot: 19, method: function (n) { return this.slot * n; } };
        var total = 0;
        for (var index = 0; index < 32; index += 1) {
            total += read(first) + read(second) + invoke(first) + invoke(second);
        }
        second.slot = 23;
        total === 2080 && read(first) === 3 && read(second) === 23 && invoke(second) === 46;
        ",
    )
}

#[test]
fn dynamic_property_keys_and_symbols_keep_distinct_slots() -> TestResult {
    check_script(
        "dynamic names and symbols",
        r"
        function read(object, key) { return object[key]; }
        var symbol = Symbol('slot');
        var other = Symbol('slot');
        var first = { slot: 3, other: 5, [symbol]: 7, [other]: 11 };
        var second = { slot: 13, other: 17, [symbol]: 19, [other]: 23 };
        var total = 0;
        for (var index = 0; index < 16; index += 1) {
            total += read(first, 'slot') + read(second, 'slot');
            total += read(first, symbol) + read(second, symbol);
            total += read(first, 'other') + read(second, 'other');
            total += read(first, other) + read(second, other);
        }
        total === 1568;
        ",
    )
}

#[test]
fn matching_shape_accessors_are_not_treated_as_data() -> TestResult {
    check_script(
        "same-layout accessor descriptors",
        r"
        function read(object) { return object.slot; }
        var gets = 0;
        var first = { slot: 7, tag: 3 };
        // A setter gives the accessor the same writable shape bit as data.
        var second = {
            get slot() { gets += 1; return this.tag * 10; },
            set slot(value) { this.tag = value; },
            tag: 9
        };
        var total = 0;
        for (var index = 0; index < 16; index += 1) {
            total += read(first) + read(second);
        }
        total === 1552 && gets === 16;
        ",
    )
}

#[test]
fn inherited_and_missing_entries_remain_receiver_specific() -> TestResult {
    check_script(
        "inherited and missing receivers",
        r"
        function read(object) { return object.slot; }
        var firstPrototype = { slot: 11 };
        var secondPrototype = { slot: 29 };
        var first = Object.create(firstPrototype);
        var second = Object.create(secondPrototype);
        var missing = Object.create(null);
        var total = 0;
        var missingCorrect = true;
        for (var index = 0; index < 16; index += 1) {
            total += read(first) + read(second);
            missingCorrect = missingCorrect && read(missing) === undefined;
        }
        secondPrototype.slot = 31;
        var changed = read(second);
        Object.setPrototypeOf(first, secondPrototype);
        var replaced = read(first);
        Object.defineProperty(first, 'slot', { value: 41, configurable: true });
        var own = read(first);
        delete first.slot;
        total === 640 && missingCorrect && changed === 31 && replaced === 31 &&
            own === 41 && read(first) === 31 && read(missing) === undefined;
        ",
    )
}

#[test]
fn descriptor_and_shape_changes_preserve_reads_writes_and_deletes() -> TestResult {
    check_script(
        "descriptor and shape transitions",
        r"
        function read(object) { return object.slot; }
        function increment(object) { return object.slot++; }
        function remove(object) { return delete object.slot; }
        var first = { slot: 3, tail: 5 };
        var second = { slot: 7, tail: 11 };
        var initial = read(first) + read(second);
        var increments = increment(first) + increment(second);
        var distinct = first.slot === 4 && second.slot === 8;
        var deleted = remove(first) && read(first) === undefined && read(second) === 8;
        first.slot = 13;
        var reordered = read(first) === 13 && first.tail === 5;
        Object.defineProperty(second, 'slot', { value: 17, writable: false });
        var protectedValue = increment(second) === 17 && read(second) === 17;
        Object.defineProperty(first, 'slot', { get: function () { return 19; } });
        var accessor = read(first) === 19;
        Object.defineProperty(first, 'slot', { value: 23, writable: true });
        Object.freeze(first);
        initial === 10 && increments === 10 && distinct && deleted && reordered &&
            protectedValue && accessor && read(first) === 23 && remove(first) === false;
        ",
    )
}

#[test]
fn exotic_receivers_keep_their_semantic_dispatch() -> TestResult {
    check_script(
        "exotic receiver reads",
        r"
        function readZero(object) { return object[0]; }
        function readLength(object) { return object.length; }
        function readSlot(object) { return object.slot; }
        var ordinary = { '0': 7 };
        var array = [13];
        var typed = new Uint8Array([19]);
        var boxed = new String('x');
        var args = (function (value) { value = 29; return arguments; })(23);
        var traps = 0;
        var proxy = new Proxy({ '0': 31 }, {
            get: function (target, key) { traps += 1; return target[key] + 10; }
        });
        var zeroCorrect = readZero(ordinary) === 7 && readZero(array) === 13 &&
            readZero(ordinary) === 7 && readZero(typed) === 19 &&
            readZero(ordinary) === 7 && readZero(boxed) === 'x' &&
            readZero(ordinary) === 7 && readZero(args) === 29 &&
            readZero(ordinary) === 7 && readZero(proxy) === 41;
        var ordinaryLength = { length: 37 };
        var lengthCorrect = readLength(ordinaryLength) === 37 && readLength(array) === 1 &&
            readLength(ordinaryLength) === 37 && readLength(boxed) === 1;
        var taggedArray = [];
        taggedArray.slot = 43;
        var ordinarySlot = { slot: 47 };
        var namedCorrect = readSlot(ordinarySlot) === 47 && readSlot(taggedArray) === 43;
        zeroCorrect && lengthCorrect && namedCorrect && traps === 1;
        ",
    )
}

#[test]
fn global_objects_in_active_and_parked_realms_keep_binding_reads() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        vm.context()
            .register_host_operation("createRealmForTest", HostOperation::CreateRealm)?;
        let result = vm.eval(
            r"
            function read(object) { return object.slot; }
            var other = createRealmForTest();
            other.eval('var slot = 31;');
            var slot = 17;
            var ordinary = { slot: 7 };
            var before = read(ordinary) + read(globalThis) + read(ordinary) + read(other);
            other.eval('slot = 37;');
            slot = 19;
            before === 62 && read(ordinary) === 7 && read(other) === 37 && read(globalThis) === 19;
            ",
        )?;
        ensure_value(&result, &Value::Bool(true), "realm global reads", mode)?;
    }
    Ok(())
}

#[test]
fn module_namespace_reads_preserve_live_export_bindings() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        vm.eval_module_named(
            "main.js",
            "import * as ns from 'values.js'; globalThis.namespaceForTest = ns;",
            &mut ExportLoader,
        )?;
        let result = vm.eval(
            r"
            function read(object) { return object.slot; }
            var namespace = namespaceForTest;
            var ordinary = Object.create(null, Object.getOwnPropertyDescriptors(namespace));
            var first = read(ordinary) + read(namespace);
            namespace.change();
            first === 34 && read(ordinary) === 17 && read(namespace) === 29;
            ",
        )?;
        ensure_value(&result, &Value::Bool(true), "module namespace reads", mode)?;
    }
    Ok(())
}

#[test]
fn cached_owner_collection_and_object_slot_reuse_are_safe() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        vm.eval("var target = { slot: 11 }; var survivor = { slot: 31 };")?;
        let script = vm.compile("target.slot")?;
        ensure_value(
            &vm.eval_compiled(&script)?,
            &Value::Number(11.0),
            "warm owner",
            mode,
        )?;
        vm.eval("target = survivor; 0")?;
        let collected = vm.collect_garbage()?;
        if collected.reclaimed(VmGcKind::Object) == 0 {
            return Err(format!("{mode:?}: cached owner was not reclaimed").into());
        }
        ensure_value(
            &vm.eval_compiled(&script)?,
            &Value::Number(31.0),
            "collected owner",
            mode,
        )?;
        vm.eval("target = null; survivor = null; 0")?;
        vm.collect_garbage()?;
        vm.eval("target = { slot: 97 }; 0")?;
        ensure_value(
            &vm.eval_compiled(&script)?,
            &Value::Number(97.0),
            "reused object slot",
            mode,
        )?;
        vm.storage_snapshot()?;
    }
    Ok(())
}

#[test]
fn compiled_property_sites_remain_isolated_between_vms() -> TestResult {
    for mode in MODES {
        let mut first = make_vm(mode);
        let mut second = make_vm(mode);
        first.eval("var target = { slot: 11 }; var replacement = { slot: 17 };")?;
        second.eval("var target = { slot: 31 }; var replacement = { slot: 37 };")?;
        let script = first.compile("target.slot")?;
        for _ in 0..3 {
            ensure_value(
                &first.eval_compiled(&script)?,
                &Value::Number(11.0),
                "first VM",
                mode,
            )?;
            ensure_value(
                &second.eval_compiled(&script)?,
                &Value::Number(31.0),
                "second VM",
                mode,
            )?;
        }
        first.eval("target = replacement; 0")?;
        first.collect_garbage()?;
        ensure_value(
            &first.eval_compiled(&script)?,
            &Value::Number(17.0),
            "first VM after GC",
            mode,
        )?;
        ensure_value(
            &second.eval_compiled(&script)?,
            &Value::Number(31.0),
            "second VM after GC",
            mode,
        )?;
        first.storage_snapshot()?;
        second.storage_snapshot()?;
    }
    Ok(())
}

struct ExportLoader;

impl ModuleLoader for ExportLoader {
    fn load(&mut self, _referrer: &str, request: &str) -> velum::Result<ModuleSource> {
        if request != "values.js" {
            return Err(velum::Error::runtime(format!(
                "unexpected test module '{request}'"
            )));
        }
        Ok(ModuleSource::new(
            request,
            "export let slot = 17; export function change() { slot = 29; }",
        ))
    }
}

fn make_vm(mode: OptimizationMode) -> Vm {
    Vm::with_config(VmConfig::default().with_optimization_mode(mode))
}

fn check_script(label: &str, source: &str) -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        let script = vm.compile(source)?;
        for _ in 0..2 {
            ensure_value(&vm.eval_compiled(&script)?, &Value::Bool(true), label, mode)?;
        }
        vm.storage_snapshot()?;
    }
    Ok(())
}

fn ensure_value(
    actual: &Value,
    expected: &Value,
    label: &str,
    mode: OptimizationMode,
) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("{label} in {mode:?} mode: expected {expected:?}, got {actual:?}").into())
}
