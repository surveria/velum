//! Engine adapters for the benchmark runner.
//!
//! Both the engine under test and the optional in-process `QuickJS` reference are
//! driven through the same `BenchEngine` lifecycle, so the benchmark sampler
//! treats them consistently and the runner stays decoupled from either engine's
//! internals. The reference is compiled only when the `reference-quickjs`
//! feature is enabled, keeping the `QuickJS` C sources out of ordinary library
//! builds.

use std::{hint::black_box, time::Instant};

use rs_quickjs::{CompiledScript, Context, RuntimeLimits};

use crate::benchmark_protocol::{
    BenchmarkChecksum, BenchmarkInput, RUN_FUNCTION, SETUP_FUNCTION, VERIFY_FUNCTION,
};

pub const REFERENCE_ENGINE_ID: &str = "rquickjs-0.9.0-bundled-quickjs-release";

const BENCH_RUNTIME_LIMITS: RuntimeLimits = RuntimeLimits {
    max_source_len: 262_144,
    max_statements: 65_536,
    max_expression_depth: 512,
    max_runtime_steps: 2_000_000_000,
    max_string_len: 1_048_576,
    max_bindings: 65_536,
    max_objects: 1_000_000,
    max_object_properties: 1_000_000,
};

/// A JavaScript engine that can evaluate a source string in a fresh top-level
/// scope. Kept deliberately tiny so the runner never depends on engine
/// internals (compiled scripts, usage counters, and so on).
pub trait BenchEngine {
    fn label(&self) -> &'static str;
    fn eval(&self, source: &str) -> anyhow::Result<()>;
    fn prepare(&self, source: &str, input: BenchmarkInput)
    -> anyhow::Result<PreparedEngineSession>;
    fn eval_with_host_image(&self, source: &str, byte_len: usize) -> anyhow::Result<()> {
        let prelude = format!("var __imageData = new Uint8Array({byte_len});\n");
        self.eval(&format!("{prelude}{source}"))
    }
}

pub trait PreparedBenchSession {
    fn run(&mut self) -> anyhow::Result<BenchmarkChecksum>;
    fn verify(&mut self) -> anyhow::Result<BenchmarkChecksum>;
}

pub struct PreparedEngineSession {
    pub compile_elapsed: Option<std::time::Duration>,
    pub setup_elapsed: std::time::Duration,
    pub session: Box<dyn PreparedBenchSession>,
}

/// The engine under test, driven only through the public runtime API. Benchmark
/// scripts use a larger resource envelope than ordinary smoke tests so active
/// workload cases can be large enough for stable timing without changing the
/// library defaults.
pub struct RsqjsEngine;

impl BenchEngine for RsqjsEngine {
    fn label(&self) -> &'static str {
        "rsqjs"
    }

    fn eval(&self, source: &str) -> anyhow::Result<()> {
        let runtime = rs_quickjs::Runtime::with_limits(BENCH_RUNTIME_LIMITS);
        let mut context = runtime.context();
        let value = context
            .eval(source)
            .map_err(|error| anyhow::anyhow!("rsqjs eval failed: {error}"))?;
        black_box(value);
        black_box(context.output().len());
        Ok(())
    }

    fn eval_with_host_image(&self, source: &str, byte_len: usize) -> anyhow::Result<()> {
        let runtime = rs_quickjs::Runtime::with_limits(BENCH_RUNTIME_LIMITS);
        let mut context = runtime.context();
        context
            .create_host_uint8_array_global("__imageData", vec![0; byte_len])
            .map_err(|error| anyhow::anyhow!("rsqjs host image setup failed: {error}"))?;
        let value = context
            .eval(source)
            .map_err(|error| anyhow::anyhow!("rsqjs eval failed: {error}"))?;
        black_box(value);
        black_box(context.output().len());
        Ok(())
    }

    fn prepare(
        &self,
        source: &str,
        input: BenchmarkInput,
    ) -> anyhow::Result<PreparedEngineSession> {
        let setup_call = function_call(SETUP_FUNCTION);
        let run_call = function_call(RUN_FUNCTION);
        let verify_call = function_call(VERIFY_FUNCTION);
        let compile_start = Instant::now();
        let runtime = rs_quickjs::Runtime::with_limits(BENCH_RUNTIME_LIMITS);
        let source_script = runtime
            .compile(source)
            .map_err(|error| anyhow::anyhow!("rsqjs benchmark source compile failed: {error}"))?;
        let setup_script = compile_call(&runtime, &setup_call, "setup")?;
        let run_script = compile_call(&runtime, &run_call, "run")?;
        let verify_script = compile_call(&runtime, &verify_call, "verify")?;
        let compile_elapsed = compile_start.elapsed();

        let setup_start = Instant::now();
        let mut context = runtime.context();
        install_rsqjs_input(&mut context, input)?;
        let source_value = context
            .eval_compiled(&source_script)
            .map_err(|error| anyhow::anyhow!("rsqjs benchmark source setup failed: {error}"))?;
        black_box(source_value);
        let setup_value = context
            .eval_compiled(&setup_script)
            .map_err(|error| anyhow::anyhow!("rsqjs benchmark setup call failed: {error}"))?;
        black_box(setup_value);
        let setup_elapsed = setup_start.elapsed();
        Ok(PreparedEngineSession {
            compile_elapsed: Some(compile_elapsed),
            setup_elapsed,
            session: Box::new(RsqjsPreparedSession {
                context,
                run_script,
                verify_script,
            }),
        })
    }
}

