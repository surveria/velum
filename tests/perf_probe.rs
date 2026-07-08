use std::time::Instant;

use rs_quickjs::{Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn timed(label: &str, source: &str) -> TestResult {
    let mut samples = Vec::new();
    let mut shown = String::new();
    for _ in 0..5 {
        let limits = RuntimeLimits {
            max_runtime_steps: usize::MAX,
            max_objects: 50_000_000,
            ..RuntimeLimits::default()
        };
        let runtime = Runtime::with_limits(limits);
        let mut context = runtime.context();
        let start = Instant::now();
        let value = context.eval(source)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        shown = match value {
            Value::Number(n) => n.to_string(),
            other => format!("{other:?}"),
        };
    }
    samples.sort_by(|a, b| a.total_cmp(b));
    let median = samples.get(2).copied().unwrap_or(0.0);
    println!("{label:<24} {median:>8.1}ms  result={shown}");
    Ok(())
}

#[test]
fn perf_probe() -> TestResult {
    timed("bare loop 3M", "let t=0; for (let i=0;i<3000000;i++){ t+=1; } t")?;
    timed("call0 body1 3M", "function f(){return 1} let t=0; for (let i=0;i<3000000;i++){ t+=f(); } t")?;
    timed("call3 body1 3M", "function f(a,b,c){return 1} let t=0; for (let i=0;i<3000000;i++){ t+=f(1,2,3); } t")?;
    timed("call0 extra3 3M", "function f(){return 1} let t=0; for (let i=0;i<3000000;i++){ t+=f(1,2,3); } t")?;
    timed("call1 bodyA 3M", "function f(a){return a} let t=0; for (let i=0;i<3000000;i++){ t+=f(1); } t")?;
    timed("method nothis 3M", "let o={m:function(){return 1}}; let t=0; for (let i=0;i<3000000;i++){ t+=o.m(); } t")?;
    timed("this read 3M", "let o={v:1,m:function(){return this.v}}; let t=0; for (let i=0;i<3000000;i++){ t+=o.m(); } t")?;
    timed("fn prop read 3M", "function f(){return 1} f.v=1; let t=0; for (let i=0;i<3000000;i++){ t+=f.v; } t")?;
    timed("nested prop 3M", "let o={p:{v:1}}; let t=0; for (let i=0;i<3000000;i++){ t+=o.p.v; } t")?;
    timed("new empty 1M", "function C(){} let t=0; for (let i=0;i<1000000;i++){ new C(); t+=1; } t")?;
    timed("closure read 3M", "function mk(){let c=1; return function(){return c}} let g=mk(); let t=0; for (let i=0;i<3000000;i++){ t+=g(); } t")?;
    timed("this via local 3M", "let o={v:1,m:function(){let s=this; return s.v}}; let t=0; for (let i=0;i<3000000;i++){ t+=o.m(); } t")?;
    timed("param prop 3M", "let o={v:1}; function f(s){return s.v} let t=0; for (let i=0;i<3000000;i++){ t+=f(o); } t")?;
    timed("this bare 3M", "let o={v:1,m:function(){return this===o?1:0}}; let t=0; for (let i=0;i<3000000;i++){ t+=o.m(); } t")?;
    Ok(())
}
