use alloc::rc::Rc;
use core::{fmt, time::Duration};
#[cfg(feature = "std")]
use std::time::Instant;

use crate::{
    error::{Error, Result},
    runtime::Context,
};

type ClockReader = dyn Fn() -> Duration;
type WallClockReader = dyn Fn() -> i128;

#[derive(Clone)]
pub(super) struct PerformanceClock {
    read: Rc<ClockReader>,
    origin: Duration,
    last_elapsed: Duration,
}

impl PerformanceClock {
    #[cfg(feature = "std")]
    pub(super) fn system() -> Self {
        let source_origin = Instant::now();
        Self::from_reader(move || source_origin.elapsed())
    }

    #[cfg(not(feature = "std"))]
    pub(super) fn system() -> Self {
        Self::from_reader(|| Duration::ZERO)
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

#[derive(Clone)]
pub(super) struct WallClock {
    read: Option<Rc<WallClockReader>>,
}

impl WallClock {
    #[cfg(feature = "std")]
    pub(super) fn system() -> Self {
        Self::from_reader(system_unix_time_nanos)
    }

    #[cfg(not(feature = "std"))]
    pub(super) const fn system() -> Self {
        Self { read: None }
    }

    pub(super) fn from_reader<F>(read: F) -> Self
    where
        F: Fn() -> i128 + 'static,
    {
        Self {
            read: Some(Rc::new(read)),
        }
    }

    fn unix_time_nanos(&self) -> Result<i128> {
        self.read.as_ref().map_or_else(
            || {
                Err(Error::runtime(
                    "wall clock is unavailable; provide an embedder clock source",
                ))
            },
            |read| Ok(read()),
        )
    }
}

impl fmt::Debug for WallClock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WallClock")
            .field("available", &self.read.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "std")]
fn system_unix_time_nanos() -> i128 {
    use std::time::{SystemTime, UNIX_EPOCH};

    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i128::try_from(duration.as_nanos()).unwrap_or(i128::MAX),
        Err(error) => i128::try_from(error.duration().as_nanos())
            .unwrap_or(i128::MAX)
            .saturating_neg(),
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
    /// Creates a context whose VM-local `performance.now()` uses `read` as its
    /// monotonic source. The first reading becomes this context's zero point.
    /// Later source regressions are clamped so JavaScript observes a
    /// non-decreasing value.
    #[must_use]
    pub fn with_monotonic_clock<F>(limits: crate::runtime::limits::RuntimeLimits, read: F) -> Self
    where
        F: Fn() -> Duration + 'static,
    {
        Self::with_optimization_and_monotonic_clock(
            limits,
            crate::runtime::OptimizationMode::Enabled,
            read,
        )
    }

    /// Creates a configured context with an embedder-provided monotonic clock.
    #[must_use]
    pub fn with_optimization_and_monotonic_clock<F>(
        limits: crate::runtime::limits::RuntimeLimits,
        mode: crate::runtime::OptimizationMode,
        read: F,
    ) -> Self
    where
        F: Fn() -> Duration + 'static,
    {
        Self::with_clocks(
            limits,
            mode,
            PerformanceClock::from_reader(read),
            WallClock::system(),
        )
    }

    /// Creates a context with embedder-provided monotonic and Unix wall clocks.
    #[must_use]
    pub fn with_clock_sources<M, W>(
        limits: crate::runtime::limits::RuntimeLimits,
        monotonic: M,
        unix_nanos: W,
    ) -> Self
    where
        M: Fn() -> Duration + 'static,
        W: Fn() -> i128 + 'static,
    {
        Self::with_optimization_and_clock_sources(
            limits,
            crate::runtime::OptimizationMode::Enabled,
            monotonic,
            unix_nanos,
        )
    }

    /// Creates a configured context with embedder-provided clock sources.
    #[must_use]
    pub fn with_optimization_and_clock_sources<M, W>(
        limits: crate::runtime::limits::RuntimeLimits,
        mode: crate::runtime::OptimizationMode,
        monotonic: M,
        unix_nanos: W,
    ) -> Self
    where
        M: Fn() -> Duration + 'static,
        W: Fn() -> i128 + 'static,
    {
        Self::with_clocks(
            limits,
            mode,
            PerformanceClock::from_reader(monotonic),
            WallClock::from_reader(unix_nanos),
        )
    }

    pub(in crate::runtime) fn performance_now_millis(&mut self) -> f64 {
        self.performance_clock.now_millis()
    }

    pub(in crate::runtime) fn wall_time_unix_nanos(&self) -> Result<i128> {
        self.wall_clock.unix_time_nanos()
    }
}
