use velum::{HostOperation, OptimizationMode, Value, Vm, VmConfig, VmStorageKind};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];

fn check(source: &str) -> TestResult {
    for mode in MODES {
        let mut vm = Vm::with_config(VmConfig::default().with_optimization_mode(mode));
        vm.context()
            .register_host_operation("hostGc", HostOperation::CollectGarbage)?;
        let actual = vm.eval(source)?;
        if actual != Value::Bool(true) {
            return Err(format!(
                "eval redeclaration in {mode:?}: expected true, got {actual:?}\n{source}"
            )
            .into());
        }
        vm.collect_garbage()?;
        let snapshot = vm.storage_snapshot()?;
        for kind in [VmStorageKind::ExecutionFrame, VmStorageKind::TransientRoot] {
            if snapshot.count(kind) != 0 {
                return Err(format!("eval redeclaration in {mode:?}: unsettled {kind:?}").into());
            }
        }
    }
    Ok(())
}

#[test]
fn deleted_var_redeclaration_without_initializer_exposes_undefined() -> TestResult {
    check(
        r#"
        var value = 100;
        function run() {
            const read = () => value;
            eval("var value = 1");
            if (read() !== 1 || !eval("delete value") || read() !== 100) return false;
            const result = eval("var value; read()");
            return result === undefined && read() === undefined && eval("delete value");
        }
        run() && run() && value === 100
        "#,
    )
}

#[test]
fn function_redeclaration_replaces_a_deleted_eval_cell() -> TestResult {
    check(
        r#"
        function run() {
            const read = () => typeof value === 'undefined' ? undefined : value();
            eval("function value() { return 1; }");
            if (read() !== 1 || !eval("delete value") || read() !== undefined) return false;
            eval("function value() { return 42; }");
            hostGc();
            return read() === 42 && eval("delete value") && read() === undefined;
        }
        run() && run()
        "#,
    )
}

#[test]
fn captures_created_before_and_inside_eval_share_recreated_bindings() -> TestResult {
    check(
        r#"
        var value = 100;
        function run() {
            const early = () => value;
            const middle = eval("var value = { answer: 1 }; () => value");
            const oldValue = middle();
            if (!eval("delete value") || early() !== 100 || middle() !== 100) return false;
            const late = eval("var value = { answer: 42 }; () => value");
            hostGc();
            return early() === late() && middle() === late() &&
                late().answer === 42 && oldValue.answer === 1;
        }
        run()
        "#,
    )
}

#[test]
fn repeated_redeclaration_preserves_the_environment_after_creator_returns() -> TestResult {
    check(
        r#"
        var value = 100;
        function make() {
            const read = () => value;
            eval("var value = { answer: 0 }");
            for (let index = 1; index <= 8; index++) {
                if (!eval("delete value") || read() !== 100) return null;
                eval("var value = { answer: index }");
                hostGc();
                if (read().answer !== index) return null;
            }
            return read;
        }
        const read = make();
        hostGc();
        read !== null && read().answer === 8 && value === 100
        "#,
    )
}

#[test]
fn function_vars_and_parameters_remain_nondeletable() -> TestResult {
    check(
        r#"
        function run(parameter) {
            var local = 2;
            const read = () => parameter + local;
            if (eval("delete parameter") || eval("delete local")) return false;
            eval("var parameter = 20; var local = 22;");
            return read() === 42 && !eval("delete parameter") && !eval("delete local");
        }
        run(1)
        "#,
    )
}

#[test]
fn lexical_conflict_does_not_reactivate_a_deleted_outer_eval_binding() -> TestResult {
    check(
        r#"
        var value = 100;
        function run() {
            const read = () => value;
            eval("var value = 1");
            if (!eval("delete value")) return false;
            let rejected = false;
            {
                let value = 2;
                try { eval("var value = 3"); }
                catch (error) { rejected = error instanceof SyntaxError; }
                if (value !== 2) return false;
            }
            if (!rejected || read() !== 100) return false;
            eval("var value = 42");
            return read() === 42;
        }
        run()
        "#,
    )
}

#[test]
fn eval_closure_keeps_the_intervening_catch_parameter() -> TestResult {
    check(
        r#"
        function run() {
            let captured;
            try { throw 2; } catch (value) {
                captured = eval("var value = 3; () => value");
                if (value !== 3 || captured() !== 3) return false;
            }
            return captured() === 3;
        }
        run()
        "#,
    )
}

#[test]
fn suspended_redeclarations_reuse_the_accounted_binding_entry() -> TestResult {
    const SETUP: &str = r#"
        var value = 100;
        function* cycles() {
            var index = 0;
            const read = () => value;
            for (; index < 8; index++) {
                eval("var value = index");
                yield read();
                if (!eval("delete value") || read() !== 100) throw new Error('delete failed');
            }
        }
        var sequence = cycles();
    "#;
    for mode in MODES {
        let mut vm = Vm::with_config(VmConfig::default().with_optimization_mode(mode));
        vm.eval(SETUP)?;
        let mut expected_bindings = None;
        for index in 0_u32..8 {
            let actual = vm.eval("sequence.next().value")?;
            if actual != Value::Number(f64::from(index)) {
                return Err(format!("{mode:?}: iteration {index} returned {actual:?}").into());
            }
            vm.collect_garbage()?;
            let count = vm.storage_snapshot()?.count(VmStorageKind::Binding);
            if expected_bindings.is_some_and(|expected| expected != count) {
                return Err(
                    format!("{mode:?}: redeclaration changed binding count to {count}").into(),
                );
            }
            expected_bindings = Some(count);
        }
        let done = vm.eval("sequence.next().done")?;
        if done != Value::Bool(true) {
            return Err(format!("{mode:?}: redeclaration generator did not finish").into());
        }
        vm.collect_garbage()?;
        let settled = vm.storage_snapshot()?;
        if settled.count(VmStorageKind::ExecutionFrame) != 0
            || settled.count(VmStorageKind::TransientRoot) != 0
        {
            return Err(
                format!("{mode:?}: redeclaration generator retained execution state").into(),
            );
        }
    }
    Ok(())
}
