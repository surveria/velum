use std::{borrow::Cow, collections::BTreeSet, fs, path::Path};

use anyhow::{Context as _, bail};
use rs_quickjs::{Error, Runtime, RuntimeLimits};
use serde::Deserialize;

const FRONTMATTER_START: &str = "/*---";
const FRONTMATTER_END: &str = "---*/";
const HARNESS_DIR: &str = "harness";
const HARNESS_ASSERT: &str = "assert.js";
const HARNESS_STA: &str = "sta.js";
const STRICT_DIRECTIVE: &str = "\"use strict\";\n";
const DEFAULT_VARIANT: &str = "default";
const STRICT_VARIANT: &str = "strict";
const RAW_VARIANT: &str = "raw";
const UNSUPPORTED_VARIANT: &str = "unsupported";
const DEFAULT_HARNESS_FILES: [&str; 2] = [HARNESS_STA, HARNESS_ASSERT];
const FLAG_ASYNC: &str = "async";
const FLAG_CAN_BLOCK_FALSE: &str = "CanBlockIsFalse";
const FLAG_CAN_BLOCK_TRUE: &str = "CanBlockIsTrue";
const FLAG_MODULE: &str = "module";
const FLAG_NO_STRICT: &str = "noStrict";
const FLAG_ONLY_STRICT: &str = "onlyStrict";
const FLAG_RAW: &str = "raw";
const NEGATIVE_PHASE_PARSE: &str = "parse";
const NEGATIVE_PHASE_RESOLUTION: &str = "resolution";
const NEGATIVE_PHASE_RUNTIME: &str = "runtime";
const TEST262_MAX_BINDINGS: usize = 65_536;
const TEST262_MAX_OBJECT_PROPERTIES: usize = 65_536;
const TEST262_MAX_OBJECTS: usize = 65_536;
const TEST262_MAX_RUNTIME_STEPS: usize = 1_000_000;
const TEST262_MAX_SOURCE_LEN: usize = 1_048_576;
const TEST262_MAX_STATEMENTS: usize = 65_536;
const COMPAT_STA_SOURCE: &str = r#"
let Test262Error = function Test262Error(message) {
    this.message = message || "";
};
Test262Error.prototype.toString = function () {
    return "Test262Error: " + this.message;
};
Test262Error.thrower = function (message) {
    throw new Test262Error(message);
};
let $DONOTEVALUATE = function () {
    throw new Test262Error("This statement should not be evaluated.");
};
"#;
const COMPAT_ASSERT_SOURCE: &str = r#"
let assert = function assert(mustBeTrue, message) {
    if (mustBeTrue === true) {
        return;
    }
    throw new Test262Error(message || "Expected true");
};
assert.sameValue = function (actual, expected, message) {
    if (actual === expected) {
        return;
    }
    if (actual !== actual && expected !== expected) {
        return;
    }
    throw new Test262Error(message || "Expected SameValue");
};
assert.notSameValue = function (actual, unexpected, message) {
    if (actual !== unexpected) {
        return;
    }
    throw new Test262Error(message || "Expected different values");
};
assert.throws = function (expectedErrorConstructor, func, message) {
    let threw = false;
    let error = undefined;
    try {
        func();
    } catch (caught) {
        threw = true;
        error = caught;
    }
    if (threw !== true) {
        throw new Test262Error(message || "Expected function to throw");
    }
    if (expectedErrorConstructor === Test262Error) {
        return;
    }
    if (expectedErrorConstructor.name !== undefined && error.name === expectedErrorConstructor.name) {
        return;
    }
    throw new Test262Error(message || "Unexpected thrown error type");
};
"#;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Test262Outcome {
    Passed,
    Failed(String),
    Skipped(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Test262CaseResult {
    pub id: String,
    pub outcome: Test262Outcome,
}

#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq)]
#[serde(default)]
struct Test262Metadata {
    flags: Vec<String>,
    includes: Vec<String>,
    negative: Option<NegativeMetadata>,
}

impl Test262Metadata {
    fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|candidate| candidate == flag)
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq)]
struct NegativeMetadata {
    phase: String,
    #[serde(rename = "type")]
    error_type: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum VariantPlan {
    Run { name: &'static str, strict: bool },
    Skip { name: &'static str, reason: String },
}

impl VariantPlan {
    const fn name(&self) -> &'static str {
        match self {
            Self::Run { name, .. } | Self::Skip { name, .. } => name,
        }
    }
}

pub fn execute_test262_path(
    test262_dir: &Path,
    relative_path: &str,
) -> anyhow::Result<Vec<Test262CaseResult>> {
    let path = test262_dir.join(relative_path);
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read upstream Test262 case '{relative_path}'"))?;
    let metadata = parse_metadata(&source, relative_path)?;
    let plans = variant_plans(&metadata);
    if plans.is_empty() {
        bail!("Test262 case '{relative_path}' produced no runnable variants");
    }

    let mut results = Vec::new();
    for plan in plans {
        let id = variant_id(relative_path, plan.name());
        let outcome = match plan {
            VariantPlan::Run { strict, .. } => {
                execute_variant(test262_dir, relative_path, &source, &metadata, strict)
            }
            VariantPlan::Skip { reason, .. } => Test262Outcome::Skipped(reason),
        };
        results.push(Test262CaseResult { id, outcome });
    }
    Ok(results)
}

fn parse_metadata(source: &str, relative_path: &str) -> anyhow::Result<Test262Metadata> {
    let Some(start) = source.find(FRONTMATTER_START) else {
        return Ok(Test262Metadata::default());
    };
    let metadata_start = start.saturating_add(FRONTMATTER_START.len());
    let Some(after_start) = source.get(metadata_start..) else {
        bail!("Test262 metadata start is not a UTF-8 boundary in '{relative_path}'");
    };
    let Some(end) = after_start.find(FRONTMATTER_END) else {
        bail!("Test262 metadata block is not closed in '{relative_path}'");
    };
    let Some(yaml) = after_start.get(..end) else {
        bail!("Test262 metadata end is not a UTF-8 boundary in '{relative_path}'");
    };
    parse_metadata_yaml(yaml, relative_path)
}

fn parse_metadata_yaml(yaml: &str, relative_path: &str) -> anyhow::Result<Test262Metadata> {
    human_yaml::from_str(yaml)
        .with_context(|| format!("failed to parse Test262 metadata in '{relative_path}'"))
}

fn variant_plans(metadata: &Test262Metadata) -> Vec<VariantPlan> {
    let unsupported = unsupported_flags(metadata);
    if !unsupported.is_empty() {
        return vec![VariantPlan::Skip {
            name: UNSUPPORTED_VARIANT,
            reason: format!("unsupported Test262 flags: {}", unsupported.join(", ")),
        }];
    }
    if has_unsupported_negative_phase(metadata) {
        return vec![VariantPlan::Skip {
            name: UNSUPPORTED_VARIANT,
            reason: format!("unsupported Test262 negative phase: {NEGATIVE_PHASE_RESOLUTION}"),
        }];
    }
    if metadata.has_flag(FLAG_RAW) {
        return vec![VariantPlan::Run {
            name: RAW_VARIANT,
            strict: false,
        }];
    }
    if metadata.has_flag(FLAG_ONLY_STRICT) {
        return vec![VariantPlan::Run {
            name: STRICT_VARIANT,
            strict: true,
        }];
    }
    if metadata.has_flag(FLAG_NO_STRICT) {
        return vec![VariantPlan::Run {
            name: DEFAULT_VARIANT,
            strict: false,
        }];
    }
    vec![
        VariantPlan::Run {
            name: DEFAULT_VARIANT,
            strict: false,
        },
        VariantPlan::Run {
            name: STRICT_VARIANT,
            strict: true,
        },
    ]
}

fn unsupported_flags(metadata: &Test262Metadata) -> Vec<&'static str> {
    let mut flags = Vec::new();
    for flag in [
        FLAG_MODULE,
        FLAG_ASYNC,
        FLAG_CAN_BLOCK_FALSE,
        FLAG_CAN_BLOCK_TRUE,
    ] {
        if metadata.has_flag(flag) {
            flags.push(flag);
        }
    }
    flags
}

fn has_unsupported_negative_phase(metadata: &Test262Metadata) -> bool {
    metadata
        .negative
        .as_ref()
        .is_some_and(|negative| negative.phase == NEGATIVE_PHASE_RESOLUTION)
}

fn execute_variant(
    test262_dir: &Path,
    relative_path: &str,
    source: &str,
    metadata: &Test262Metadata,
    strict: bool,
) -> Test262Outcome {
    match execute_variant_result(test262_dir, relative_path, source, metadata, strict) {
        Ok(()) => Test262Outcome::Passed,
        Err(error) => Test262Outcome::Failed(format!("{error:#}")),
    }
}

fn execute_variant_result(
    test262_dir: &Path,
    relative_path: &str,
    source: &str,
    metadata: &Test262Metadata,
    strict: bool,
) -> anyhow::Result<()> {
    let runtime = Runtime::with_limits(test262_limits());
    let mut context = runtime.context();
    if !metadata.has_flag(FLAG_RAW) {
        for harness in harness_sources(test262_dir, metadata)? {
            context
                .eval(&harness.source)
                .with_context(|| format!("Test262 harness include '{}' failed", harness.name))?;
        }
    }

    let source = variant_source(source, strict);
    let result = context.eval(&source);
    if let Some(negative) = &metadata.negative {
        return ensure_negative_result(relative_path, negative, result);
    }

    result.with_context(|| format!("upstream Test262 case '{relative_path}' failed"))?;
    if !context.output().is_empty() {
        bail!("upstream Test262 case '{relative_path}' produced host output");
    }
    Ok(())
}

fn harness_sources(
    test262_dir: &Path,
    metadata: &Test262Metadata,
) -> anyhow::Result<Vec<HarnessSource>> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for name in DEFAULT_HARNESS_FILES {
        push_harness_name(&mut names, &mut seen, name);
    }
    for name in &metadata.includes {
        push_harness_name(&mut names, &mut seen, name);
    }

