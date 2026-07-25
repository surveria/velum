use anyhow::ensure;

use crate::compare::{OutcomeStatus, outcome};
use crate::reference_gaps::{correctness_oracle, is_engine262_unsupported};

#[test]
fn missing_math_sum_precise_v8_fallback_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = type_error("Math.sumPrecise is not a function");
    let source = "new SharedArrayBuffer(8); Math.sumPrecise(new Int16Array(1));";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn missing_v8_array_buffer_transfer_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = type_error("Int32Array.transfer is not a function");
    let source = "\
        const buffer = new SharedArrayBuffer(40, { maxByteLength: 151 });\
        let value = new BigUint64Array(1545);\
        ({\"buffer\":Int32Array} = value);\
        Int32Array[\"transfer\"]();\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn missing_v8_map_get_or_insert_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = type_error("v1.getOrInsert is not a function");
    let source = "\
        const map = new Map();\
        const shared = new SharedArrayBuffer(66, { maxByteLength: 536870889 });\
        map.getOrInsert(66, -536870889);\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn missing_v8_date_to_temporal_instant_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = type_error("date.toTemporalInstant is not a function");
    let source = "\
        const date = new Date();\
        const shared = new SharedArrayBuffer(4096, { maxByteLength: 1073741824 });\
        date.toTemporalInstant();\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn missing_engine262_date_set_year_falls_back_to_v8() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = type_error("TypeError: v2.setYear is not a function");
    let v8 = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let source = "const date = new Date(); date.setYear(38016);";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    let Some(oracle) = correctness_oracle(source, &engine262, &v8, unsupported) else {
        anyhow::bail!("expected V8 fallback oracle");
    };
    ensure!(crate::reference_gaps::outcomes_equivalent(oracle, &v8));
    Ok(())
}

#[test]
fn native_typed_array_throw_without_oracle_is_ignored() -> anyhow::Result<()> {
    let velum = js_error("Error", "function()");
    let engine262 = reference_error("ReferenceError: \"SharedArrayBuffer\" is not defined");
    let v8 = js_error("Int8Array", "function Int8Array() { [native code] }");
    let source = "new SharedArrayBuffer(0, { maxByteLength: 1 }); throw Int8Array;";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn rab_alignment_with_engine262_syntax_gap_disables_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = js_error("SyntaxError", "SyntaxError: Expected } but got z");
    let v8 = range_error("byte length of Float32Array should be a multiple of 4");
    let source = "\
        const buffer = new ArrayBuffer(3, { maxByteLength: 1879474229 });\
        new Float32Array(buffer);\
        /a{12z}/misd;\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

#[test]
fn locale_to_locale_string_with_v8_alignment_disables_oracle() -> anyhow::Result<()> {
    let velum = type_error("Intl locale entry is invalid");
    let engine262 = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let v8 = range_error("byte length of Uint32Array should be a multiple of 4");
    let source = "\
        const buffer = new ArrayBuffer(3, { maxByteLength: 960945313 });\
        new Uint32Array(buffer);\
        const locales = new BigUint64Array(2800);\
        locales.toLocaleString(locales, [-2.0]);\
    ";
    let unsupported = is_engine262_unsupported(source, &velum, &engine262, &v8);
    ensure!(unsupported);
    ensure!(correctness_oracle(source, &engine262, &v8, unsupported).is_none());
    Ok(())
}

fn reference_error(message: &str) -> crate::compare::EngineOutcome {
    js_error("ReferenceError", message)
}

fn type_error(message: &str) -> crate::compare::EngineOutcome {
    js_error("TypeError", message)
}

fn range_error(message: &str) -> crate::compare::EngineOutcome {
    js_error("RangeError", message)
}

fn js_error(name: &str, message: &str) -> crate::compare::EngineOutcome {
    outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some(name.to_owned()),
        Some(message.to_owned()),
    )
}
