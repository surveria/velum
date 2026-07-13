use super::TemporalFunctionKind;

impl TemporalFunctionKind {
    pub(in crate::runtime::native) const fn zoned_date_time_index(self) -> usize {
        match self {
            Self::ZonedDateTimeCompare => 183,
            Self::ZonedDateTimePrototypeEpochMilliseconds => 184,
            Self::ZonedDateTimePrototypeYear => 185,
            Self::ZonedDateTimePrototypeMonth => 186,
            Self::ZonedDateTimePrototypeMonthCode => 187,
            Self::ZonedDateTimePrototypeDay => 188,
            Self::ZonedDateTimePrototypeHour => 189,
            Self::ZonedDateTimePrototypeMinute => 190,
            Self::ZonedDateTimePrototypeSecond => 191,
            Self::ZonedDateTimePrototypeMillisecond => 192,
            Self::ZonedDateTimePrototypeMicrosecond => 193,
            Self::ZonedDateTimePrototypeNanosecond => 194,
            Self::ZonedDateTimePrototypeEra => 195,
            Self::ZonedDateTimePrototypeEraYear => 196,
            Self::ZonedDateTimePrototypeDayOfWeek => 197,
            Self::ZonedDateTimePrototypeDayOfYear => 198,
            Self::ZonedDateTimePrototypeWeekOfYear => 199,
            Self::ZonedDateTimePrototypeYearOfWeek => 200,
            Self::ZonedDateTimePrototypeHoursInDay => 201,
            Self::ZonedDateTimePrototypeDaysInWeek => 202,
            Self::ZonedDateTimePrototypeDaysInMonth => 203,
            Self::ZonedDateTimePrototypeDaysInYear => 204,
            Self::ZonedDateTimePrototypeMonthsInYear => 205,
            Self::ZonedDateTimePrototypeInLeapYear => 206,
            Self::ZonedDateTimePrototypeOffset => 207,
            Self::ZonedDateTimePrototypeOffsetNanoseconds => 208,
            Self::ZonedDateTimePrototypeAdd => 209,
            Self::ZonedDateTimePrototypeSubtract => 210,
            Self::ZonedDateTimePrototypeWith => 211,
            Self::ZonedDateTimePrototypeUntil => 212,
            Self::ZonedDateTimePrototypeSince => 213,
            Self::ZonedDateTimePrototypeRound => 214,
            Self::ZonedDateTimePrototypeEquals => 215,
            Self::ZonedDateTimePrototypeStartOfDay => 216,
            Self::ZonedDateTimePrototypeGetTimeZoneTransition => 217,
            Self::ZonedDateTimePrototypeWithPlainTime => 218,
            Self::ZonedDateTimePrototypeWithTimeZone => 219,
            Self::ZonedDateTimePrototypeWithCalendar => 220,
            Self::ZonedDateTimePrototypeToInstant => 221,
            Self::ZonedDateTimePrototypeToPlainDate => 222,
            Self::ZonedDateTimePrototypeToPlainTime => 223,
            Self::ZonedDateTimePrototypeToPlainDateTime => 224,
            Self::ZonedDateTimePrototypeToLocaleString => 225,
            _ => 46,
        }
    }

