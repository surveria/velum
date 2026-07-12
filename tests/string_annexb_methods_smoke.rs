use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_annex_b_string_methods_with_standard_descriptors() -> TestResult {
    eval_is_42(
        r#"
        let methods = [
            ["anchor", 1], ["big", 0], ["blink", 0], ["bold", 0],
            ["fixed", 0], ["fontcolor", 1], ["fontsize", 1], ["italics", 0],
            ["link", 1], ["small", 0], ["strike", 0], ["sub", 0],
            ["substr", 2], ["sup", 0]
        ];
        let valid = true;
        for (let entry of methods) {
            let descriptor = Object.getOwnPropertyDescriptor(String.prototype, entry[0]);
            valid = valid && descriptor.value.name === entry[0] &&
                descriptor.value.length === entry[1] && descriptor.writable &&
                !descriptor.enumerable && descriptor.configurable;
        }
        valid ? 42 : 0
        "#,
    )
}

#[test]
fn creates_legacy_html_wrappers_with_required_coercion_and_escaping() -> TestResult {
    eval_is_42(
        r#"
        let log = "";
        let receiver = { toString() { log = log + "r"; return "text"; } };
        let argument = { toString() { log = log + "a"; return "x\"&y"; } };
        let anchor = String.prototype.anchor.call(receiver, argument);
        let wrappers =
            "x".big() + "x".blink() + "x".bold() + "x".fixed() +
            "x".fontcolor("red") + "x".fontsize(3) + "x".italics() +
            "x".link("url") + "x".small() + "x".strike() +
            "x".sub() + "x".sup();
        let expected =
            "<big>x</big><blink>x</blink><b>x</b><tt>x</tt>" +
            "<font color=\"red\">x</font><font size=\"3\">x</font><i>x</i>" +
            "<a href=\"url\">x</a><small>x</small><strike>x</strike>" +
            "<sub>x</sub><sup>x</sup>";
        anchor === "<a name=\"x&quot;&y\">text</a>" && log === "ra" &&
            wrappers === expected ? 42 : 0
        "#,
    )
}

#[test]
fn substr_uses_utf16_indices_and_annex_b_bounds() -> TestResult {
    eval_is_42(
        r#"
        let order = "";
        let receiver = { toString() { order = order + "r"; return "abcdef"; } };
        let start = { valueOf() { order = order + "s"; return 1; } };
        let length = { valueOf() { order = order + "l"; return 3; } };
        let coerced = String.prototype.substr.call(receiver, start, length);
        let lone = "\ud800x".substr(0, 1);
        coerced === "bcd" && order === "rsl" &&
            "012345".substr(-2) === "45" &&
            "012345".substr(-20, 2) === "01" &&
            "012345".substr(2, -1) === "" &&
            "012345".substr(-Infinity, 2) === "01" &&
            "012345".substr(Infinity) === "" &&
            String.prototype.substr.call(12345, 1, 3) === "234" &&
            lone.length === 1 && lone.charCodeAt(0) === 0xd800 ? 42 : 0
        "#,
    )
}

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected value Number(42), got {value:?}").into())
}
