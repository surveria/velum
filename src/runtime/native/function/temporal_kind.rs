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
        }
    }

    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Compare => 2.0,
            Self::From
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
            Self::From => "from",
            Self::Compare => "compare",
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
            Self::PrototypeWith => "with",
            Self::PrototypeNegated => "negated",
            Self::PrototypeAbs => "abs",
            Self::PrototypeAdd => "add",
            Self::PrototypeSubtract => "subtract",
            Self::PrototypeRound => "round",
            Self::PrototypeTotal => "total",
            Self::PrototypeToString => "toString",
            Self::PrototypeToJson => "toJSON",
            Self::PrototypeToLocaleString => "toLocaleString",
            Self::PrototypeValueOf => "valueOf",
        }
    }
}
