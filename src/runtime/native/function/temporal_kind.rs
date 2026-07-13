pub(in crate::runtime) const TEMPORAL_NAME: &str = "Temporal";
pub(in crate::runtime) const TEMPORAL_DURATION_NAME: &str = "Duration";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum TemporalFunctionKind {
    Constructor,
    From,
    Compare,
    PrototypeYears,
    PrototypeMonths,
    PrototypeWeeks,
    PrototypeDays,
    PrototypeHours,
    PrototypeMinutes,
    PrototypeSeconds,
    PrototypeMilliseconds,
    PrototypeMicroseconds,
    PrototypeNanoseconds,
    PrototypeSign,
    PrototypeBlank,
    PrototypeWith,
    PrototypeNegated,
    PrototypeAbs,
    PrototypeAdd,
    PrototypeSubtract,
    PrototypeRound,
    PrototypeTotal,
    PrototypeToString,
    PrototypeToJson,
    PrototypeToLocaleString,
    PrototypeValueOf,
    PlainDateConstructor,
    PlainDateFrom,
    PlainDatePrototypeYear,
    PlainDatePrototypeMonth,
    PlainDatePrototypeDay,
    PlainDatePrototypeCalendarId,
    PlainDatePrototypeToString,
    PlainDatePrototypeToJson,
    PlainDatePrototypeValueOf,
    ZonedDateTimeConstructor,
    ZonedDateTimeFrom,
    ZonedDateTimePrototypeEpochNanoseconds,
    ZonedDateTimePrototypeTimeZoneId,
    ZonedDateTimePrototypeCalendarId,
    ZonedDateTimePrototypeToString,
    ZonedDateTimePrototypeToJson,
    ZonedDateTimePrototypeValueOf,
    PlainDateTimeConstructor,
    PlainMonthDayConstructor,
    PlainYearMonthConstructor,
    InstantConstructor,
    PlainTimeConstructor,
    PlainDateCompare,
    PlainDatePrototypeMonthCode,
    PlainDatePrototypeEra,
    PlainDatePrototypeEraYear,
    PlainDatePrototypeDayOfWeek,
    PlainDatePrototypeDayOfYear,
    PlainDatePrototypeWeekOfYear,
    PlainDatePrototypeYearOfWeek,
    PlainDatePrototypeDaysInWeek,
    PlainDatePrototypeDaysInMonth,
    PlainDatePrototypeDaysInYear,
    PlainDatePrototypeMonthsInYear,
    PlainDatePrototypeInLeapYear,
    PlainDatePrototypeWith,
    PlainDatePrototypeWithCalendar,
    PlainDatePrototypeAdd,
    PlainDatePrototypeSubtract,
    PlainDatePrototypeUntil,
    PlainDatePrototypeSince,
    PlainDatePrototypeEquals,
    PlainDatePrototypeToPlainDateTime,
    PlainDatePrototypeToZonedDateTime,
    PlainDatePrototypeToPlainYearMonth,
    PlainDatePrototypeToPlainMonthDay,
    PlainDatePrototypeToLocaleString,
}

