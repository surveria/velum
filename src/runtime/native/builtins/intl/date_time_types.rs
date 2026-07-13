#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum DateTimeInputKind {
    Instant,
    PlainDate,
    PlainDateTime,
    PlainMonthDay,
    PlainTime,
    PlainYearMonth,
    ZonedDateTime,
    LegacyDate,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct DateTimeInput {
    pub(super) kind: DateTimeInputKind,
    pub(super) year: Option<i32>,
    pub(super) era: Option<String>,
    pub(super) month: Option<u8>,
    pub(super) month_code: Option<String>,
    pub(super) day: Option<u8>,
    pub(super) weekday: Option<u16>,
    pub(super) hour: Option<u8>,
    pub(super) minute: Option<u8>,
    pub(super) second: Option<u8>,
    pub(super) millisecond: Option<u16>,
    pub(super) time_zone: Option<String>,
    pub(super) offset: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct FormatPart {
    pub(super) kind: &'static str,
    pub(super) value: String,
}
