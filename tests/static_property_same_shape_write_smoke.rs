use velum::{
    HostOperation, ModuleLoader, ModuleSource, OptimizationMode, Value, Vm, VmConfig, VmGcKind,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];

#[test]
fn rotating_receivers_preserve_assignment_update_and_compound_results() -> TestResult {
    check_script(
        "rotating ordinary writes",
        r"
        function write(object, value) { return object.slot = value; }
        function increment(object) { return object.slot++; }
        function add(object, value) { return object.slot += value; }
        var first = { slot: 1, tail: 3 };
        var second = { slot: 11, tail: 7 };
        var correct = true;
        for (var index = 0; index < 16; index++) {
            correct = correct && write(first, index) === index;
            correct = correct && write(second, index + 10) === index + 10;
            correct = correct && increment(first) === index;
            correct = correct && increment(second) === index + 10;
            correct = correct && add(first, 2) === index + 3;
            correct = correct && add(second, 2) === index + 13;
        }
        correct && first.slot === 18 && second.slot === 28 &&
            first.tail === 3 && second.tail === 7;
        ",
    )
}

#[test]
fn dynamic_keys_symbols_and_key_coercion_keep_their_target_slot() -> TestResult {
    check_script(
        "dynamic own-slot writes",
        r"
        function write(object, key, value) { return object[key] = value; }
        function add(object, key, value) { return object[key] += value; }
        var symbol = Symbol('slot');
        var other = Symbol('slot');
        var first = { slot: 1, [symbol]: 3, [other]: 5 };
        var second = { slot: 11, [symbol]: 13, [other]: 17 };
        write(first, 'slot', 19);
        write(second, 'slot', 23);
        write(first, symbol, 29);
        write(second, symbol, 31);
        add(first, other, 2);
        add(second, other, 3);
        var conversions = 0;
        var key = {
            [Symbol.toPrimitive]: function () { conversions++; return symbol; }
        };
        write(first, key, 37);
        write(second, key, 41);
        first.slot === 19 && second.slot === 23 && first[symbol] === 37 &&
            second[symbol] === 41 && first[other] === 7 && second[other] === 20 &&
            conversions === 2;
        ",
    )
}

#[test]
fn matching_shape_accessors_invoke_the_current_receiver_setter() -> TestResult {
    check_script(
        "same-layout accessor writes",
        r"
        function write(object, value) { return object.slot = value; }
        function increment(object) { return object.slot++; }
        function add(object, value) { return object.slot += value; }
        var gets = 0, sets = 0, lastReceiver;
        var first = { slot: 1, backing: 3 };
        var second = {
            get slot() { gets++; return this.backing; },
            set slot(value) { sets++; lastReceiver = this; this.backing = value; },
            backing: 7
        };
        write(first, 11);
        var assigned = write(second, 13);
        increment(first);
        var old = increment(second);
        add(first, 2);
        var sum = add(second, 3);
        first.slot === 14 && assigned === 13 && old === 13 && sum === 17 &&
            second.backing === 17 && gets === 2 && sets === 3 && lastReceiver === second;
        ",
    )
}

#[test]
fn nonwritable_slots_keep_sloppy_noop_and_strict_throw_semantics() -> TestResult {
    check_script(
        "nonwritable writes",
        r"
        function write(object, value) { return object.slot = value; }
        function increment(object) { return object.slot++; }
        function add(object, value) { return object.slot += value; }
        function strictWrite(object, value) { 'use strict'; return object.slot = value; }
        function strictIncrement(object) { 'use strict'; return object.slot++; }
        function strictAdd(object, value) { 'use strict'; return object.slot += value; }
        var first = { slot: 1 };
        var second = { slot: 7 };
        Object.defineProperty(second, 'slot', { writable: false });
        write(first, 3); increment(first); add(first, 2);
        var sloppy = write(second, 11) === 11 && increment(second) === 7 &&
            add(second, 5) === 12 && second.slot === 7;
        strictWrite(first, 13); strictIncrement(first); strictAdd(first, 2);
        var failures = 0;
        try { strictWrite(second, 17); } catch (error) { if (error instanceof TypeError) failures++; }
        try { strictIncrement(second); } catch (error) { if (error instanceof TypeError) failures++; }
        try { strictAdd(second, 3); } catch (error) { if (error instanceof TypeError) failures++; }
        sloppy && failures === 3 && first.slot === 16 && second.slot === 7;
        ",
    )
}

