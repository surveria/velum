use super::TemporalFunctionKind;

impl TemporalFunctionKind {
    pub(in crate::runtime::native) const fn instant_index(self) -> usize {
        match self {
            Self::InstantFrom => 166,
            Self::InstantFromEpochMilliseconds => 167,
            Self::InstantFromEpochNanoseconds => 168,
            Self::InstantCompare => 169,
            Self::InstantPrototypeEpochMilliseconds => 170,
            Self::InstantPrototypeEpochNanoseconds => 171,
            Self::InstantPrototypeAdd => 172,
            Self::InstantPrototypeSubtract => 173,
            Self::InstantPrototypeUntil => 174,
            Self::InstantPrototypeSince => 175,
            Self::InstantPrototypeRound => 176,
            Self::InstantPrototypeEquals => 177,
            Self::InstantPrototypeToZonedDateTimeIso => 178,
            Self::InstantPrototypeToString => 179,
            Self::InstantPrototypeToLocaleString => 180,
            Self::InstantPrototypeToJson => 181,
            Self::InstantPrototypeValueOf => 182,
            _ => self.zoned_date_time_index(),
        }
    }

    pub(in crate::runtime::native) const fn instant_name(self) -> &'static str {
        match self {
            Self::InstantFrom => "from",
            Self::InstantFromEpochMilliseconds => "fromEpochMilliseconds",
            Self::InstantFromEpochNanoseconds => "fromEpochNanoseconds",
            Self::InstantCompare => "compare",
            Self::InstantPrototypeEpochMilliseconds => "get epochMilliseconds",
            Self::InstantPrototypeEpochNanoseconds => "get epochNanoseconds",
            Self::InstantPrototypeAdd => "add",
            Self::InstantPrototypeSubtract => "subtract",
            Self::InstantPrototypeUntil => "until",
            Self::InstantPrototypeSince => "since",
            Self::InstantPrototypeRound => "round",
            Self::InstantPrototypeEquals => "equals",
            Self::InstantPrototypeToZonedDateTimeIso => "toZonedDateTimeISO",
            Self::InstantPrototypeToString => "toString",
            Self::InstantPrototypeToLocaleString => "toLocaleString",
            Self::InstantPrototypeToJson => "toJSON",
            Self::InstantPrototypeValueOf => "valueOf",
            _ => self.zoned_date_time_name(),
        }
    }

    pub(in crate::runtime::native) const fn is_instant(self) -> bool {
        matches!(
            self,
            Self::InstantConstructor
                | Self::InstantFrom
                | Self::InstantFromEpochMilliseconds
                | Self::InstantFromEpochNanoseconds
                | Self::InstantCompare
                | Self::InstantPrototypeEpochMilliseconds
                | Self::InstantPrototypeEpochNanoseconds
                | Self::InstantPrototypeAdd
                | Self::InstantPrototypeSubtract
                | Self::InstantPrototypeUntil
                | Self::InstantPrototypeSince
                | Self::InstantPrototypeRound
                | Self::InstantPrototypeEquals
                | Self::InstantPrototypeToZonedDateTimeIso
                | Self::InstantPrototypeToString
                | Self::InstantPrototypeToLocaleString
                | Self::InstantPrototypeToJson
                | Self::InstantPrototypeValueOf
        )
    }
}
