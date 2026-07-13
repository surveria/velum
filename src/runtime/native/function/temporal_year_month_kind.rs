use super::TemporalFunctionKind;

impl TemporalFunctionKind {
    pub(in crate::runtime::native) const fn plain_year_month_index(self) -> usize {
        match self {
            Self::PlainYearMonthFrom => 143,
            Self::PlainYearMonthCompare => 144,
            Self::PlainYearMonthPrototypeYear => 145,
            Self::PlainYearMonthPrototypeMonth => 146,
            Self::PlainYearMonthPrototypeMonthCode => 147,
            Self::PlainYearMonthPrototypeCalendarId => 148,
            Self::PlainYearMonthPrototypeEra => 149,
            Self::PlainYearMonthPrototypeEraYear => 150,
            Self::PlainYearMonthPrototypeDaysInMonth => 151,
            Self::PlainYearMonthPrototypeDaysInYear => 152,
            Self::PlainYearMonthPrototypeMonthsInYear => 153,
            Self::PlainYearMonthPrototypeInLeapYear => 154,
            Self::PlainYearMonthPrototypeWith => 155,
            Self::PlainYearMonthPrototypeAdd => 156,
            Self::PlainYearMonthPrototypeSubtract => 157,
            Self::PlainYearMonthPrototypeUntil => 158,
            Self::PlainYearMonthPrototypeSince => 159,
            Self::PlainYearMonthPrototypeEquals => 160,
            Self::PlainYearMonthPrototypeToPlainDate => 161,
            Self::PlainYearMonthPrototypeToString => 162,
            Self::PlainYearMonthPrototypeToLocaleString => 163,
            Self::PlainYearMonthPrototypeToJson => 164,
            Self::PlainYearMonthPrototypeValueOf => 165,
            _ => self.instant_index(),
        }
    }

    pub(in crate::runtime::native) const fn plain_year_month_name(self) -> &'static str {
        match self {
            Self::PlainYearMonthFrom => "from",
            Self::PlainYearMonthCompare => "compare",
            Self::PlainYearMonthPrototypeYear => "get year",
            Self::PlainYearMonthPrototypeMonth => "get month",
            Self::PlainYearMonthPrototypeMonthCode => "get monthCode",
            Self::PlainYearMonthPrototypeCalendarId => "get calendarId",
            Self::PlainYearMonthPrototypeEra => "get era",
            Self::PlainYearMonthPrototypeEraYear => "get eraYear",
            Self::PlainYearMonthPrototypeDaysInMonth => "get daysInMonth",
            Self::PlainYearMonthPrototypeDaysInYear => "get daysInYear",
            Self::PlainYearMonthPrototypeMonthsInYear => "get monthsInYear",
            Self::PlainYearMonthPrototypeInLeapYear => "get inLeapYear",
            Self::PlainYearMonthPrototypeWith => "with",
            Self::PlainYearMonthPrototypeAdd => "add",
            Self::PlainYearMonthPrototypeSubtract => "subtract",
            Self::PlainYearMonthPrototypeUntil => "until",
            Self::PlainYearMonthPrototypeSince => "since",
            Self::PlainYearMonthPrototypeEquals => "equals",
            Self::PlainYearMonthPrototypeToPlainDate => "toPlainDate",
            Self::PlainYearMonthPrototypeToString => "toString",
            Self::PlainYearMonthPrototypeToLocaleString => "toLocaleString",
            Self::PlainYearMonthPrototypeToJson => "toJSON",
            Self::PlainYearMonthPrototypeValueOf => "valueOf",
            Self::PlainYearMonthConstructor => "PlainYearMonth",
            _ => self.instant_name(),
        }
    }

    pub(in crate::runtime::native) const fn is_plain_year_month(self) -> bool {
        matches!(
            self,
            Self::PlainYearMonthConstructor
                | Self::PlainYearMonthFrom
                | Self::PlainYearMonthCompare
                | Self::PlainYearMonthPrototypeYear
                | Self::PlainYearMonthPrototypeMonth
                | Self::PlainYearMonthPrototypeMonthCode
                | Self::PlainYearMonthPrototypeCalendarId
                | Self::PlainYearMonthPrototypeEra
                | Self::PlainYearMonthPrototypeEraYear
                | Self::PlainYearMonthPrototypeDaysInMonth
                | Self::PlainYearMonthPrototypeDaysInYear
                | Self::PlainYearMonthPrototypeMonthsInYear
                | Self::PlainYearMonthPrototypeInLeapYear
                | Self::PlainYearMonthPrototypeWith
                | Self::PlainYearMonthPrototypeAdd
                | Self::PlainYearMonthPrototypeSubtract
                | Self::PlainYearMonthPrototypeUntil
                | Self::PlainYearMonthPrototypeSince
                | Self::PlainYearMonthPrototypeEquals
                | Self::PlainYearMonthPrototypeToPlainDate
                | Self::PlainYearMonthPrototypeToString
                | Self::PlainYearMonthPrototypeToLocaleString
                | Self::PlainYearMonthPrototypeToJson
                | Self::PlainYearMonthPrototypeValueOf
        )
    }
}
