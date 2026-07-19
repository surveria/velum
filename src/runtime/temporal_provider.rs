#[cfg(not(feature = "std"))]
use alloc::borrow::Cow;

use temporal_rs::provider::TimeZoneProvider;

#[cfg(not(feature = "std"))]
use timezone_provider::{
    TimeZoneProviderError,
    epoch_nanoseconds::EpochNanoseconds,
    provider::{
        CandidateEpochNanoseconds, EpochNanosecondsAndOffset, IsoDateTime, TimeZoneId,
        TransitionDirection, UtcOffsetSeconds,
    },
};

#[cfg(not(feature = "std"))]
#[derive(Debug)]
struct UtcTimeZoneProvider;

#[cfg(not(feature = "std"))]
static UTC_TIME_ZONE_PROVIDER: UtcTimeZoneProvider = UtcTimeZoneProvider;

#[cfg(feature = "std")]
pub fn time_zone_provider() -> &'static impl TimeZoneProvider {
    &*temporal_rs::provider::COMPILED_TZ_PROVIDER
}

#[cfg(not(feature = "std"))]
pub fn time_zone_provider() -> &'static impl TimeZoneProvider {
    &UTC_TIME_ZONE_PROVIDER
}

#[cfg(not(feature = "std"))]
impl TimeZoneProvider for UtcTimeZoneProvider {
    fn get(&self, identifier: &[u8]) -> Result<TimeZoneId, TimeZoneProviderError> {
        if identifier.eq_ignore_ascii_case(b"UTC")
            || identifier.eq_ignore_ascii_case(b"GMT")
            || identifier.eq_ignore_ascii_case(b"Etc/UTC")
            || identifier.eq_ignore_ascii_case(b"Etc/GMT")
        {
            return Ok(TimeZoneId::default());
        }
        Err(TimeZoneProviderError::Range(
            "no_std time zone provider supports only UTC",
        ))
    }

    fn identifier(&self, _id: TimeZoneId) -> Result<Cow<'_, str>, TimeZoneProviderError> {
        Ok(Cow::Borrowed("UTC"))
    }

    fn canonicalized(&self, _id: TimeZoneId) -> Result<TimeZoneId, TimeZoneProviderError> {
        Ok(TimeZoneId::default())
    }

    fn candidate_nanoseconds_for_local_epoch_nanoseconds(
        &self,
        _id: TimeZoneId,
        local_datetime: IsoDateTime,
    ) -> Result<CandidateEpochNanoseconds, TimeZoneProviderError> {
        Ok(CandidateEpochNanoseconds::One(EpochNanosecondsAndOffset {
            ns: local_datetime.as_nanoseconds(),
            offset: UtcOffsetSeconds(0),
        }))
    }

    fn transition_nanoseconds_for_utc_epoch_nanoseconds(
        &self,
        _id: TimeZoneId,
        _epoch_nanoseconds: i128,
    ) -> Result<UtcOffsetSeconds, TimeZoneProviderError> {
        Ok(UtcOffsetSeconds(0))
    }

    fn get_time_zone_transition(
        &self,
        _id: TimeZoneId,
        _epoch_nanoseconds: i128,
        _direction: TransitionDirection,
    ) -> Result<Option<EpochNanoseconds>, TimeZoneProviderError> {
        Ok(None)
    }
}