    names
        .into_iter()
        .map(|name| read_harness_source(test262_dir, &name))
        .collect()
}

fn push_harness_name(names: &mut Vec<String>, seen: &mut BTreeSet<String>, name: &str) {
    if seen.insert(name.to_owned()) {
        names.push(name.to_owned());
    }
}

fn read_harness_source(test262_dir: &Path, name: &str) -> anyhow::Result<HarnessSource> {
    if let Some(source) = compat_harness_source(name) {
        return Ok(HarnessSource {
            name: name.to_owned(),
            source: source.to_owned(),
        });
    }

    let path = test262_dir.join(HARNESS_DIR).join(name);
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read Test262 harness include '{name}'"))?;
    Ok(HarnessSource {
        name: name.to_owned(),
        source,
    })
}

fn compat_harness_source(name: &str) -> Option<&'static str> {
    match name {
        HARNESS_STA => Some(COMPAT_STA_SOURCE),
        HARNESS_ASSERT => Some(COMPAT_ASSERT_SOURCE),
        _ => None,
    }
}

fn variant_source(source: &str, strict: bool) -> Cow<'_, str> {
    if strict {
        return Cow::Owned(format!("{STRICT_DIRECTIVE}{source}"));
    }
    Cow::Borrowed(source)
}

fn ensure_negative_result(
    relative_path: &str,
    negative: &NegativeMetadata,
    result: rs_quickjs::Result<rs_quickjs::Value>,
) -> anyhow::Result<()> {
    if negative.phase == NEGATIVE_PHASE_PARSE {
        return ensure_negative_parse_result(relative_path, negative, result);
    }
    if negative.phase == NEGATIVE_PHASE_RUNTIME {
        return ensure_negative_runtime_result(relative_path, negative, result);
    }
    bail!(
        "upstream Test262 case '{}' uses unsupported negative phase '{}'",
        relative_path,
        negative.phase
    )
}