#[test]
fn numeric_coercion_replacing_data_with_an_accessor_revalidates_the_write() -> TestResult {
    check_script(
        "coercion installs an accessor",
        r"
        function increment(object) { return object.slot++; }
        var first = { slot: 1 };
        var second = { slot: null };
        var conversions = 0, sets = 0, written = 0;
        second.slot = {
            valueOf: function () {
                conversions++;
                Object.defineProperty(second, 'slot', {
                    get: function () { return 101; },
                    set: function (value) { sets++; written = value; },
                    enumerable: true,
                    configurable: true
                });
                return 7;
            }
        };
        var warm = increment(first);
        var old = increment(second);
        warm === 1 && first.slot === 2 && old === 7 && second.slot === 101 &&
            conversions === 1 && sets === 1 && written === 8;
        ",
    )
}

#[test]
fn compound_coercion_making_a_slot_nonwritable_keeps_the_replacement_value() -> TestResult {
    check_script(
        "coercion makes data nonwritable",
        r"
        function add(object) { return object.slot += 2; }
        var first = { slot: 1 };
        var second = { slot: null };
        var conversions = 0;
        second.slot = {
            [Symbol.toPrimitive]: function () {
                conversions++;
                Object.defineProperty(second, 'slot', { value: 101, writable: false });
                return 7;
            }
        };
        var warm = add(first);
        var result = add(second);
        warm === 3 && first.slot === 3 && result === 9 && second.slot === 101 &&
            conversions === 1;
        ",
    )
}

#[test]
fn coercion_deleting_the_own_slot_resolves_the_new_prototype_setter() -> TestResult {
    check_script(
        "coercion deletes an own slot",
        r"
        function increment(object) { return object.slot++; }
        var first = { slot: 1 };
        var second = { slot: null };
        var sets = 0, written = 0, receiver;
        var prototype = {
            set slot(value) { sets++; written = value; receiver = this; }
        };
        second.slot = {
            valueOf: function () {
                delete second.slot;
                Object.setPrototypeOf(second, prototype);
                return 7;
            }
        };
        increment(first);
        var old = increment(second);
        old === 7 && first.slot === 2 && !Object.hasOwn(second, 'slot') &&
            sets === 1 && written === 8 && receiver === second;
        ",
    )
}

#[test]
fn inherited_missing_and_reordered_slots_use_normal_property_semantics() -> TestResult {
    check_script(
        "prototype and shape transitions",
        r"
        function write(object, value) { return object.slot = value; }
        function increment(object) { return object.slot++; }
        var first = { slot: 1, tail: 3 };
        var second = { slot: 7, tail: 11 };
        var prototype = { slot: 13 };
        var inherited = Object.create(prototype);
        var missing = Object.create(null);
        write(first, 17); write(second, 19);
        write(inherited, 23); write(missing, 29);
        delete second.slot;
        second.slot = 31;
        write(first, 37); write(second, 41);
        delete inherited.slot;
        var old = increment(inherited);
        Object.preventExtensions(first);
        write(first, 43);
        first.slot === 43 && first.tail === 3 && second.slot === 41 && second.tail === 11 &&
            old === 13 && inherited.slot === 14 && prototype.slot === 13 && missing.slot === 29;
        ",
    )
}

#[test]
fn exotic_receivers_preserve_proxy_array_typed_array_and_arguments_writes() -> TestResult {
    check_script(
        "exotic receiver writes",
        r"
        function writeZero(object, value) { return object[0] = value; }
        function writeSlot(object, value) { return object.slot = value; }
        var ordinary = { '0': 1 };
        var array = [3];
        var typed = new Uint8Array([7]);
        var boxed = new String('x');
        var getArgument;
        var args = (function (value) {
            getArgument = function () { return value; };
            return arguments;
        })(11);
        var traps = 0;
        var target = { '0': 13 };
        var proxy = new Proxy(target, {
            set: function (object, key, value, receiver) {
                traps++;
                if (receiver !== proxy) return false;
                object[key] = value + 1;
                return true;
            }
        });
        writeZero(ordinary, 17); writeZero(array, 19);
        writeZero(ordinary, 23); writeZero(typed, 257);
        writeZero(ordinary, 29); writeZero(boxed, 'z');
        writeZero(ordinary, 31); writeZero(args, 37);
        writeZero(ordinary, 41); writeZero(proxy, 43);
        var named = { slot: 47 };
        array.slot = 53;
        writeSlot(named, 59); writeSlot(array, 61);
        ordinary[0] === 41 && array[0] === 19 && array.length === 1 && typed[0] === 1 &&
            boxed[0] === 'x' && args[0] === 37 && getArgument() === 37 && target[0] === 44 &&
            traps === 1 && named.slot === 59 && array.slot === 61;
        ",
    )
}

