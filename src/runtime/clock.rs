use std::{fmt, rc::Rc, time::Duration, time::Instant};

use crate::runtime::Context;

type ClockReader = dyn Fn() -> Duration;

#[derive(Clone)]
pub(super) struct PerformanceClock {
    read: Rc<ClockReader>,
    origin: Duration,
    last_elapsed: Duration,
}

impl PerformanceClock {
    pub(super) fn system() -> Self {
        let source_origin = Instant::now();
        Self::from_reader(move || source_origin.elapsed())
    }

    pub(super) fn from_reader<F>(read: F) -> Self
    where
        F: Fn() -> Duration + 'static,
    {
        let read: Rc<ClockReader> = Rc::new(read);
        let origin = read();
        Self {
            read,
            origin,
            last_elapsed: Duration::ZERO,
        }
    }

    fn now_millis(&mut self) -> f64 {
        let reading = (self.read)();
        let elapsed = reading
            .checked_sub(self.origin)
            .map_or(Duration::ZERO, |value| value);
        if elapsed > self.last_elapsed {
            self.last_elapsed = elapsed;
        }
        self.last_elapsed.as_secs_f64() * 1_000.0
    }
}

impl fmt::Debug for PerformanceClock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PerformanceClock")
            .field("origin", &self.origin)
            .field("last_elapsed", &self.last_elapsed)
            .finish_non_exhaustive()
    }
}

impl Context {
    pub(in crate::runtime) fn performance_now_millis(&mut self) -> f64 {
        self.performance_clock.now_millis()
    }
}