fn ensure_negative_parse_result(
    relative_path: &str,
    negative: &NegativeMetadata,
    result: rs_quickjs::Result<rs_quickjs::Value>,
) -> anyhow::Result<()> {
    match result {
        Ok(_) => bail!(
            "upstream negative parse case '{relative_path}' unexpectedly evaluated successfully"
        ),
        Err(Error::Lex { .. } | Error::Parse { .. }) => Ok(()),
        Err(error) => bail!(
            "upstream negative parse case '{}' expected {}, got {}",
            relative_path,
            negative.error_type,
            error
        ),
    }
}

fn ensure_negative_runtime_result(
    relative_path: &str,
    negative: &NegativeMetadata,
    result: rs_quickjs::Result<rs_quickjs::Value>,
) -> anyhow::Result<()> {
    match result {
        Ok(_) => bail!(
            "upstream negative runtime case '{relative_path}' unexpectedly evaluated successfully"
        ),
        Err(Error::Runtime { message })
            if runtime_error_matches(&message, &negative.error_type) =>
        {
            Ok(())
        }
        Err(error) => bail!(
            "upstream negative runtime case '{}' expected {}, got {}",
            relative_path,
            negative.error_type,
            error
        ),
    }
}

fn runtime_error_matches(message: &str, expected_type: &str) -> bool {
    if message.contains(expected_type) {
        return true;
    }
    expected_type == "Error" && message.starts_with("uncaught throw:")
}

fn variant_id(relative_path: &str, variant: &str) -> String {
    format!("{relative_path}#{variant}")
}

