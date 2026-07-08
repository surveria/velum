const DATE_GETTER_LENGTH: f64 = 0.0;

const DATE_FUNCTION_LENGTH: f64 = 7.0;
pub(in crate::runtime) const DATE_NAME: &str = "Date";
const DATE_NOW_FUNCTION_LENGTH: f64 = 0.0;
pub(in crate::runtime) const DATE_NOW_NAME: &str = "now";
const DATE_PARSE_FUNCTION_LENGTH: f64 = 1.0;
pub(in crate::runtime) const DATE_PARSE_NAME: &str = "parse";
const DATE_PROTOTYPE_GET_DATE_NAME: &str = "getDate";
const DATE_PROTOTYPE_GET_DAY_NAME: &str = "getDay";
const DATE_PROTOTYPE_GET_FULL_YEAR_NAME: &str = "getFullYear";
const DATE_PROTOTYPE_GET_HOURS_NAME: &str = "getHours";
const DATE_PROTOTYPE_GET_MILLISECONDS_NAME: &str = "getMilliseconds";
const DATE_PROTOTYPE_GET_MINUTES_NAME: &str = "getMinutes";
const DATE_PROTOTYPE_GET_MONTH_NAME: &str = "getMonth";
const DATE_PROTOTYPE_GET_SECONDS_NAME: &str = "getSeconds";
const DATE_PROTOTYPE_GET_TIME_NAME: &str = "getTime";
const DATE_PROTOTYPE_GET_UTC_DATE_NAME: &str = "getUTCDate";
const DATE_PROTOTYPE_GET_UTC_DAY_NAME: &str = "getUTCDay";
const DATE_PROTOTYPE_GET_UTC_FULL_YEAR_NAME: &str = "getUTCFullYear";
const DATE_PROTOTYPE_GET_UTC_HOURS_NAME: &str = "getUTCHours";
const DATE_PROTOTYPE_GET_UTC_MILLISECONDS_NAME: &str = "getUTCMilliseconds";
const DATE_PROTOTYPE_GET_UTC_MINUTES_NAME: &str = "getUTCMinutes";
const DATE_PROTOTYPE_GET_UTC_MONTH_NAME: &str = "getUTCMonth";
const DATE_PROTOTYPE_GET_UTC_SECONDS_NAME: &str = "getUTCSeconds";
const DATE_PROTOTYPE_SET_TIME_NAME: &str = "setTime";
const DATE_PROTOTYPE_TO_DATE_STRING_NAME: &str = "toDateString";
const DATE_PROTOTYPE_TO_ISO_STRING_NAME: &str = "toISOString";
const DATE_PROTOTYPE_TO_JSON_NAME: &str = "toJSON";
const DATE_PROTOTYPE_TO_STRING_NAME: &str = "toString";
const DATE_PROTOTYPE_TO_TIME_STRING_NAME: &str = "toTimeString";
const DATE_PROTOTYPE_TO_UTC_STRING_NAME: &str = "toUTCString";
const DATE_PROTOTYPE_VALUE_OF_NAME: &str = "valueOf";
const DATE_UTC_FUNCTION_LENGTH: f64 = 7.0;
pub(in crate::runtime) const DATE_UTC_NAME: &str = "UTC";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum DateFunctionKind {
    Constructor,
    Now,
    Parse,
    PrototypeGetDate,
    PrototypeGetDay,
    PrototypeGetFullYear,
    PrototypeGetHours,
    PrototypeGetMilliseconds,
    PrototypeGetMinutes,
    PrototypeGetMonth,
    PrototypeGetSeconds,
    PrototypeGetTime,
    PrototypeGetUtcDate,
    PrototypeGetUtcDay,
    PrototypeGetUtcFullYear,
    PrototypeGetUtcHours,
    PrototypeGetUtcMilliseconds,
    PrototypeGetUtcMinutes,
    PrototypeGetUtcMonth,
    PrototypeGetUtcSeconds,
    PrototypeSetTime,
    PrototypeToDateString,
    PrototypeToIsoString,
    PrototypeToJson,
    PrototypeToString,
    PrototypeToTimeString,
    PrototypeToUtcString,
    PrototypeValueOf,
    Utc,
}

