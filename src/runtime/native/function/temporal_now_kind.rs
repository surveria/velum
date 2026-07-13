use super::TemporalFunctionKind;

impl TemporalFunctionKind {
    pub(in crate::runtime::native) const fn temporal_now_index(self) -> usize {
        match self {
            Self::NowInstant => 226,
            Self::NowTimeZoneId => 227,
            Self::NowZonedDateTimeIso => 228,
            Self::NowPlainDateTimeIso => 229,
            Self::NowPlainDateIso => 230,
            Self::NowPlainTimeIso => 231,
            _ => 46,
        }
    }

    pub(in crate::runtime::native) const fn temporal_now_name(self) -> &'static str {
        match self {
            Self::NowInstant => "instant",
            Self::NowTimeZoneId => "timeZoneId",
            Self::NowZonedDateTimeIso => "zonedDateTimeISO",
            Self::NowPlainDateTimeIso => "plainDateTimeISO",
            Self::NowPlainDateIso => "plainDateISO",
            Self::NowPlainTimeIso => "plainTimeISO",
            _ => "Instant",
        }
    }

    pub(in crate::runtime::native) const fn is_temporal_now(self) -> bool {
        matches!(
            self,
            Self::NowInstant
                | Self::NowTimeZoneId
                | Self::NowZonedDateTimeIso
                | Self::NowPlainDateTimeIso
                | Self::NowPlainDateIso
                | Self::NowPlainTimeIso
        )
    }
}
