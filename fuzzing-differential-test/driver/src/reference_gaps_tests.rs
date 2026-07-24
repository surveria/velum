use anyhow::ensure;

use crate::compare::{OutcomeStatus, outcome};
use crate::reference_gaps::{
    correctness_oracle, is_engine262_unsupported, outcomes_equivalent,
    source_contains_resource_management_syntax,
};

#[test]
fn missing_float16_v8_fallback_global_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = reference_error("Float16Array is not defined");
    let unsupported = is_engine262_unsupported(
        "new SharedArrayBuffer(8); new Float16Array(1);",
        &velum,
        &engine262,
        &v8,
    );
    ensure!(unsupported);
    ensure!(correctness_oracle("new Float16Array(1)", &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn missing_typed_array_base64_v8_fallback_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = type_error("Uint8Array.of(...).toBase64 is not a function");
    let unsupported = is_engine262_unsupported(
        "new SharedArrayBuffer(8); Uint8Array.of(1).toBase64();",
        &velum,
        &engine262,
        &v8,
    );
    ensure!(unsupported);
    ensure!(
        correctness_oracle("Uint8Array.of(1).toBase64()", &engine262, &v8, unsupported).is_none()
    );
    Ok(())
}

#[test]
fn missing_math_f16round_v8_fallback_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = type_error("Math.f16round is not a function");
    let source = "new SharedArrayBuffer(8); Math.f16round(1);";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn annex_b_string_legacy_engine262_gap_falls_back_to_v8() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = type_error("TypeError: (\"\").bold is not a function");
    let v8 = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let source = "(\"\").bold()";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    let Some(oracle) = correctness_oracle(source, &engine262, &v8, unsupported) else {
        anyhow::bail!("expected V8 fallback oracle");
    };
    ensure!(unsupported);
    ensure!(outcomes_equivalent(oracle, &v8));
    Ok(())
}

#[test]
fn annex_b_string_legacy_bracket_and_apply_forms_fall_back_to_v8() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = type_error("TypeError: Cannot convert undefined to object");
    let v8 = outcome(OutcomeStatus::Ok, 1, "", None, None);
    for source in [
        "(\"129\")[\"bold\"](\"bold\")",
        "(\"1D\").blink.apply(\"1D\", [])",
        "(\"take\")[\"substr\"](1073741824, 1073741824)",
        "String.prototype.trimRight.apply(\"number\", [])",
    ] {
        let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
        let Some(oracle) = correctness_oracle(source, &engine262, &v8, unsupported) else {
            anyhow::bail!("expected V8 fallback oracle for {source}");
        };
        ensure!(unsupported);
        ensure!(outcomes_equivalent(oracle, &v8));
    }
    Ok(())
}

#[test]
fn missing_immutable_array_buffer_reference_methods_disable_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = type_error("TypeError: buffer.sliceToImmutable is not a function");
    let v8 = type_error("buffer.sliceToImmutable is not a function");
    let source = "const buffer = new ArrayBuffer(); buffer.sliceToImmutable(800, 8);";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    let engine262 = outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("SyntaxError".to_owned()),
        Some("SyntaxError: Invalid identity escape".to_owned()),
    );
    let v8 = range_error("byte length of Float32Array should be a multiple of 4");
    let source = "\
        const buffer = new ArrayBuffer(230, { maxByteLength: 230 });\
        new Float32Array(buffer);\
        /[o\\9\\cA]/d;\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn missing_date_temporal_instant_reference_method_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = type_error("TypeError: v12.toTemporalInstant is not a function");
    let v8 = type_error("v12.toTemporalInstant is not a function");
    let source = "const v12 = new Date(); v12.toTemporalInstant();";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn engine262_locale_validation_gap_falls_back_to_v8() -> anyhow::Result<()> {
    let velum = outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("RangeError".to_owned()),
        Some("Intl.Locale tag or option is invalid".to_owned()),
    );
    let engine262 = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let v8 = outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("RangeError".to_owned()),
        Some("Incorrect locale information provided".to_owned()),
    );
    let source = "(5).toLocaleString(\"o\")";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    let Some(oracle) = correctness_oracle(source, &engine262, &v8, unsupported) else {
        anyhow::bail!("expected V8 fallback oracle");
    };
    ensure!(unsupported);
    ensure!(outcomes_equivalent(oracle, &v8));
    Ok(())
}

#[test]
fn resource_management_symbol_gap_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("TypeError".to_owned()),
        None,
    );
    let engine262 = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let v8 = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let source = "new Float64Array(Symbol.dispose, Symbol.dispose, Symbol.dispose)";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn engine262_only_resource_management_symbol_gap_falls_back_to_v8() -> anyhow::Result<()> {
    let velum = type_error("Cannot convert a Symbol value to a number");
    let engine262 = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let v8 = type_error("Cannot convert a Symbol value to a number");
    let source = "new Uint16Array(Symbol.asyncDispose, Symbol.asyncDispose)";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    let Some(oracle) = correctness_oracle(source, &engine262, &v8, unsupported) else {
        anyhow::bail!("expected V8 fallback oracle");
    };
    ensure!(unsupported);
    ensure!(outcomes_equivalent(oracle, &v8));
    Ok(())
}