struct RsqjsPreparedSession {
    context: Context,
    run_script: CompiledScript,
    verify_script: CompiledScript,
}

impl PreparedBenchSession for RsqjsPreparedSession {
    fn run(&mut self) -> anyhow::Result<BenchmarkChecksum> {
        let value = self
            .context
            .eval_compiled(&self.run_script)
            .map_err(|error| anyhow::anyhow!("rsqjs prepared benchmark run failed: {error}"))?;
        BenchmarkChecksum::from_rsqjs(value)
    }

    fn verify(&mut self) -> anyhow::Result<BenchmarkChecksum> {
        let value = self
            .context
            .eval_compiled(&self.verify_script)
            .map_err(|error| anyhow::anyhow!("rsqjs prepared benchmark verify failed: {error}"))?;
        BenchmarkChecksum::from_rsqjs(value)
    }
}

fn function_call(name: &str) -> String {
    format!("{name}();")
}

fn compile_call(
    runtime: &rs_quickjs::Runtime,
    source: &str,
    phase: &str,
) -> anyhow::Result<CompiledScript> {
    runtime
        .compile(source)
        .map_err(|error| anyhow::anyhow!("rsqjs benchmark {phase} call compile failed: {error}"))
}

fn install_rsqjs_input(context: &mut Context, input: BenchmarkInput) -> anyhow::Result<()> {
    match input {
        BenchmarkInput::Standard => Ok(()),
        BenchmarkInput::HostImage { byte_len } => context
            .create_host_uint8_array_global("__imageData", vec![0; byte_len])
            .map(|value| {
                black_box(value);
            })
            .map_err(|error| anyhow::anyhow!("rsqjs host image setup failed: {error}")),
    }
}

/// Build the in-process `QuickJS` reference when the feature is enabled and the
/// runtime initialises; otherwise return `None` and benchmarks report
/// rs-quickjs numbers without a cross-engine ratio.
#[cfg(feature = "reference-quickjs")]
#[must_use]
pub fn make_reference() -> Option<Box<dyn BenchEngine>> {
    QuickjsReference::new()
        .ok()
        .map(|engine| Box::new(engine) as Box<dyn BenchEngine>)
}

#[cfg(not(feature = "reference-quickjs"))]
#[must_use]
pub fn make_reference() -> Option<Box<dyn BenchEngine>> {
    None
}

/// In-process reference over the original `QuickJS` via the `rquickjs` binding.
/// The runtime (and its garbage-collected heap) is created once; a fresh
/// context is used per evaluation so allocating benchmarks are reclaimed by GC
/// and top-level `let`/`const` bindings do not clash between iterations.
#[cfg(feature = "reference-quickjs")]
struct QuickjsReference {
    runtime: rquickjs::Runtime,
}

#[cfg(feature = "reference-quickjs")]
impl QuickjsReference {
    fn new() -> anyhow::Result<Self> {
        let runtime = rquickjs::Runtime::new()
            .map_err(|error| anyhow::anyhow!("quickjs runtime init failed: {error}"))?;
        Ok(Self { runtime })
    }
}

/// Minimal shim so benchmark scripts that call the rs-quickjs host `print` also
/// run under the reference engine: it forces argument evaluation (so the work
/// is not dead-code-eliminated) without producing any output.
#[cfg(feature = "reference-quickjs")]
const PRINT_SHIM: &str = "var print = function () {};";

