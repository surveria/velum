use velum::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn structured_loop_preserves_apply_and_has_instance_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let source = r"
        function sum() {
            var total = 0;
            for (var i = 0; i < arguments.length; i++) { total += arguments[i]; }
            return total;
        }
        function Animal() {}
        function Dog() {}
        Dog.prototype = Object.create(Animal.prototype);
        var hasInstance = Function.prototype[Symbol.hasInstance];
        var total = 0;
        for (var round = 0; round < 64; round++) {
            total += sum.apply(null, [round, round + 1, round + 2, round + 3]);
            total += sum.apply({}, { length: 3, 0: round, 1: round + 1, 2: round + 2 });
            var dog = new Dog();
            if (dog instanceof Dog) { total += 1; }
            if (dog instanceof Animal) { total += 1; }
            if (hasInstance.call(Dog, dog)) { total += 1; }
            if (!hasInstance.call(Dog, round)) { total += 1; }
        }
        total
    ";
    let script = vm.compile(source)?;
    let before = vm.resource_usage();
    let value = vm.eval_compiled(&script)?;
    let after = vm.resource_usage();
    ensure_value(&value, &Value::Number(14_944.0))?;
    let direct_runs = after
        .bytecode_linear_direct_runs
        .checked_sub(before.bytecode_linear_direct_runs)
        .ok_or("bytecode direct run counter moved backwards")?;
    if direct_runs < 64 {
        return Err(format!("expected at least 64 direct runs, got {direct_runs}").into());
    }
    Ok(())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