impl TemporalFunctionKind {
    pub(in crate::runtime::native) const fn index(self) -> usize {
        match self {
            Self::Constructor => 0,
            Self::From => 1,
            Self::Compare => 2,
            Self::PrototypeYears => 3,
            Self::PrototypeMonths => 4,
            Self::PrototypeWeeks => 5,
            Self::PrototypeDays => 6,
            Self::PrototypeHours => 7,
            Self::PrototypeMinutes => 8,
            Self::PrototypeSeconds => 9,
            Self::PrototypeMilliseconds => 10,
            Self::PrototypeMicroseconds => 11,
            Self::PrototypeNanoseconds => 12,
            Self::PrototypeSign => 13,
            Self::PrototypeBlank => 14,
            Self::PrototypeWith => 15,
            Self::PrototypeNegated => 16,
            Self::PrototypeAbs => 17,
            Self::PrototypeAdd => 18,
            Self::PrototypeSubtract => 19,
            Self::PrototypeRound => 20,
            Self::PrototypeTotal => 21,
            Self::PrototypeToString => 22,
            Self::PrototypeToJson => 23,
            Self::PrototypeToLocaleString => 24,
            Self::PrototypeValueOf => 25,
            Self::PlainDateConstructor => 26,
            Self::PlainDateFrom => 27,
            Self::PlainDatePrototypeYear => 28,
            Self::PlainDatePrototypeMonth => 29,
            Self::PlainDatePrototypeDay => 30,
            Self::PlainDatePrototypeCalendarId => 31,
            Self::PlainDatePrototypeToString => 32,
            Self::PlainDatePrototypeToJson => 33,
            Self::PlainDatePrototypeValueOf => 34,
            Self::ZonedDateTimeConstructor => 35,
            Self::ZonedDateTimeFrom => 36,
            Self::ZonedDateTimePrototypeEpochNanoseconds => 37,
            Self::ZonedDateTimePrototypeTimeZoneId => 38,
            Self::ZonedDateTimePrototypeCalendarId => 39,
            Self::ZonedDateTimePrototypeToString => 40,
            Self::ZonedDateTimePrototypeToJson => 41,
            Self::ZonedDateTimePrototypeValueOf => 42,
            Self::PlainDateTimeConstructor => 43,
            Self::PlainMonthDayConstructor => 44,
            Self::PlainYearMonthConstructor => 45,
            Self::InstantConstructor => 46,
            Self::PlainTimeConstructor => 47,
            Self::PlainDateCompare => 48,
            Self::PlainDatePrototypeMonthCode => 49,
            Self::PlainDatePrototypeEra => 50,
            Self::PlainDatePrototypeEraYear => 51,
            Self::PlainDatePrototypeDayOfWeek => 52,
            Self::PlainDatePrototypeDayOfYear => 53,
            Self::PlainDatePrototypeWeekOfYear => 54,
            Self::PlainDatePrototypeYearOfWeek => 55,
            Self::PlainDatePrototypeDaysInWeek => 56,
            Self::PlainDatePrototypeDaysInMonth => 57,
            Self::PlainDatePrototypeDaysInYear => 58,
            Self::PlainDatePrototypeMonthsInYear => 59,
            Self::PlainDatePrototypeInLeapYear => 60,
            Self::PlainDatePrototypeWith => 61,
            Self::PlainDatePrototypeWithCalendar => 62,
            Self::PlainDatePrototypeAdd => 63,
            Self::PlainDatePrototypeSubtract => 64,
            Self::PlainDatePrototypeUntil => 65,
            Self::PlainDatePrototypeSince => 66,
            Self::PlainDatePrototypeEquals => 67,
            Self::PlainDatePrototypeToPlainDateTime => 68,
            Self::PlainDatePrototypeToZonedDateTime => 69,
            Self::PlainDatePrototypeToPlainYearMonth => 70,
            Self::PlainDatePrototypeToPlainMonthDay => 71,
            Self::PlainDatePrototypeToLocaleString => 72,
        }
    }

    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Compare
            | Self::PlainDateCompare
            | Self::ZonedDateTimeConstructor
            | Self::PlainMonthDayConstructor
            | Self::PlainYearMonthConstructor => 2.0,
            Self::PlainDateConstructor | Self::PlainDateTimeConstructor => 3.0,
            Self::From
            | Self::InstantConstructor
            | Self::PlainDateFrom
            | Self::PlainDatePrototypeWith
            | Self::PlainDatePrototypeWithCalendar
            | Self::PlainDatePrototypeAdd
            | Self::PlainDatePrototypeSubtract
            | Self::PlainDatePrototypeUntil
            | Self::PlainDatePrototypeSince
            | Self::PlainDatePrototypeEquals
            | Self::PlainDatePrototypeToPlainDateTime
            | Self::PlainDatePrototypeToZonedDateTime
            | Self::ZonedDateTimeFrom
            | Self::PrototypeWith
            | Self::PrototypeAdd
            | Self::PrototypeSubtract
            | Self::PrototypeRound
            | Self::PrototypeTotal => 1.0,
            _ => 0.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Constructor => TEMPORAL_DURATION_NAME,
            Self::From | Self::PlainDateFrom | Self::ZonedDateTimeFrom => "from",
            Self::Compare | Self::PlainDateCompare => "compare",
            Self::PrototypeYears => "get years",
            Self::PrototypeMonths => "get months",
            Self::PrototypeWeeks => "get weeks",
            Self::PrototypeDays => "get days",
            Self::PrototypeHours => "get hours",
            Self::PrototypeMinutes => "get minutes",
            Self::PrototypeSeconds => "get seconds",
            Self::PrototypeMilliseconds => "get milliseconds",
            Self::PrototypeMicroseconds => "get microseconds",
            Self::PrototypeNanoseconds => "get nanoseconds",
            Self::PrototypeSign => "get sign",
            Self::PrototypeBlank => "get blank",
            Self::PrototypeWith | Self::PlainDatePrototypeWith => "with",
            Self::PrototypeNegated => "negated",
            Self::PrototypeAbs => "abs",
            Self::PrototypeAdd | Self::PlainDatePrototypeAdd => "add",
            Self::PrototypeSubtract | Self::PlainDatePrototypeSubtract => "subtract",
            Self::PrototypeRound => "round",
            Self::PrototypeTotal => "total",
            Self::PrototypeToString
            | Self::PlainDatePrototypeToString
            | Self::ZonedDateTimePrototypeToString => "toString",
            Self::PrototypeToJson
            | Self::PlainDatePrototypeToJson
            | Self::ZonedDateTimePrototypeToJson => "toJSON",
            Self::PrototypeToLocaleString | Self::PlainDatePrototypeToLocaleString => {
                "toLocaleString"
            }
            Self::PrototypeValueOf
            | Self::PlainDatePrototypeValueOf
            | Self::ZonedDateTimePrototypeValueOf => "valueOf",
            Self::PlainDateConstructor => "PlainDate",
            Self::PlainDatePrototypeYear => "get year",
            Self::PlainDatePrototypeMonth => "get month",
            Self::PlainDatePrototypeDay => "get day",
            Self::PlainDatePrototypeCalendarId | Self::ZonedDateTimePrototypeCalendarId => {
                "get calendarId"
            }
            Self::ZonedDateTimeConstructor => "ZonedDateTime",
            Self::ZonedDateTimePrototypeEpochNanoseconds => "get epochNanoseconds",
            Self::ZonedDateTimePrototypeTimeZoneId => "get timeZoneId",
            Self::PlainDateTimeConstructor => "PlainDateTime",
            Self::PlainMonthDayConstructor => "PlainMonthDay",
            Self::PlainYearMonthConstructor => "PlainYearMonth",
            Self::InstantConstructor => "Instant",
            Self::PlainTimeConstructor => "PlainTime",
            Self::PlainDatePrototypeMonthCode => "get monthCode",
            Self::PlainDatePrototypeEra => "get era",
            Self::PlainDatePrototypeEraYear => "get eraYear",
            Self::PlainDatePrototypeDayOfWeek => "get dayOfWeek",
            Self::PlainDatePrototypeDayOfYear => "get dayOfYear",
            Self::PlainDatePrototypeWeekOfYear => "get weekOfYear",
            Self::PlainDatePrototypeYearOfWeek => "get yearOfWeek",
            Self::PlainDatePrototypeDaysInWeek => "get daysInWeek",
            Self::PlainDatePrototypeDaysInMonth => "get daysInMonth",
            Self::PlainDatePrototypeDaysInYear => "get daysInYear",
            Self::PlainDatePrototypeMonthsInYear => "get monthsInYear",
            Self::PlainDatePrototypeInLeapYear => "get inLeapYear",
            Self::PlainDatePrototypeWithCalendar => "withCalendar",
            Self::PlainDatePrototypeUntil => "until",
            Self::PlainDatePrototypeSince => "since",
            Self::PlainDatePrototypeEquals => "equals",
            Self::PlainDatePrototypeToPlainDateTime => "toPlainDateTime",
            Self::PlainDatePrototypeToZonedDateTime => "toZonedDateTime",
            Self::PlainDatePrototypeToPlainYearMonth => "toPlainYearMonth",
            Self::PlainDatePrototypeToPlainMonthDay => "toPlainMonthDay",
        }
    }

    pub(in crate::runtime::native) const fn is_plain_date(self) -> bool {
        matches!(
            self,
            Self::PlainDateConstructor
                | Self::PlainDateFrom
                | Self::PlainDateCompare
                | Self::PlainDatePrototypeYear
                | Self::PlainDatePrototypeMonth
                | Self::PlainDatePrototypeMonthCode
                | Self::PlainDatePrototypeDay
                | Self::PlainDatePrototypeCalendarId
                | Self::PlainDatePrototypeEra
                | Self::PlainDatePrototypeEraYear
                | Self::PlainDatePrototypeDayOfWeek
                | Self::PlainDatePrototypeDayOfYear
                | Self::PlainDatePrototypeWeekOfYear
                | Self::PlainDatePrototypeYearOfWeek
                | Self::PlainDatePrototypeDaysInWeek
                | Self::PlainDatePrototypeDaysInMonth
                | Self::PlainDatePrototypeDaysInYear
                | Self::PlainDatePrototypeMonthsInYear
                | Self::PlainDatePrototypeInLeapYear
                | Self::PlainDatePrototypeWith
                | Self::PlainDatePrototypeWithCalendar
                | Self::PlainDatePrototypeAdd
                | Self::PlainDatePrototypeSubtract
                | Self::PlainDatePrototypeUntil
                | Self::PlainDatePrototypeSince
                | Self::PlainDatePrototypeEquals
                | Self::PlainDatePrototypeToPlainDateTime
                | Self::PlainDatePrototypeToZonedDateTime
                | Self::PlainDatePrototypeToPlainYearMonth
                | Self::PlainDatePrototypeToPlainMonthDay
                | Self::PlainDatePrototypeToString
                | Self::PlainDatePrototypeToJson
                | Self::PlainDatePrototypeToLocaleString
                | Self::PlainDatePrototypeValueOf
        )
    }
}