#[test]
fn webassembly_host_api_gap_disables_oracle() -> anyhow::Result<()> {
    let velum = reference_error("ReferenceError: WebAssembly is not defined");
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = type_error("WebAssembly.Suspending is not a constructor");
    let source = "new SharedArrayBuffer(8); WebAssembly.Suspending;";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn shared_array_buffer_alignment_gap_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = range_error("byte length of BigInt64Array should be a multiple of 8");
    let source = "\
        const buffer = new SharedArrayBuffer(26, { maxByteLength: 40 });\
        new BigInt64Array(buffer);\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    let engine262 = outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("SyntaxError".to_owned()),
        Some("SyntaxError: Unexpected token".to_owned()),
    );
    let source = "/DL[p[\\0]*]/msy; \
        new Uint32Array(new SharedArrayBuffer(9, { maxByteLength: 2520 }));";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    let engine262 = range_error("RangeError: Cannot allocate memory");
    let v8 = range_error("byte length of Uint32Array should be a multiple of 4");
    let source = "new Uint32Array(new ArrayBuffer(7, { maxByteLength: 4294967296 }))";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn legacy_decimal_escape_and_v8_resizable_alignment_gap_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("SyntaxError".to_owned()),
        Some("There is no 9 capture groups".to_owned()),
    );
    let v8 = range_error("byte length of Uint32Array should be a multiple of 4");
    let source = "\
        const legacy = /8(x)(x)(x)(x)(x)(x)(x)(x)(x)(x)\\11?/ds;\
        new Uint32Array(new ArrayBuffer(1522, { maxByteLength: 1522 }));\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn legacy_control_escape_and_v8_resizable_alignment_gap_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("SyntaxError".to_owned()),
        Some("SyntaxError: Unexpected token".to_owned()),
    );
    let v8 = range_error("byte length of BigInt64Array should be a multiple of 8");
    let source = "\
        const legacy = /a\\bc\\c(ab|cde)/mgd;\
        const buffer = new ArrayBuffer(127, { maxByteLength: 536870888 });\
        new BigInt64Array(buffer);\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn legacy_quantified_lookahead_and_v8_resizable_alignment_gap_disables_oracle()
-> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("SyntaxError".to_owned()),
        Some("SyntaxError: Expected a character but got {".to_owned()),
    );
    let v8 = range_error("byte length of BigUint64Array should be a multiple of 8");
    let source = "\
        const legacy = /kaPc(?=a){1,10}a*/mi;\
        const buffer = new ArrayBuffer(4047, { maxByteLength: 4047 });\
        new BigUint64Array(buffer);\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn closing_bracket_regexp_and_v8_resizable_alignment_gap_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("SyntaxError".to_owned()),
        Some("SyntaxError: Unexpected token".to_owned()),
    );
    let v8 = range_error("byte length of Uint32Array should be a multiple of 4");
    let source = "\
        const buffer = new ArrayBuffer(129, { maxByteLength: 224 });\
        new Uint32Array(buffer);\
        /]l/ymi;\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn shared_array_buffer_zero_length_slice_gap_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = type_error("SharedArrayBuffer subclass returned this from species constructor");
    let source = "const buffer = new SharedArrayBuffer(SharedArrayBuffer, SharedArrayBuffer); buffer.slice(buffer, buffer);";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn unstable_fuzzilli_introspection_disables_oracle_when_references_disagree()
-> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "EXPLORE_ACTION: left\n", None, None);
    let engine262 = outcome(
        OutcomeStatus::Ok,
        1,
        "EXPLORE_ACTION: engine262\n",
        None,
        None,
    );
    let v8 = outcome(OutcomeStatus::Ok, 1, "EXPLORE_ACTION: v8\n", None, None);
    let source = "if (typeof fuzzilli === 'undefined') fuzzilli = function() {}; 'EXPLORE_ACTION';";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn resource_management_syntax_detector_ignores_plain_identifiers() -> anyhow::Result<()> {
    ensure!(source_contains_resource_management_syntax(
        "for (using value of []) {}"
    ));
    ensure!(!source_contains_resource_management_syntax(
        "const usingValue = 1;"
    ));
    Ok(())
}

fn reference_error(message: &str) -> crate::compare::EngineOutcome {
    outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("ReferenceError".to_owned()),
        Some(message.to_owned()),
    )
}

fn type_error(message: &str) -> crate::compare::EngineOutcome {
    outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("TypeError".to_owned()),
        Some(message.to_owned()),
    )
}

fn range_error(message: &str) -> crate::compare::EngineOutcome {
    outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some("RangeError".to_owned()),
        Some(message.to_owned()),
    )
}
