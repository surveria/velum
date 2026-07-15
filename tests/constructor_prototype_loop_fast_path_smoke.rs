use velum::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn structured_loop_preserves_constructor_prototype_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let source = r#"
        let total = 0;
        let Camera = function Camera(value) { this.value = value; };
        Camera.prototype.bump = function(delta) {
            this.value += delta;
            return this.value;
        };
        Camera.prototype.read = function() { return this.value; };

        for (let index = 0; index < 64; index++) {
            let camera = new Camera(index);
            total += camera.bump(1);
            total += camera.read();
            if ("bump" in camera) { total += 1; }
        }
        total
    "#;
    let script = vm.compile(source)?;

    let before = vm.resource_usage();
    let value = vm.eval_compiled(&script)?;
    let after = vm.resource_usage();

    ensure_value(&value, &Value::Number(4_224.0))?;
    let direct_runs = after
        .bytecode_linear_direct_runs
        .checked_sub(before.bytecode_linear_direct_runs)
        .ok_or("bytecode direct run counter moved backwards")?;
    if direct_runs < 64 {
        return Err(format!("expected at least 64 direct bytecode runs, got {direct_runs}").into());
    }
    Ok(())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
