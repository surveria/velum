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
    PlainDateTimeFrom,
    PlainDateTimeCompare,
    PlainDateTimePrototypeYear,
    PlainDateTimePrototypeMonth,
    PlainDateTimePrototypeMonthCode,
    PlainDateTimePrototypeDay,
    PlainDateTimePrototypeHour,
    PlainDateTimePrototypeMinute,
    PlainDateTimePrototypeSecond,
    PlainDateTimePrototypeMillisecond,
    PlainDateTimePrototypeMicrosecond,
    PlainDateTimePrototypeNanosecond,
    PlainDateTimePrototypeCalendarId,
    PlainDateTimePrototypeEra,
    PlainDateTimePrototypeEraYear,
    PlainDateTimePrototypeDayOfWeek,
    PlainDateTimePrototypeDayOfYear,
    PlainDateTimePrototypeWeekOfYear,
    PlainDateTimePrototypeYearOfWeek,
    PlainDateTimePrototypeDaysInWeek,
    PlainDateTimePrototypeDaysInMonth,
    PlainDateTimePrototypeDaysInYear,
    PlainDateTimePrototypeMonthsInYear,
    PlainDateTimePrototypeInLeapYear,
    PlainDateTimePrototypeWith,
    PlainDateTimePrototypeWithPlainTime,
    PlainDateTimePrototypeWithCalendar,
    PlainDateTimePrototypeAdd,
    PlainDateTimePrototypeSubtract,
    PlainDateTimePrototypeUntil,
    PlainDateTimePrototypeSince,
    PlainDateTimePrototypeRound,
    PlainDateTimePrototypeEquals,
    PlainDateTimePrototypeToString,
    PlainDateTimePrototypeToLocaleString,
    PlainDateTimePrototypeToJson,
    PlainDateTimePrototypeToZonedDateTime,
    PlainDateTimePrototypeToPlainDate,
    PlainDateTimePrototypeToPlainTime,
    PlainDateTimePrototypeValueOf,
    PlainTimeFrom,
    PlainTimeCompare,
    PlainTimePrototypeHour,
    PlainTimePrototypeMinute,
    PlainTimePrototypeSecond,
    PlainTimePrototypeMillisecond,
    PlainTimePrototypeMicrosecond,
    PlainTimePrototypeNanosecond,
    PlainTimePrototypeWith,
    PlainTimePrototypeAdd,
    PlainTimePrototypeSubtract,
    PlainTimePrototypeUntil,
    PlainTimePrototypeSince,
    PlainTimePrototypeRound,
    PlainTimePrototypeEquals,
    PlainTimePrototypeToString,
    PlainTimePrototypeToLocaleString,
    PlainTimePrototypeToJson,
    PlainTimePrototypeValueOf,
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
            _ => self.plain_date_time_index(),
        }
    }

    const fn plain_date_time_index(self) -> usize {
        match self {
            Self::PlainDateTimeFrom => 73,
            Self::PlainDateTimeCompare => 74,
            Self::PlainDateTimePrototypeYear => 75,
            Self::PlainDateTimePrototypeMonth => 76,
            Self::PlainDateTimePrototypeMonthCode => 77,
            Self::PlainDateTimePrototypeDay => 78,
            Self::PlainDateTimePrototypeHour => 79,
            Self::PlainDateTimePrototypeMinute => 80,
            Self::PlainDateTimePrototypeSecond => 81,
            Self::PlainDateTimePrototypeMillisecond => 82,
            Self::PlainDateTimePrototypeMicrosecond => 83,
            Self::PlainDateTimePrototypeNanosecond => 84,
            Self::PlainDateTimePrototypeCalendarId => 85,
            Self::PlainDateTimePrototypeEra => 86,
            Self::PlainDateTimePrototypeEraYear => 87,
            Self::PlainDateTimePrototypeDayOfWeek => 88,
            Self::PlainDateTimePrototypeDayOfYear => 89,
            Self::PlainDateTimePrototypeWeekOfYear => 90,
            Self::PlainDateTimePrototypeYearOfWeek => 91,
            Self::PlainDateTimePrototypeDaysInWeek => 92,
            Self::PlainDateTimePrototypeDaysInMonth => 93,
            Self::PlainDateTimePrototypeDaysInYear => 94,
            Self::PlainDateTimePrototypeMonthsInYear => 95,
            Self::PlainDateTimePrototypeInLeapYear => 96,
            Self::PlainDateTimePrototypeWith => 97,
            Self::PlainDateTimePrototypeWithPlainTime => 98,
            Self::PlainDateTimePrototypeWithCalendar => 99,
            Self::PlainDateTimePrototypeAdd => 100,
            Self::PlainDateTimePrototypeSubtract => 101,
            Self::PlainDateTimePrototypeUntil => 102,
            Self::PlainDateTimePrototypeSince => 103,
            Self::PlainDateTimePrototypeRound => 104,
            Self::PlainDateTimePrototypeEquals => 105,
            Self::PlainDateTimePrototypeToString => 106,
            Self::PlainDateTimePrototypeToLocaleString => 107,
            Self::PlainDateTimePrototypeToJson => 108,
            Self::PlainDateTimePrototypeToZonedDateTime => 109,
            Self::PlainDateTimePrototypeToPlainDate => 110,
            Self::PlainDateTimePrototypeToPlainTime => 111,
            Self::PlainDateTimePrototypeValueOf => 112,
            _ => self.plain_time_index(),
        }
    }

    const fn plain_time_index(self) -> usize {
        match self {
            Self::PlainTimeFrom => 113,
            Self::PlainTimeCompare => 114,
            Self::PlainTimePrototypeHour => 115,
            Self::PlainTimePrototypeMinute => 116,
            Self::PlainTimePrototypeSecond => 117,
            Self::PlainTimePrototypeMillisecond => 118,
            Self::PlainTimePrototypeMicrosecond => 119,
            Self::PlainTimePrototypeNanosecond => 120,
            Self::PlainTimePrototypeWith => 121,
            Self::PlainTimePrototypeAdd => 122,
            Self::PlainTimePrototypeSubtract => 123,
            Self::PlainTimePrototypeUntil => 124,
            Self::PlainTimePrototypeSince => 125,
            Self::PlainTimePrototypeRound => 126,
            Self::PlainTimePrototypeEquals => 127,
            Self::PlainTimePrototypeToString => 128,
            Self::PlainTimePrototypeToLocaleString => 129,
            Self::PlainTimePrototypeToJson => 130,
            Self::PlainTimePrototypeValueOf => 131,
            _ => 47,
        }
    }

    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Compare
            | Self::PlainDateCompare
            | Self::PlainDateTimeCompare
            | Self::PlainTimeCompare
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
            | Self::PlainDatePrototypeToZonedDateTime
            | Self::PlainDateTimeFrom
            | Self::PlainDateTimePrototypeWith
            | Self::PlainDateTimePrototypeWithCalendar
            | Self::PlainDateTimePrototypeAdd
            | Self::PlainDateTimePrototypeSubtract
            | Self::PlainDateTimePrototypeUntil
            | Self::PlainDateTimePrototypeSince
            | Self::PlainDateTimePrototypeRound
            | Self::PlainDateTimePrototypeEquals
            | Self::PlainDateTimePrototypeToZonedDateTime
            | Self::PlainTimeFrom
            | Self::PlainTimePrototypeWith
            | Self::PlainTimePrototypeAdd
            | Self::PlainTimePrototypeSubtract
            | Self::PlainTimePrototypeUntil
            | Self::PlainTimePrototypeSince
            | Self::PlainTimePrototypeRound
            | Self::PlainTimePrototypeEquals
            | Self::ZonedDateTimeFrom
            | Self::PrototypeWith
            | Self::PrototypeAdd
            | Self::PrototypeSubtract
            | Self::PrototypeTotal => 1.0,
            _ => 0.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Constructor => TEMPORAL_DURATION_NAME,
            Self::From
            | Self::PlainDateFrom
            | Self::PlainDateTimeFrom
            | Self::PlainTimeFrom
            | Self::ZonedDateTimeFrom => "from",
            Self::Compare
            | Self::PlainDateCompare
            | Self::PlainDateTimeCompare
            | Self::PlainTimeCompare => "compare",
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
            Self::PrototypeWith
            | Self::PlainDatePrototypeWith
            | Self::PlainDateTimePrototypeWith
            | Self::PlainTimePrototypeWith => "with",
            Self::PrototypeNegated => "negated",
            Self::PrototypeAbs => "abs",
            Self::PrototypeAdd
            | Self::PlainDatePrototypeAdd
            | Self::PlainDateTimePrototypeAdd
            | Self::PlainTimePrototypeAdd => "add",
            Self::PrototypeSubtract
            | Self::PlainDatePrototypeSubtract
            | Self::PlainDateTimePrototypeSubtract
            | Self::PlainTimePrototypeSubtract => "subtract",
            Self::PrototypeRound
            | Self::PlainDateTimePrototypeRound
            | Self::PlainTimePrototypeRound => "round",
            Self::PrototypeTotal => "total",
            Self::PrototypeToString
            | Self::PlainDatePrototypeToString
            | Self::PlainDateTimePrototypeToString
            | Self::PlainTimePrototypeToString
            | Self::ZonedDateTimePrototypeToString => "toString",
            Self::PrototypeToJson
            | Self::PlainDatePrototypeToJson
            | Self::PlainDateTimePrototypeToJson
            | Self::PlainTimePrototypeToJson
            | Self::ZonedDateTimePrototypeToJson => "toJSON",
            Self::PrototypeToLocaleString
            | Self::PlainDatePrototypeToLocaleString
            | Self::PlainDateTimePrototypeToLocaleString
            | Self::PlainTimePrototypeToLocaleString => "toLocaleString",
            Self::PrototypeValueOf
            | Self::PlainDatePrototypeValueOf
            | Self::PlainDateTimePrototypeValueOf
            | Self::PlainTimePrototypeValueOf
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
            _ => self.plain_date_time_name(),
        }
    }

    const fn plain_date_time_name(self) -> &'static str {
        match self {
            Self::PlainDateTimePrototypeYear => "get year",
            Self::PlainDateTimePrototypeMonth => "get month",
            Self::PlainDateTimePrototypeMonthCode => "get monthCode",
            Self::PlainDateTimePrototypeDay => "get day",
            Self::PlainDateTimePrototypeHour => "get hour",
            Self::PlainDateTimePrototypeMinute => "get minute",
            Self::PlainDateTimePrototypeSecond => "get second",
            Self::PlainDateTimePrototypeMillisecond => "get millisecond",
            Self::PlainDateTimePrototypeMicrosecond => "get microsecond",
            Self::PlainDateTimePrototypeNanosecond => "get nanosecond",
            Self::PlainDateTimePrototypeCalendarId => "get calendarId",
            Self::PlainDateTimePrototypeEra => "get era",
            Self::PlainDateTimePrototypeEraYear => "get eraYear",
            Self::PlainDateTimePrototypeDayOfWeek => "get dayOfWeek",
            Self::PlainDateTimePrototypeDayOfYear => "get dayOfYear",
            Self::PlainDateTimePrototypeWeekOfYear => "get weekOfYear",
            Self::PlainDateTimePrototypeYearOfWeek => "get yearOfWeek",
            Self::PlainDateTimePrototypeDaysInWeek => "get daysInWeek",
            Self::PlainDateTimePrototypeDaysInMonth => "get daysInMonth",
            Self::PlainDateTimePrototypeDaysInYear => "get daysInYear",
            Self::PlainDateTimePrototypeMonthsInYear => "get monthsInYear",
            Self::PlainDateTimePrototypeInLeapYear => "get inLeapYear",
            Self::PlainDateTimePrototypeWithPlainTime => "withPlainTime",
            Self::PlainDateTimePrototypeWithCalendar => "withCalendar",
            Self::PlainDateTimePrototypeUntil => "until",
            Self::PlainDateTimePrototypeSince => "since",
            Self::PlainDateTimePrototypeEquals => "equals",
            Self::PlainDateTimePrototypeToZonedDateTime => "toZonedDateTime",
            Self::PlainDateTimePrototypeToPlainDate => "toPlainDate",
            Self::PlainDateTimePrototypeToPlainTime => "toPlainTime",
            _ => self.plain_time_name(),
        }
    }

    const fn plain_time_name(self) -> &'static str {
        match self {
            Self::PlainTimePrototypeHour => "get hour",
            Self::PlainTimePrototypeMinute => "get minute",
            Self::PlainTimePrototypeSecond => "get second",
            Self::PlainTimePrototypeMillisecond => "get millisecond",
            Self::PlainTimePrototypeMicrosecond => "get microsecond",
            Self::PlainTimePrototypeNanosecond => "get nanosecond",
            Self::PlainTimePrototypeUntil => "until",
            Self::PlainTimePrototypeSince => "since",
            Self::PlainTimePrototypeEquals => "equals",
            _ => "PlainTime",
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

    pub(in crate::runtime::native) const fn is_plain_date_time(self) -> bool {
        matches!(
            self,
            Self::PlainDateTimeConstructor
                | Self::PlainDateTimeFrom
                | Self::PlainDateTimeCompare
                | Self::PlainDateTimePrototypeYear
                | Self::PlainDateTimePrototypeMonth
                | Self::PlainDateTimePrototypeMonthCode
                | Self::PlainDateTimePrototypeDay
                | Self::PlainDateTimePrototypeHour
                | Self::PlainDateTimePrototypeMinute
                | Self::PlainDateTimePrototypeSecond
                | Self::PlainDateTimePrototypeMillisecond
                | Self::PlainDateTimePrototypeMicrosecond
                | Self::PlainDateTimePrototypeNanosecond
                | Self::PlainDateTimePrototypeCalendarId
                | Self::PlainDateTimePrototypeEra
                | Self::PlainDateTimePrototypeEraYear
                | Self::PlainDateTimePrototypeDayOfWeek
                | Self::PlainDateTimePrototypeDayOfYear
                | Self::PlainDateTimePrototypeWeekOfYear
                | Self::PlainDateTimePrototypeYearOfWeek
                | Self::PlainDateTimePrototypeDaysInWeek
                | Self::PlainDateTimePrototypeDaysInMonth
                | Self::PlainDateTimePrototypeDaysInYear
                | Self::PlainDateTimePrototypeMonthsInYear
                | Self::PlainDateTimePrototypeInLeapYear
                | Self::PlainDateTimePrototypeWith
                | Self::PlainDateTimePrototypeWithPlainTime
                | Self::PlainDateTimePrototypeWithCalendar
                | Self::PlainDateTimePrototypeAdd
                | Self::PlainDateTimePrototypeSubtract
                | Self::PlainDateTimePrototypeUntil
                | Self::PlainDateTimePrototypeSince
                | Self::PlainDateTimePrototypeRound
                | Self::PlainDateTimePrototypeEquals
                | Self::PlainDateTimePrototypeToString
                | Self::PlainDateTimePrototypeToLocaleString
                | Self::PlainDateTimePrototypeToJson
                | Self::PlainDateTimePrototypeToZonedDateTime
                | Self::PlainDateTimePrototypeToPlainDate
                | Self::PlainDateTimePrototypeToPlainTime
                | Self::PlainDateTimePrototypeValueOf
        )
    }

    pub(in crate::runtime::native) const fn is_plain_time(self) -> bool {
        matches!(
            self,
            Self::PlainTimeConstructor
                | Self::PlainTimeFrom
                | Self::PlainTimeCompare
                | Self::PlainTimePrototypeHour
                | Self::PlainTimePrototypeMinute
                | Self::PlainTimePrototypeSecond
                | Self::PlainTimePrototypeMillisecond
                | Self::PlainTimePrototypeMicrosecond
                | Self::PlainTimePrototypeNanosecond
                | Self::PlainTimePrototypeWith
                | Self::PlainTimePrototypeAdd
                | Self::PlainTimePrototypeSubtract
                | Self::PlainTimePrototypeUntil
                | Self::PlainTimePrototypeSince
                | Self::PlainTimePrototypeRound
                | Self::PlainTimePrototypeEquals
                | Self::PlainTimePrototypeToString
                | Self::PlainTimePrototypeToLocaleString
                | Self::PlainTimePrototypeToJson
                | Self::PlainTimePrototypeValueOf
        )
    }
}
