use std::time::Instant;

use rs_quickjs::{Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn timed(label: &str, source: &str) -> TestResult {
    let limits = RuntimeLimits {
        max_runtime_steps: usize::MAX,
        max_objects: 50_000_000,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();
    let start = Instant::now();
    let value = context.eval(source)?;
    let elapsed = start.elapsed();
    let shown = match value {
        Value::Number(n) => n.to_string(),
        other => format!("{other:?}"),
    };
    println!(
        "{label:<22} {:>8.1}ms  result={shown}",
        elapsed.as_secs_f64() * 1000.0
    );
    Ok(())
}

#[test]
fn perf_probe() -> TestResult {
    timed(
        "bare loop 3M",
        "let t=0; for (let i=0;i<3000000;i++){ t+=i&3; } t",
    )?;
    timed(
        "prop read 3M",
        "let o={v:1}; let t=0; for (let i=0;i<3000000;i++){ t+=o.v; } t",
    )?;
    timed(
        "prop write 3M",
        "let o={v:1}; for (let i=0;i<3000000;i++){ o.v=i; } o.v",
    )?;
    timed(
        "prop RMW 3M",
        "let o={v:1}; for (let i=0;i<3000000;i++){ o.v+=1; } o.v",
    )?;
    timed(
        "plain call 3M",
        "function f(){return 1} let t=0; for (let i=0;i<3000000;i++){ t+=f(); } t",
    )?;
    timed(
        "method call 3M",
        "let o={m:function(){return this.v},v:1}; let t=0; for (let i=0;i<3000000;i++){ t+=o.m(); } t",
    )?;
    timed(
        "new+field 1M",
        "function C(v){this.v=v} let t=0; for (let i=0;i<1000000;i++){ t+=new C(i).v; } t",
    )?;
    timed(
        "in op 3M",
        "let o={v:1}; let t=0; for (let i=0;i<3000000;i++){ if(\"v\" in o) t+=1; } t",
    )?;
    timed(
        "elem RMW 3M",
        "let a=[1,2,3,4]; for (let i=0;i<3000000;i++){ a[i&3]+=1; } a[0]",
    )?;
    timed(
        "method nothis 3M",
        "let o={m:function(){return 1}}; let t=0; for (let i=0;i<3000000;i++){ t+=o.m(); } t",
    )?;
    timed(
        "member read fn 3M",
        "let o={m:function(){return 1}}; let t=0; for (let i=0;i<3000000;i++){ if(o.m) t+=1; } t",
    )?;
    timed(
        "call 1arg 3M",
        "function f(a){return a} let t=0; for (let i=0;i<3000000;i++){ t+=f(1); } t",
    )?;
    timed(
        "call 3arg 3M",
        "function f(a,b,c){return a+b+c} let t=0; for (let i=0;i<3000000;i++){ t+=f(1,2,3); } t",
    )?;
    timed(
        "new empty 1M",
        "function C(){} let t=0; for (let i=0;i<1000000;i++){ new C(); t+=1; } t",
    )?;
    timed(
        "objlit empty 1M",
        "let t=0; for (let i=0;i<1000000;i++){ let o={}; t+=1; } t",
    )?;
    timed(
        "objlit 2prop 1M",
        "let t=0; for (let i=0;i<1000000;i++){ let o={a:1,b:2}; t+=o.a; } t",
    )?;
    timed(
        "this read 3M",
        "let o={v:1,m:function(){return this.v}}; let t=0; for (let i=0;i<3000000;i++){ t+=o.m(); } t",
    )?;
    Ok(())
}

#[test]
fn perf_probe_bench_files() -> TestResult {
    let dir = std::env::var("RSQJS_PROBE_DIR").unwrap_or_default();
    if dir.is_empty() {
        println!("RSQJS_PROBE_DIR not set; skipping file probes");
        return Ok(());
    }
    for name in [
        "method_this",
        "constructor_prototypes",
        "compound_assignment",
        "update_expressions",
        "in_operator",
    ] {
        let path = format!("{dir}/{name}.js");
        let source = std::fs::read_to_string(&path)?;
        timed(name, &source)?;
    }
    Ok(())
}
