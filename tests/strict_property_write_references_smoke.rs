use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn strict_property_assignments_throw_for_rejected_sets() -> TestResult {
    expect_true(
        r#"
        var object = {};
        Object.defineProperty(object, "locked", { value: 1, writable: false });
        Object.defineProperty(object, "getterOnly", { get() { return 2; } });
        var caught = 0;
        (function () {
            "use strict";
            try { object.locked = 3; } catch (error) { caught += error.constructor === TypeError; }
            try { object["locked"] = 4; } catch (error) { caught += error.constructor === TypeError; }
            try { object.getterOnly = 5; } catch (error) { caught += error.constructor === TypeError; }
        })();
        caught === 3 && object.locked === 1 && object.getterOnly === 2
        "#,
    )
}

#[test]
fn strict_updates_compounds_and_logical_writes_share_set_failure() -> TestResult {
    expect_true(
        r#"
        var object = {};
        Object.defineProperty(object, "locked", { value: 1, writable: false });
        var caught = 0;
        (function () {
            "use strict";
            try { object.locked++; } catch (error) { caught += error.constructor === TypeError; }
            try { ++object["locked"]; } catch (error) { caught += error.constructor === TypeError; }
            try { object.locked += 2; } catch (error) { caught += error.constructor === TypeError; }
            try { object["locked"] &&= 3; } catch (error) { caught += error.constructor === TypeError; }
        })();
        caught === 4 && object.locked === 1
        "#,
    )
}

#[test]
fn strict_destructuring_and_non_extensible_writes_throw() -> TestResult {
    expect_true(
        r#"
        var object = {};
        Object.defineProperty(object, "locked", { value: 1, writable: false });
        Object.preventExtensions(object);
        var caught = 0;
        (function () {
            "use strict";
            try { ({ value: object.locked } = { value: 2 }); } catch (error) {
                caught += error.constructor === TypeError;
            }
            try { object.added = 3; } catch (error) {
                caught += error.constructor === TypeError;
            }
        })();
        caught === 2 && object.locked === 1 && !("added" in object)
        "#,
    )
}

#[test]
fn nullish_property_assignment_evaluates_rhs_before_type_error() -> TestResult {
    expect_true(
        r"
        var count = 0;
        var caught = false;
        try { null.value = count += 1; } catch (error) { caught = error.constructor === TypeError; }
        caught && count === 1
        ",
    )
}

#[test]
fn strict_restricted_identifiers_are_rejected_for_every_write_form() -> TestResult {
    for source in [
        r#""use strict"; eval = 1;"#,
        r#""use strict"; arguments += 1;"#,
        r#""use strict"; eval &&= 1;"#,
        r#""use strict"; arguments++;"#,
        r#""use strict"; --eval;"#,
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.context();
        if context.eval(source).is_ok() {
            return Err(format!("expected strict write target to be rejected: {source}").into());
        }
    }
    expect_true("var eval = 1; eval += 1; eval++ === 2 && eval === 3")
}

#[test]
fn postfix_updates_reject_intervening_line_terminators() -> TestResult {
    for source in ["var value = 1; value\n++;", "var value = 1; value\r--;"] {
        let runtime = Runtime::new();
        let mut context = runtime.context();
        if context.eval(source).is_ok() {
            return Err(
                format!("expected line-terminated postfix update to fail: {source:?}").into(),
            );
        }
    }
    Ok(())
}

fn expect_true(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, got {value:?}").into())
}