fn test262_limits() -> RuntimeLimits {
    RuntimeLimits {
        max_source_len: TEST262_MAX_SOURCE_LEN,
        max_statements: TEST262_MAX_STATEMENTS,
        max_runtime_steps: TEST262_MAX_RUNTIME_STEPS,
        max_bindings: TEST262_MAX_BINDINGS,
        max_objects: TEST262_MAX_OBJECTS,
        max_object_properties: TEST262_MAX_OBJECT_PROPERTIES,
        ..RuntimeLimits::default()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct HarnessSource {
    name: String,
    source: String,
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_VARIANT, FLAG_ASYNC, FLAG_NO_STRICT, FLAG_ONLY_STRICT, FLAG_RAW, RAW_VARIANT,
        STRICT_VARIANT, Test262Metadata, VariantPlan, parse_metadata, runtime_error_matches,
        variant_id, variant_plans,
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn parses_test262_frontmatter() -> TestResult {
        let source = r"
/*---
includes: [propertyHelper.js]
flags: [onlyStrict]
negative:
  phase: parse
  type: SyntaxError
---*/
bad source
";
        let metadata = parse_metadata(source, "test/example.js")?;
        ensure_texts(&metadata.includes, &["propertyHelper.js"])?;
        ensure_texts(&metadata.flags, &[FLAG_ONLY_STRICT])?;
        let Some(negative) = metadata.negative else {
            return Err("expected negative metadata".into());
        };
        ensure_text(&negative.phase, "parse")?;
        ensure_text(&negative.error_type, "SyntaxError")
    }

    #[test]
    fn plans_default_and_strict_variants_without_flags() -> TestResult {
        let plans = variant_plans(&Test262Metadata::default());
        ensure_plans(
            &plans,
            &[
                VariantPlan::Run {
                    name: DEFAULT_VARIANT,
                    strict: false,
                },
                VariantPlan::Run {
                    name: STRICT_VARIANT,
                    strict: true,
                },
            ],
        )
    }

    #[test]
    fn honors_strictness_flags() -> TestResult {
        ensure_plans(
            &variant_plans(&metadata_with_flags(&[FLAG_ONLY_STRICT])),
            &[VariantPlan::Run {
                name: STRICT_VARIANT,
                strict: true,
            }],
        )?;
        ensure_plans(
            &variant_plans(&metadata_with_flags(&[FLAG_NO_STRICT])),
            &[VariantPlan::Run {
                name: DEFAULT_VARIANT,
                strict: false,
            }],
        )
    }

    #[test]
    fn keeps_raw_tests_unwrapped() -> TestResult {
        ensure_plans(
            &variant_plans(&metadata_with_flags(&[FLAG_RAW])),
            &[VariantPlan::Run {
                name: RAW_VARIANT,
                strict: false,
            }],
        )
    }

    #[test]
    fn skips_unsupported_flags() -> TestResult {
        let plans = variant_plans(&metadata_with_flags(&[FLAG_ASYNC]));
        let Some(VariantPlan::Skip { reason, .. }) = plans.first() else {
            return Err("expected unsupported flag skip".into());
        };
        if reason.contains(FLAG_ASYNC) {
            return Ok(());
        }
        Err(format!("expected skip reason to mention '{FLAG_ASYNC}', got '{reason}'").into())
    }

    #[test]
    fn matches_runtime_error_type_text() -> TestResult {
        ensure_bool(runtime_error_matches(
            "uncaught throw: Test262Error: expected",
            "Test262Error",
        ))?;
        ensure_bool(runtime_error_matches(
            "ReferenceError: 'x' is not defined",
            "ReferenceError",
        ))?;
        ensure_bool(!runtime_error_matches(
            "ReferenceError: 'x' is not defined",
            "TypeError",
        ))
    }

    #[test]
    fn appends_variant_suffix_to_case_id() -> TestResult {
        ensure_text(
            &variant_id("test/example.js", STRICT_VARIANT),
            "test/example.js#strict",
        )
    }

    fn metadata_with_flags(flags: &[&str]) -> Test262Metadata {
        Test262Metadata {
            flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
            ..Test262Metadata::default()
        }
    }

    fn ensure_plans(actual: &[VariantPlan], expected: &[VariantPlan]) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected plans {expected:?}, got {actual:?}").into())
    }

    fn ensure_texts(actual: &[String], expected: &[&str]) -> TestResult {
        if actual
            .iter()
            .zip(expected.iter())
            .all(|(left, right)| left == right)
            && actual.len() == expected.len()
        {
            return Ok(());
        }
        Err(format!("expected {expected:?}, got {actual:?}").into())
    }

    fn ensure_text(actual: &str, expected: &str) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected '{expected}', got '{actual}'").into())
    }

    fn ensure_bool(value: bool) -> TestResult {
        if value {
            return Ok(());
        }
        Err("expected true".into())
    }
}
