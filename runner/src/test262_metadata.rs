use std::{borrow::Cow, collections::BTreeSet, fs, path::Path, time::Duration};

use anyhow::{Context as _, bail};
use rs_quickjs::{Error, Runtime, RuntimeLimits};
use serde::Deserialize;

use super::{
    test262_agent::Test262AgentCoordinator, test262_compat_harness,
    test262_module_loader::Test262ModuleLoader, timing,
};

const FRONTMATTER_START: &str = "/*---";
const FRONTMATTER_END: &str = "---*/";
const HARNESS_DIR: &str = "harness";
const HARNESS_ASSERT: &str = "assert.js";
const HARNESS_DONEPRINT_HANDLE: &str = "doneprintHandle.js";
const HARNESS_STA: &str = "sta.js";
const STRICT_DIRECTIVE: &str = "\"use strict\";\n";
const DEFAULT_VARIANT: &str = "default";
const MODULE_VARIANT: &str = "module";
const STRICT_VARIANT: &str = "strict";
const RAW_VARIANT: &str = "raw";
const DEFAULT_HARNESS_FILES: [&str; 2] = [HARNESS_STA, HARNESS_ASSERT];
const ASYNC_COMPLETE_OUTPUT: &str = "Test262:AsyncTestComplete";
const ASYNC_FAILURE_PREFIX: &str = "Test262:AsyncTestFailure:";
const FLAG_ASYNC: &str = "async";
const FLAG_CAN_BLOCK_IS_TRUE: &str = "CanBlockIsTrue";
const FLAG_MODULE: &str = "module";
const FLAG_NO_STRICT: &str = "noStrict";
const FLAG_ONLY_STRICT: &str = "onlyStrict";
const FLAG_RAW: &str = "raw";
const NEGATIVE_PHASE_PARSE: &str = "parse";
const NEGATIVE_PHASE_RESOLUTION: &str = "resolution";
const NEGATIVE_PHASE_RUNTIME: &str = "runtime";
const TEST262_MAX_BINDINGS: usize = 65_536;
const TEST262_MAX_OBJECT_PROPERTIES: usize = 65_536;
const TEST262_MAX_OBJECTS: usize = 65_600;
const TEST262_MAX_RUNTIME_STEPS: usize = 100_000_000;
const TEST262_MAX_SOURCE_LEN: usize = 1_048_576;
const TEST262_MAX_STATEMENTS: usize = 65_536;
const TEST262_MAX_STRING_LEN: usize = 8_388_608;
#[cfg(test)]
const COMPAT_STA_SOURCE: &str = test262_compat_harness::STA_SOURCE;
#[cfg(test)]
const COMPAT_ASSERT_SOURCE: &str = test262_compat_harness::ASSERT_SOURCE;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Test262Outcome {
    Passed,
    Failed(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Test262CaseResult {
    pub id: String,
    pub outcome: Test262Outcome,
    pub elapsed: Duration,
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
}

impl VariantPlan {
    const fn name(&self) -> &'static str {
        match self {
            Self::Run { name, .. } => name,
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
        let result = timing::timed(|| match plan {
            VariantPlan::Run { strict, .. } => {
                execute_variant(test262_dir, relative_path, &source, &metadata, strict)
            }
        });
        results.push(Test262CaseResult {
            id,
            outcome: result.value,
            elapsed: result.elapsed,
        });
    }
    Ok(results)
}

pub fn test262_path_has_all_flags(
    test262_dir: &Path,
    relative_path: &str,
    flags: &[String],
) -> anyhow::Result<bool> {
    if flags.is_empty() {
        return Ok(true);
    }

    let path = test262_dir.join(relative_path);
    let source = fs::read_to_string(&path)
        .with_context(|| format!("failed to read upstream Test262 case '{relative_path}'"))?;
    let metadata = parse_metadata(&source, relative_path)?;
    Ok(flags.iter().all(|flag| metadata.has_flag(flag)))
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
    if metadata.has_flag(FLAG_MODULE) {
        return vec![VariantPlan::Run {
            name: if metadata.has_flag(FLAG_RAW) {
                RAW_VARIANT
            } else {
                MODULE_VARIANT
            },
            strict: true,
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
    context.set_agent_can_block(metadata.has_flag(FLAG_CAN_BLOCK_IS_TRUE));
    let agents = Test262AgentCoordinator::install(&mut context)
        .map_err(|error| anyhow::anyhow!("failed to install Test262 agents: {error}"))?;
    test262_compat_harness::install_host(&mut context)
        .map_err(|error| anyhow::anyhow!("failed to install Test262 host capabilities: {error}"))?;
    if !metadata.has_flag(FLAG_RAW) {
        for harness in harness_sources(test262_dir, metadata)? {
            context.eval(&harness.source).map_err(|error| {
                anyhow::anyhow!("Test262 harness include '{}' failed: {error}", harness.name)
            })?;
        }
    }

    let mut loader = Test262ModuleLoader::new(test262_dir);
    context.set_dynamic_module_loader(loader.clone());
    let result = if metadata.has_flag(FLAG_MODULE) {
        context.eval_module_named(relative_path, source, &mut loader)
    } else {
        let source = variant_source(source, strict);
        context.eval_named(relative_path, &source)
    };
    if let Some(negative) = &metadata.negative {
        return ensure_negative_result(relative_path, negative, result);
    }

    result.map_err(|error| {
        anyhow::anyhow!("upstream Test262 case '{relative_path}' failed: {error}")
    })?;
    context.run_jobs().map_err(|error| {
        anyhow::anyhow!("upstream Test262 case '{relative_path}' Promise jobs failed: {error}")
    })?;
    agents.finish().with_context(|| {
        format!("upstream Test262 case '{relative_path}' agent execution failed")
    })?;
    if metadata.has_flag(FLAG_ASYNC) {
        return ensure_async_completion(relative_path, context.output());
    }
    ensure_no_host_output(relative_path, context.output())?;
    Ok(())
}

fn harness_sources(
    test262_dir: &Path,
    metadata: &Test262Metadata,
) -> anyhow::Result<Vec<HarnessSource>> {
    harness_names(metadata)
        .into_iter()
        .map(|name| read_harness_source(test262_dir, &name))
        .collect()
}

fn harness_names(metadata: &Test262Metadata) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for name in DEFAULT_HARNESS_FILES {
        push_harness_name(&mut names, &mut seen, name);
    }
    if metadata.has_flag(FLAG_ASYNC) {
        push_harness_name(&mut names, &mut seen, HARNESS_DONEPRINT_HANDLE);
    }
    for name in &metadata.includes {
        push_harness_name(&mut names, &mut seen, name);
    }
    names
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
    test262_compat_harness::source(name)
}

fn variant_source(source: &str, strict: bool) -> Cow<'_, str> {
    if strict {
        return Cow::Owned(format!("{STRICT_DIRECTIVE}{source}"));
    }
    Cow::Borrowed(source)
}

fn ensure_no_host_output(relative_path: &str, output: &[String]) -> anyhow::Result<()> {
    if output.is_empty() {
        return Ok(());
    }
    bail!("upstream Test262 case '{relative_path}' produced host output")
}

fn ensure_async_completion(relative_path: &str, output: &[String]) -> anyhow::Result<()> {
    let mut completion_count = 0usize;
    for line in output {
        if line == ASYNC_COMPLETE_OUTPUT {
            completion_count = completion_count.saturating_add(1);
        } else if line.starts_with(ASYNC_FAILURE_PREFIX) {
            bail!("upstream async Test262 case '{relative_path}' signaled failure: {line}");
        } else {
            bail!(
                "upstream async Test262 case '{relative_path}' produced unexpected output: {line}"
            );
        }
    }

    if completion_count == 1 {
        return Ok(());
    }
    if completion_count == 0 {
        bail!("upstream async Test262 case '{relative_path}' did not signal completion");
    }
    bail!(
        "upstream async Test262 case '{relative_path}' signaled completion {completion_count} times"
    )
}

fn ensure_negative_result(
    relative_path: &str,
    negative: &NegativeMetadata,
    result: rs_quickjs::Result<rs_quickjs::Value>,
) -> anyhow::Result<()> {
    if negative.phase == NEGATIVE_PHASE_PARSE {
        return ensure_negative_parse_result(relative_path, negative, result);
    }
    if negative.phase == NEGATIVE_PHASE_RESOLUTION {
        return ensure_negative_resolution_result(relative_path, negative, result);
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

fn ensure_negative_resolution_result(
    relative_path: &str,
    negative: &NegativeMetadata,
    result: rs_quickjs::Result<rs_quickjs::Value>,
) -> anyhow::Result<()> {
    match result {
        Ok(_) => bail!(
            "upstream negative resolution case '{relative_path}' unexpectedly linked successfully"
        ),
        Err(Error::Lex { .. } | Error::Parse { .. } | Error::Runtime { .. }) => Ok(()),
        Err(error) => bail!(
            "upstream negative resolution case '{}' expected {} during linking, got {}",
            relative_path,
            negative.error_type,
            error
        ),
    }
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
        Err(error) if execution_error_matches(&error, &negative.error_type) => Ok(()),
        Err(error) => bail!(
            "upstream negative runtime case '{}' expected {}, got {}",
            relative_path,
            negative.error_type,
            error
        ),
    }
}

fn execution_error_matches(error: &Error, expected_type: &str) -> bool {
    let Some(actual_type) = error.javascript_error_name() else {
        return false;
    };
    actual_type == expected_type || (expected_type == "Error" && actual_type.ends_with("Error"))
}

fn variant_id(relative_path: &str, variant: &str) -> String {
    format!("{relative_path}#{variant}")
}

pub fn test262_limits() -> RuntimeLimits {
    RuntimeLimits {
        max_source_len: TEST262_MAX_SOURCE_LEN,
        max_statements: TEST262_MAX_STATEMENTS,
        max_runtime_steps: TEST262_MAX_RUNTIME_STEPS,
        max_string_len: TEST262_MAX_STRING_LEN,
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
    use rs_quickjs::{Error, Runtime, Value};

    use super::{
        ASYNC_COMPLETE_OUTPUT, ASYNC_FAILURE_PREFIX, COMPAT_ASSERT_SOURCE, COMPAT_STA_SOURCE,
        DEFAULT_VARIANT, FLAG_ASYNC, FLAG_MODULE, FLAG_NO_STRICT, FLAG_ONLY_STRICT, FLAG_RAW,
        HARNESS_ASSERT, HARNESS_DONEPRINT_HANDLE, HARNESS_STA, MODULE_VARIANT, RAW_VARIANT,
        STRICT_VARIANT, Test262Metadata, VariantPlan, ensure_async_completion,
        execution_error_matches, harness_names, parse_metadata, variant_id, variant_plans,
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    const FLAG_CAN_BLOCK_FALSE: &str = "CanBlockIsFalse";

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
    fn plans_async_tests_as_runnable_variants() -> TestResult {
        ensure_plans(
            &variant_plans(&metadata_with_flags(&[FLAG_ASYNC])),
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
    fn plans_module_tests_as_one_strict_variant() -> TestResult {
        ensure_plans(
            &variant_plans(&metadata_with_flags(&[FLAG_MODULE])),
            &[VariantPlan::Run {
                name: MODULE_VARIANT,
                strict: true,
            }],
        )
    }

    #[test]
    fn keeps_raw_module_tests_on_the_module_goal() -> TestResult {
        ensure_plans(
            &variant_plans(&metadata_with_flags(&[FLAG_MODULE, FLAG_RAW])),
            &[VariantPlan::Run {
                name: RAW_VARIANT,
                strict: true,
            }],
        )
    }

    #[test]
    fn plans_canblock_tests_as_runnable_variants() -> TestResult {
        ensure_plans(
            &variant_plans(&metadata_with_flags(&[FLAG_CAN_BLOCK_FALSE])),
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
    fn includes_doneprint_handle_for_async_tests() -> TestResult {
        ensure_texts(
            &harness_names(&metadata_with_flags(&[FLAG_ASYNC])),
            &[HARNESS_STA, HARNESS_ASSERT, HARNESS_DONEPRINT_HANDLE],
        )
    }

    #[test]
    fn compatibility_assert_preserves_mandatory_global_helpers() -> TestResult {
        let runtime = Runtime::new();
        let mut context = runtime.context();
        context.eval(COMPAT_STA_SOURCE)?;
        context.eval(COMPAT_ASSERT_SOURCE)?;
        let result = context.eval(
            r#"
                isNegativeZero(-0) &&
                !isNegativeZero(0) &&
                isPrimitive(undefined) &&
                isPrimitive(null) &&
                isPrimitive(false) &&
                isPrimitive(1) &&
                isPrimitive("value") &&
                !isPrimitive({}) &&
                !isPrimitive(function () {}) &&
                compareArray([1, NaN], [1, NaN]) &&
                compareArray.format([1, 2]) === "[1, 2]" &&
                assert._formatIdentityFreeValue(-0) === "-0"
            "#,
        )?;
        ensure_value(&result, &Value::Bool(true))
    }

    #[test]
    fn accepts_single_async_completion_signal() -> TestResult {
        ensure_unit(ensure_async_completion(
            "test/example.js",
            &[ASYNC_COMPLETE_OUTPUT.to_owned()],
        ))
    }

    #[test]
    fn rejects_async_failure_signal() -> TestResult {
        let output = format!("{ASYNC_FAILURE_PREFIX}Test262Error: bad");
        let Err(error) = ensure_async_completion("test/example.js", &[output]) else {
            return Err("expected async failure signal to fail".into());
        };
        ensure_text_contains(&error.to_string(), "signaled failure")
    }

    #[test]
    fn matches_typed_javascript_error() -> TestResult {
        let runtime = Runtime::new();
        let mut context = runtime.context();
        let Err(error) = context.eval("missingBinding") else {
            return Err("expected missing binding to throw".into());
        };
        ensure_bool(execution_error_matches(&error, "ReferenceError"))?;
        ensure_bool(execution_error_matches(&error, "Error"))?;
        ensure_bool(!execution_error_matches(&error, "TypeError"))?;
        ensure_bool(!execution_error_matches(
            &Error::runtime("ReferenceError: forged"),
            "ReferenceError",
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

    fn ensure_text_contains(actual: &str, expected: &str) -> TestResult {
        if actual.contains(expected) {
            return Ok(());
        }
        Err(format!("expected '{actual}' to contain '{expected}'").into())
    }

    fn ensure_bool(value: bool) -> TestResult {
        if value {
            return Ok(());
        }
        Err("expected true".into())
    }

    fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected {expected:?}, got {actual:?}").into())
    }

    fn ensure_unit(result: anyhow::Result<()>) -> TestResult {
        result.map_err(Into::into)
    }
}