impl DateFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Constructor => DATE_FUNCTION_LENGTH,
            Self::Utc => DATE_UTC_FUNCTION_LENGTH,
            Self::Now => DATE_NOW_FUNCTION_LENGTH,
            Self::Parse | Self::PrototypeSetTime => DATE_PARSE_FUNCTION_LENGTH,
            Self::PrototypeGetDate
            | Self::PrototypeGetDay
            | Self::PrototypeGetFullYear
            | Self::PrototypeGetHours
            | Self::PrototypeGetMilliseconds
            | Self::PrototypeGetMinutes
            | Self::PrototypeGetMonth
            | Self::PrototypeGetSeconds
            | Self::PrototypeGetTime
            | Self::PrototypeGetUtcDate
            | Self::PrototypeGetUtcDay
            | Self::PrototypeGetUtcFullYear
            | Self::PrototypeGetUtcHours
            | Self::PrototypeGetUtcMilliseconds
            | Self::PrototypeGetUtcMinutes
            | Self::PrototypeGetUtcMonth
            | Self::PrototypeGetUtcSeconds
            | Self::PrototypeToDateString
            | Self::PrototypeToIsoString
            | Self::PrototypeToJson
            | Self::PrototypeToString
            | Self::PrototypeToTimeString
            | Self::PrototypeToUtcString
            | Self::PrototypeValueOf => DATE_GETTER_LENGTH,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Constructor => DATE_NAME,
            Self::Now => DATE_NOW_NAME,
            Self::Parse => DATE_PARSE_NAME,
            Self::PrototypeGetDate => DATE_PROTOTYPE_GET_DATE_NAME,
            Self::PrototypeGetDay => DATE_PROTOTYPE_GET_DAY_NAME,
            Self::PrototypeGetFullYear => DATE_PROTOTYPE_GET_FULL_YEAR_NAME,
            Self::PrototypeGetHours => DATE_PROTOTYPE_GET_HOURS_NAME,
            Self::PrototypeGetMilliseconds => DATE_PROTOTYPE_GET_MILLISECONDS_NAME,
            Self::PrototypeGetMinutes => DATE_PROTOTYPE_GET_MINUTES_NAME,
            Self::PrototypeGetMonth => DATE_PROTOTYPE_GET_MONTH_NAME,
            Self::PrototypeGetSeconds => DATE_PROTOTYPE_GET_SECONDS_NAME,
            Self::PrototypeGetTime => DATE_PROTOTYPE_GET_TIME_NAME,
            Self::PrototypeGetUtcDate => DATE_PROTOTYPE_GET_UTC_DATE_NAME,
            Self::PrototypeGetUtcDay => DATE_PROTOTYPE_GET_UTC_DAY_NAME,
            Self::PrototypeGetUtcFullYear => DATE_PROTOTYPE_GET_UTC_FULL_YEAR_NAME,
            Self::PrototypeGetUtcHours => DATE_PROTOTYPE_GET_UTC_HOURS_NAME,
            Self::PrototypeGetUtcMilliseconds => DATE_PROTOTYPE_GET_UTC_MILLISECONDS_NAME,
            Self::PrototypeGetUtcMinutes => DATE_PROTOTYPE_GET_UTC_MINUTES_NAME,
            Self::PrototypeGetUtcMonth => DATE_PROTOTYPE_GET_UTC_MONTH_NAME,
            Self::PrototypeGetUtcSeconds => DATE_PROTOTYPE_GET_UTC_SECONDS_NAME,
            Self::PrototypeSetTime => DATE_PROTOTYPE_SET_TIME_NAME,
            Self::PrototypeToDateString => DATE_PROTOTYPE_TO_DATE_STRING_NAME,
            Self::PrototypeToIsoString => DATE_PROTOTYPE_TO_ISO_STRING_NAME,
            Self::PrototypeToJson => DATE_PROTOTYPE_TO_JSON_NAME,
            Self::PrototypeToString => DATE_PROTOTYPE_TO_STRING_NAME,
            Self::PrototypeToTimeString => DATE_PROTOTYPE_TO_TIME_STRING_NAME,
            Self::PrototypeToUtcString => DATE_PROTOTYPE_TO_UTC_STRING_NAME,
            Self::PrototypeValueOf => DATE_PROTOTYPE_VALUE_OF_NAME,
            Self::Utc => DATE_UTC_NAME,
        }
    }
}