#[test]
fn replacement_native_methods_invalidate_cached_call_versions() -> TestResult {
    check_script(
        "native method version updates",
        r"
        function call(object) { return object.method(-1.5); }
        function write(object, method) { object.method = method; }
        var first = { method: Math.abs };
        var second = { method: Math.abs };
        var before = call(first) + call(second);
        write(first, Math.ceil);
        write(second, Math.floor);
        before === 3 && call(first) === -1 && call(second) === -2;
        ",
    )
}

#[test]
fn global_writes_update_bindings_in_active_and_parked_realms() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        vm.context()
            .register_host_operation("createRealmForTest", HostOperation::CreateRealm)?;
        let result = vm.eval(
            r"
            function write(object, value) { object.slot = value; }
            function increment(object) { return object.slot++; }
            var other = createRealmForTest();
            other.eval('var slot = 7;');
            var slot = 1;
            var ordinary = { slot: 3 };
            write(ordinary, 11); write(globalThis, 13);
            write(ordinary, 17); write(other, 19);
            increment(ordinary); increment(globalThis);
            increment(ordinary); increment(other);
            ordinary.slot === 19 && slot === 14 && other.eval('slot') === 20;
            ",
        )?;
        ensure_value(&result, &Value::Bool(true), "realm global writes", mode)?;
        vm.storage_snapshot()?;
    }
    Ok(())
}

#[test]
fn module_namespace_writes_preserve_live_bindings_and_strict_failures() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        vm.eval_module_named(
            "main.js",
            "import * as ns from 'values.js'; globalThis.namespaceForTest = ns;",
            &mut ExportLoader,
        )?;
        let result = vm.eval(
            r"
            function write(object, value) { object.slot = value; }
            function strictWrite(object, value) { 'use strict'; object.slot = value; }
            var namespace = namespaceForTest;
            var ordinary = Object.create(null, Object.getOwnPropertyDescriptors(namespace));
            write(ordinary, 23); write(namespace, 31);
            strictWrite(ordinary, 37);
            var threw = false;
            try { strictWrite(namespace, 41); }
            catch (error) { threw = error instanceof TypeError; }
            var before = namespace.slot;
            namespace.change();
            ordinary.slot === 37 && before === 17 && namespace.slot === 29 && threw;
            ",
        )?;
        ensure_value(&result, &Value::Bool(true), "module namespace writes", mode)?;
        vm.storage_snapshot()?;
    }
    Ok(())
}

#[test]
fn collected_cached_owners_and_reused_object_slots_do_not_redirect_writes() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        vm.eval("var target = { slot: 11 }; var survivor = { slot: 31 };")?;
        let script = vm.compile("target.slot += 2; target.slot")?;
        ensure_value(
            &vm.eval_compiled(&script)?,
            &Value::Number(13.0),
            "warm owner",
            mode,
        )?;
        vm.eval("target = survivor; 0")?;
        let collected = vm.collect_garbage()?;
        if collected.reclaimed(VmGcKind::Object) == 0 {
            return Err(format!("{mode:?}: cached write owner was not reclaimed").into());
        }
        ensure_value(
            &vm.eval_compiled(&script)?,
            &Value::Number(33.0),
            "collected owner",
            mode,
        )?;
        vm.eval("target = null; survivor = null; 0")?;
        vm.collect_garbage()?;
        vm.eval("target = { slot: 97 }; 0")?;
        ensure_value(
            &vm.eval_compiled(&script)?,
            &Value::Number(99.0),
            "reused object slot",
            mode,
        )?;
        vm.storage_snapshot()?;
    }
    Ok(())
}

#[test]
fn compiled_write_sites_remain_isolated_between_vms() -> TestResult {
    for mode in MODES {
        let mut first = make_vm(mode);
        let mut second = make_vm(mode);
        first.eval("var target = { slot: 11 }; var replacement = { slot: 17 };")?;
        second.eval("var target = { slot: 31 }; var replacement = { slot: 37 };")?;
        let script = first.compile("target.slot += 2; target.slot")?;
        ensure_value(
            &first.eval_compiled(&script)?,
            &Value::Number(13.0),
            "first VM",
            mode,
        )?;
        ensure_value(
            &second.eval_compiled(&script)?,
            &Value::Number(33.0),
            "second VM",
            mode,
        )?;
        first.eval("target = replacement; 0")?;
        first.collect_garbage()?;
        ensure_value(
            &first.eval_compiled(&script)?,
            &Value::Number(19.0),
            "first VM after GC",
            mode,
        )?;
        ensure_value(
            &second.eval_compiled(&script)?,
            &Value::Number(35.0),
            "second VM after peer GC",
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
