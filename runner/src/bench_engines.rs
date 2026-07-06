//! Engine adapters for the benchmark runner.
//!
//! Both the engine under test and the optional in-process `QuickJS` reference are
//! driven through the same minimal `BenchEngine::eval` interface, so the
//! benchmark sampler treats them identically and the runner stays decoupled
//! from either engine's internals. The reference is compiled only when the
//! `reference-quickjs` feature is enabled, keeping the `QuickJS` C sources out of
//! ordinary library builds.

use std::hint::black_box;

/// A JavaScript engine that can evaluate a source string in a fresh top-level
/// scope. Kept deliberately tiny so the runner never depends on engine
/// internals (compiled scripts, usage counters, and so on).
pub trait BenchEngine {
    fn label(&self) -> &'static str;
    fn eval(&self, source: &str) -> anyhow::Result<()>;
}

/// The engine under test, driven only through its oldest stable public API
/// (`Runtime::new` + `eval`) so this harness can also be grafted onto historic
/// commits.
pub struct RsqjsEngine;

impl BenchEngine for RsqjsEngine {
    fn label(&self) -> &'static str {
        "rsqjs"
    }

    fn eval(&self, source: &str) -> anyhow::Result<()> {
        let runtime = rs_quickjs::Runtime::new();
        let mut context = runtime.context();
        let value = context
            .eval(source)
            .map_err(|error| anyhow::anyhow!("rsqjs eval failed: {error}"))?;
        black_box(value);
        black_box(context.output().len());
        Ok(())
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
}