#[cfg(feature = "reference-quickjs")]
impl BenchEngine for QuickjsReference {
    fn label(&self) -> &'static str {
        "quickjs"
    }

    fn eval(&self, source: &str) -> anyhow::Result<()> {
        use rquickjs::CatchResultExt as _;
        let context = rquickjs::Context::full(&self.runtime)
            .map_err(|error| anyhow::anyhow!("quickjs context init failed: {error}"))?;
        context.with(|ctx| -> anyhow::Result<()> {
            ctx.eval::<rquickjs::Value, _>(PRINT_SHIM.as_bytes())
                .catch(&ctx)
                .map_err(|caught| anyhow::anyhow!("quickjs prelude failed: {caught}"))?;
            let value = ctx
                .eval::<rquickjs::Value, _>(source.as_bytes())
                .catch(&ctx)
                .map_err(|caught| anyhow::anyhow!("{caught}"))?;
            black_box(value);
            Ok(())
        })
    }

    fn prepare(
        &self,
        source: &str,
        input: BenchmarkInput,
    ) -> anyhow::Result<PreparedEngineSession> {
        use rquickjs::CatchResultExt as _;

        let setup_start = Instant::now();
        let context = rquickjs::Context::full(&self.runtime)
            .map_err(|error| anyhow::anyhow!("quickjs context init failed: {error}"))?;
        context.with(|ctx| -> anyhow::Result<()> {
            ctx.eval::<rquickjs::Value, _>(PRINT_SHIM.as_bytes())
                .catch(&ctx)
                .map_err(|caught| anyhow::anyhow!("quickjs prelude failed: {caught}"))?;
            if let BenchmarkInput::HostImage { byte_len } = input {
                let prelude = format!("var __imageData = new Uint8Array({byte_len});");
                ctx.eval::<rquickjs::Value, _>(prelude.as_bytes())
                    .catch(&ctx)
                    .map_err(|caught| {
                        anyhow::anyhow!("quickjs host image setup failed: {caught}")
                    })?;
            }
            ctx.eval::<rquickjs::Value, _>(source.as_bytes())
                .catch(&ctx)
                .map_err(|caught| {
                    anyhow::anyhow!("quickjs benchmark source setup failed: {caught}")
                })?;
            call_quickjs_function(&ctx, SETUP_FUNCTION).map(|value| {
                black_box(value);
            })
        })?;
        let setup_elapsed = setup_start.elapsed();
        Ok(PreparedEngineSession {
            compile_elapsed: None,
            setup_elapsed,
            session: Box::new(QuickjsPreparedSession { context }),
        })
    }
}

#[cfg(feature = "reference-quickjs")]
struct QuickjsPreparedSession {
    context: rquickjs::Context,
}

#[cfg(feature = "reference-quickjs")]
impl PreparedBenchSession for QuickjsPreparedSession {
    fn run(&mut self) -> anyhow::Result<BenchmarkChecksum> {
        self.context
            .with(|ctx| call_quickjs_function(&ctx, RUN_FUNCTION))
    }

    fn verify(&mut self) -> anyhow::Result<BenchmarkChecksum> {
        self.context
            .with(|ctx| call_quickjs_function(&ctx, VERIFY_FUNCTION))
    }
}

#[cfg(feature = "reference-quickjs")]
fn call_quickjs_function(ctx: &rquickjs::Ctx<'_>, name: &str) -> anyhow::Result<BenchmarkChecksum> {
    use rquickjs::CatchResultExt as _;

    let function = ctx
        .globals()
        .get::<_, rquickjs::Function>(name)
        .catch(ctx)
        .map_err(|caught| {
            anyhow::anyhow!("quickjs benchmark function '{name}' missing: {caught}")
        })?;
    let value = function
        .call::<_, rquickjs::Value>(())
        .catch(ctx)
        .map_err(|caught| {
            anyhow::anyhow!("quickjs benchmark function '{name}' failed: {caught}")
        })?;
    quickjs_checksum(&value)
}

#[cfg(feature = "reference-quickjs")]
fn quickjs_checksum(value: &rquickjs::Value<'_>) -> anyhow::Result<BenchmarkChecksum> {
    if value.is_undefined() {
        return Ok(BenchmarkChecksum::Undefined);
    }
    if value.is_null() {
        return Ok(BenchmarkChecksum::Null);
    }
    if let Some(value) = value.as_bool() {
        return Ok(BenchmarkChecksum::boolean(value));
    }
    if let Some(value) = value.as_number() {
        return Ok(BenchmarkChecksum::number(value));
    }
    if let Some(value) = value.as_string() {
        let text = value.to_string().map_err(|error| {
            anyhow::anyhow!("quickjs benchmark string conversion failed: {error}")
        })?;
        return Ok(BenchmarkChecksum::string(text));
    }
    anyhow::bail!(
        "benchmark checksum must be a primitive value, got {}",
        value.type_name()
    )
}
