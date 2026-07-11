use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn expect_value(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    if &actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

#[test]
fn supports_numeric_switch_dispatch() -> TestResult {
    expect_value(
        r"
        let values = [10, 20, 30, 40];
        let total = 0;
        for (let index = 0; index < 8; index = index + 1) {
            switch (index & 3) {
                case 0:
                    total = total + values[0];
                    break;
                case 1:
                    total = total + values[1];
                    break;
                case 2:
                    total = total + values[2];
                    break;
                default:
                    total = total + values[3];
            }
        }
        total
        ",
        &Value::Number(200.0),
    )
}

#[test]
fn preserves_expression_case_fallback_order() -> TestResult {
    expect_value(
        r"
        let calls = 0;
        let hit = function(value) {
            calls = calls * 10 + value + 1;
            return value;
        };
        let total = 0;
        switch (1) {
            case hit(0):
                total = 100;
                break;
            case hit(1):
                total = calls + 40;
                break;
            default:
                total = 0;
        }
        total
        ",
        &Value::Number(52.0),
    )
}
