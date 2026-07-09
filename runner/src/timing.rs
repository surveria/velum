use std::time::{Duration, Instant};

use crate::bench_measure;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Timed<T> {
    pub value: T,
    pub elapsed: Duration,
}

#[derive(Debug)]
pub struct RunTimer {
    start: Instant,
}

#[derive(Debug, Clone)]
pub struct MeasurementColumns {
    pub case_elapsed: String,
    pub rsqjs_measure: String,
    pub quickjs_measure: String,
}

#[derive(Debug, Clone)]
pub struct ReferenceColumns {
    pub eval: String,
    pub cv: String,
}

impl RunTimer {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

pub fn timed<T>(operation: impl FnOnce() -> T) -> Timed<T> {
    let timer = RunTimer::start();
    let value = operation();
    Timed {
        value,
        elapsed: timer.elapsed(),
    }
}

pub fn format_duration(duration: Duration) -> String {
    bench_measure::format_duration(duration)
}

impl MeasurementColumns {
    pub fn measured(
        case_elapsed: String,
        rsqjs_elapsed: Duration,
        quickjs_elapsed: Duration,
    ) -> Self {
        Self {
            case_elapsed,
            rsqjs_measure: format_duration(rsqjs_elapsed),
            quickjs_measure: format_duration(quickjs_elapsed),
        }
    }

    pub fn without_reference(case_elapsed: String, rsqjs_elapsed: Duration) -> Self {
        Self {
            case_elapsed,
            rsqjs_measure: format_duration(rsqjs_elapsed),
            quickjs_measure: "-".to_owned(),
        }
    }

    pub fn failed_with_reference(
        case_elapsed: String,
        rsqjs_measure: String,
        quickjs_elapsed: Duration,
    ) -> Self {
        Self {
            case_elapsed,
            rsqjs_measure,
            quickjs_measure: format_duration(quickjs_elapsed),
        }
    }

    pub fn not_measured(case_elapsed: String) -> Self {
        Self {
            case_elapsed,
            rsqjs_measure: "-".to_owned(),
            quickjs_measure: "-".to_owned(),
        }
    }
}

impl ReferenceColumns {
    pub const fn measured(eval: String, cv: String) -> Self {
        Self { eval, cv }
    }

    pub fn not_measured(label: &str) -> Self {
        Self {
            eval: label.to_owned(),
            cv: "-".to_owned(),
        }
    }
}
