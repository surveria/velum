use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn float16_array_and_data_view_share_one_binary16_codec() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let result = context.eval(
        r"
        const values = new Float16Array([1, -2, -0, Infinity, NaN]);
        const bits = new Uint16Array(values.buffer);
        const buffer = new ArrayBuffer(4);
        const view = new DataView(buffer);
        view.setFloat16(0, 1.337, true);
        const projected = new Float16Array(buffer, 0, 1);
        const nanBits = bits[4];

        bits[0] === 0x3c00 && bits[1] === 0xc000 && bits[2] === 0x8000 &&
            bits[3] === 0x7c00 && (nanBits & 0x7c00) === 0x7c00 &&
            (nanBits & 0x03ff) !== 0 &&
            projected[0] === Math.f16round(1.337) &&
            view.getFloat16(0, true) === projected[0] ? 42 : 0
        ",
    )?;
    if result == Value::Number(42.0) {
        Ok(())
    } else {
        Err(format!("expected 42, got {result:?}").into())
    }
}