    pub(in crate::runtime::native) const fn zoned_date_time_name(self) -> &'static str {
        match self {
            Self::ZonedDateTimeCompare => "compare",
            Self::ZonedDateTimePrototypeEpochMilliseconds => "get epochMilliseconds",
            Self::ZonedDateTimePrototypeYear => "get year",
            Self::ZonedDateTimePrototypeMonth => "get month",
            Self::ZonedDateTimePrototypeMonthCode => "get monthCode",
            Self::ZonedDateTimePrototypeDay => "get day",
            Self::ZonedDateTimePrototypeHour => "get hour",
            Self::ZonedDateTimePrototypeMinute => "get minute",
            Self::ZonedDateTimePrototypeSecond => "get second",
            Self::ZonedDateTimePrototypeMillisecond => "get millisecond",
            Self::ZonedDateTimePrototypeMicrosecond => "get microsecond",
            Self::ZonedDateTimePrototypeNanosecond => "get nanosecond",
            Self::ZonedDateTimePrototypeEra => "get era",
            Self::ZonedDateTimePrototypeEraYear => "get eraYear",
            Self::ZonedDateTimePrototypeDayOfWeek => "get dayOfWeek",
            Self::ZonedDateTimePrototypeDayOfYear => "get dayOfYear",
            Self::ZonedDateTimePrototypeWeekOfYear => "get weekOfYear",
            Self::ZonedDateTimePrototypeYearOfWeek => "get yearOfWeek",
            Self::ZonedDateTimePrototypeHoursInDay => "get hoursInDay",
            Self::ZonedDateTimePrototypeDaysInWeek => "get daysInWeek",
            Self::ZonedDateTimePrototypeDaysInMonth => "get daysInMonth",
            Self::ZonedDateTimePrototypeDaysInYear => "get daysInYear",
            Self::ZonedDateTimePrototypeMonthsInYear => "get monthsInYear",
            Self::ZonedDateTimePrototypeInLeapYear => "get inLeapYear",
            Self::ZonedDateTimePrototypeOffset => "get offset",
            Self::ZonedDateTimePrototypeOffsetNanoseconds => "get offsetNanoseconds",
            Self::ZonedDateTimePrototypeAdd => "add",
            Self::ZonedDateTimePrototypeSubtract => "subtract",
            Self::ZonedDateTimePrototypeWith => "with",
            Self::ZonedDateTimePrototypeUntil => "until",
            Self::ZonedDateTimePrototypeSince => "since",
            Self::ZonedDateTimePrototypeRound => "round",
            Self::ZonedDateTimePrototypeEquals => "equals",
            Self::ZonedDateTimePrototypeStartOfDay => "startOfDay",
            Self::ZonedDateTimePrototypeGetTimeZoneTransition => "getTimeZoneTransition",
            Self::ZonedDateTimePrototypeWithPlainTime => "withPlainTime",
            Self::ZonedDateTimePrototypeWithTimeZone => "withTimeZone",
            Self::ZonedDateTimePrototypeWithCalendar => "withCalendar",
            Self::ZonedDateTimePrototypeToInstant => "toInstant",
            Self::ZonedDateTimePrototypeToPlainDate => "toPlainDate",
            Self::ZonedDateTimePrototypeToPlainTime => "toPlainTime",
            Self::ZonedDateTimePrototypeToPlainDateTime => "toPlainDateTime",
            Self::ZonedDateTimePrototypeToLocaleString => "toLocaleString",
            _ => "Instant",
        }
    }

    pub(in crate::runtime::native) const fn is_zoned_date_time(self) -> bool {
        matches!(
            self,
            Self::ZonedDateTimeConstructor
                | Self::ZonedDateTimeFrom
                | Self::ZonedDateTimeCompare
                | Self::ZonedDateTimePrototypeEpochMilliseconds
                | Self::ZonedDateTimePrototypeEpochNanoseconds
                | Self::ZonedDateTimePrototypeTimeZoneId
                | Self::ZonedDateTimePrototypeCalendarId
                | Self::ZonedDateTimePrototypeYear
                | Self::ZonedDateTimePrototypeMonth
                | Self::ZonedDateTimePrototypeMonthCode
                | Self::ZonedDateTimePrototypeDay
                | Self::ZonedDateTimePrototypeHour
                | Self::ZonedDateTimePrototypeMinute
                | Self::ZonedDateTimePrototypeSecond
                | Self::ZonedDateTimePrototypeMillisecond
                | Self::ZonedDateTimePrototypeMicrosecond
                | Self::ZonedDateTimePrototypeNanosecond
                | Self::ZonedDateTimePrototypeEra
                | Self::ZonedDateTimePrototypeEraYear
                | Self::ZonedDateTimePrototypeDayOfWeek
                | Self::ZonedDateTimePrototypeDayOfYear
                | Self::ZonedDateTimePrototypeWeekOfYear
                | Self::ZonedDateTimePrototypeYearOfWeek
                | Self::ZonedDateTimePrototypeHoursInDay
                | Self::ZonedDateTimePrototypeDaysInWeek
                | Self::ZonedDateTimePrototypeDaysInMonth
                | Self::ZonedDateTimePrototypeDaysInYear
                | Self::ZonedDateTimePrototypeMonthsInYear
                | Self::ZonedDateTimePrototypeInLeapYear
                | Self::ZonedDateTimePrototypeOffset
                | Self::ZonedDateTimePrototypeOffsetNanoseconds
                | Self::ZonedDateTimePrototypeAdd
                | Self::ZonedDateTimePrototypeSubtract
                | Self::ZonedDateTimePrototypeWith
                | Self::ZonedDateTimePrototypeUntil
                | Self::ZonedDateTimePrototypeSince
                | Self::ZonedDateTimePrototypeRound
                | Self::ZonedDateTimePrototypeEquals
                | Self::ZonedDateTimePrototypeStartOfDay
                | Self::ZonedDateTimePrototypeGetTimeZoneTransition
                | Self::ZonedDateTimePrototypeWithPlainTime
                | Self::ZonedDateTimePrototypeWithTimeZone
                | Self::ZonedDateTimePrototypeWithCalendar
                | Self::ZonedDateTimePrototypeToInstant
                | Self::ZonedDateTimePrototypeToPlainDate
                | Self::ZonedDateTimePrototypeToPlainTime
                | Self::ZonedDateTimePrototypeToPlainDateTime
                | Self::ZonedDateTimePrototypeToString
                | Self::ZonedDateTimePrototypeToLocaleString
                | Self::ZonedDateTimePrototypeToJson
                | Self::ZonedDateTimePrototypeValueOf
        )
    }
}
