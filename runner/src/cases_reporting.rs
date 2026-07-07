#[path = "cases_benchmark.rs"]
mod cases_benchmark;
#[path = "cases_quickjs.rs"]
mod cases_quickjs;

pub use cases_benchmark::benchmark_cases;
pub use cases_quickjs::quickjs_differential_cases;
