use crate::value::ObjectId;

pub(in crate::runtime) const INTL_NAME: &str = "Intl";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum LocaleAccessorKind {
    BaseName,
    Calendar,
    CaseFirst,
    Collation,
    FirstDayOfWeek,
    HourCycle,
    Language,
    NumberingSystem,
    Numeric,
    Region,
    Script,
    Variants,
}

impl LocaleAccessorKind {
    const fn index(self) -> usize {
        match self {
            Self::BaseName => 0,
            Self::Calendar => 1,
            Self::CaseFirst => 2,
            Self::Collation => 3,
            Self::FirstDayOfWeek => 4,
            Self::HourCycle => 5,
            Self::Language => 6,
            Self::NumberingSystem => 7,
            Self::Numeric => 8,
            Self::Region => 9,
            Self::Script => 10,
            Self::Variants => 11,
        }
    }

    pub(in crate::runtime) const fn property_name(self) -> &'static str {
        match self {
            Self::BaseName => "baseName",
            Self::Calendar => "calendar",
            Self::CaseFirst => "caseFirst",
            Self::Collation => "collation",
            Self::FirstDayOfWeek => "firstDayOfWeek",
            Self::HourCycle => "hourCycle",
            Self::Language => "language",
            Self::NumberingSystem => "numberingSystem",
            Self::Numeric => "numeric",
            Self::Region => "region",
            Self::Script => "script",
            Self::Variants => "variants",
        }
    }

    const fn function_name(self) -> &'static str {
        match self {
            Self::BaseName => "get baseName",
            Self::Calendar => "get calendar",
            Self::CaseFirst => "get caseFirst",
            Self::Collation => "get collation",
            Self::FirstDayOfWeek => "get firstDayOfWeek",
            Self::HourCycle => "get hourCycle",
            Self::Language => "get language",
            Self::NumberingSystem => "get numberingSystem",
            Self::Numeric => "get numeric",
            Self::Region => "get region",
            Self::Script => "get script",
            Self::Variants => "get variants",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum LocaleMethodKind {
    GetCalendars,
    GetCollations,
    GetHourCycles,
    GetNumberingSystems,
    GetTextInfo,
    GetTimeZones,
    GetWeekInfo,
    Maximize,
    Minimize,
    ToString,
}

impl LocaleMethodKind {
    const fn index(self) -> usize {
        match self {
            Self::GetCalendars => 0,
            Self::GetCollations => 1,
            Self::GetHourCycles => 2,
            Self::GetNumberingSystems => 3,
            Self::GetTextInfo => 4,
            Self::GetTimeZones => 5,
            Self::GetWeekInfo => 6,
            Self::Maximize => 7,
            Self::Minimize => 8,
            Self::ToString => 9,
        }
    }

    pub(in crate::runtime) const fn name(self) -> &'static str {
        match self {
            Self::GetCalendars => "getCalendars",
            Self::GetCollations => "getCollations",
            Self::GetHourCycles => "getHourCycles",
            Self::GetNumberingSystems => "getNumberingSystems",
            Self::GetTextInfo => "getTextInfo",
            Self::GetTimeZones => "getTimeZones",
            Self::GetWeekInfo => "getWeekInfo",
            Self::Maximize => "maximize",
            Self::Minimize => "minimize",
            Self::ToString => "toString",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum IntlFunctionKind {
    DateTimeFormatConstructor,
    DateTimeFormatFormatToParts,
    DateTimeFormatResolvedOptions,
    DurationFormatConstructor,
    DurationFormatFormat,
    SupportedValuesOf,
    CollatorConstructor,
    NumberFormatConstructor,
    NumberFormatFormatGetter,
    NumberFormatBoundFormat(ObjectId),
    NumberFormatFormatToParts,
    NumberFormatResolvedOptions,
    NumberFormatFormatRange,
    NumberFormatFormatRangeToParts,
    NumberFormatSupportedLocalesOf,
    PluralRulesConstructor,
    RelativeTimeFormatConstructor,
    DateTimeFormatFormatGetter,
    DateTimeFormatBoundFormat(ObjectId),
    DateTimeFormatFormatRange,
    DateTimeFormatFormatRangeToParts,
    DateTimeFormatSupportedLocalesOf,
    LocaleConstructor,
    LocaleAccessor(LocaleAccessorKind),
    LocaleMethod(LocaleMethodKind),
    GetCanonicalLocales,
    ListFormatConstructor,
    ListFormatFormat,
    ListFormatFormatToParts,
    ListFormatResolvedOptions,
    ListFormatSupportedLocalesOf,
}

impl IntlFunctionKind {
    pub(in crate::runtime::native) const fn index(self) -> usize {
        match self {
            Self::DateTimeFormatConstructor => 0,
            Self::DateTimeFormatFormatToParts => 2,
            Self::DateTimeFormatResolvedOptions => 3,
            Self::DurationFormatConstructor => 4,
            Self::DurationFormatFormat => 5,
            Self::SupportedValuesOf => 6,
            Self::CollatorConstructor => 7,
            Self::NumberFormatConstructor => 8,
            Self::NumberFormatFormatGetter => 9,
            Self::NumberFormatBoundFormat(_) => 10,
            Self::NumberFormatFormatToParts => 11,
            Self::NumberFormatResolvedOptions => 12,
            Self::NumberFormatFormatRange => 13,
            Self::NumberFormatFormatRangeToParts => 14,
            Self::NumberFormatSupportedLocalesOf => 15,
            Self::PluralRulesConstructor => 16,
            Self::RelativeTimeFormatConstructor => 17,
            Self::DateTimeFormatFormatGetter => 18,
            Self::DateTimeFormatBoundFormat(_) => 19,
            Self::DateTimeFormatFormatRange => 20,
            Self::DateTimeFormatFormatRangeToParts => 21,
            Self::DateTimeFormatSupportedLocalesOf => 22,
            Self::LocaleConstructor => 23,
            Self::LocaleAccessor(kind) => 24 + kind.index(),
            Self::LocaleMethod(kind) => 36 + kind.index(),
            Self::GetCanonicalLocales => 46,
            Self::ListFormatConstructor => 52,
            Self::ListFormatFormat => 53,
            Self::ListFormatFormatToParts => 54,
            Self::ListFormatResolvedOptions => 55,
            Self::ListFormatSupportedLocalesOf => 56,
        }
    }

    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::DateTimeFormatConstructor
            | Self::DurationFormatConstructor
            | Self::DateTimeFormatResolvedOptions
            | Self::CollatorConstructor
            | Self::NumberFormatConstructor
            | Self::NumberFormatFormatGetter
            | Self::NumberFormatResolvedOptions
            | Self::PluralRulesConstructor
            | Self::RelativeTimeFormatConstructor
            | Self::ListFormatConstructor
            | Self::ListFormatResolvedOptions
            | Self::DateTimeFormatFormatGetter
            | Self::LocaleAccessor(_)
            | Self::LocaleMethod(_) => 0.0,
            Self::LocaleConstructor
            | Self::DateTimeFormatFormatToParts
            | Self::DurationFormatFormat
            | Self::SupportedValuesOf
            | Self::NumberFormatBoundFormat(_)
            | Self::NumberFormatFormatToParts
            | Self::NumberFormatSupportedLocalesOf
            | Self::DateTimeFormatBoundFormat(_)
            | Self::DateTimeFormatSupportedLocalesOf
            | Self::GetCanonicalLocales => 1.0,
            Self::ListFormatFormat
            | Self::ListFormatFormatToParts
            | Self::ListFormatSupportedLocalesOf => 1.0,
            Self::NumberFormatFormatRange
            | Self::NumberFormatFormatRangeToParts
            | Self::DateTimeFormatFormatRange
            | Self::DateTimeFormatFormatRangeToParts => 2.0,
        }
    }

    pub(in crate::runtime) const fn name(self) -> &'static str {
        match self {
            Self::DateTimeFormatConstructor => "DateTimeFormat",
            Self::DurationFormatFormat => "format",
            Self::DateTimeFormatFormatToParts | Self::NumberFormatFormatToParts => "formatToParts",
            Self::DateTimeFormatResolvedOptions | Self::NumberFormatResolvedOptions => {
                "resolvedOptions"
            }
            Self::DurationFormatConstructor => "DurationFormat",
            Self::SupportedValuesOf => "supportedValuesOf",
            Self::CollatorConstructor => "Collator",
            Self::NumberFormatConstructor => "NumberFormat",
            Self::NumberFormatFormatGetter | Self::DateTimeFormatFormatGetter => "get format",
            Self::NumberFormatBoundFormat(_) | Self::DateTimeFormatBoundFormat(_) => "",
            Self::NumberFormatFormatRange | Self::DateTimeFormatFormatRange => "formatRange",
            Self::NumberFormatFormatRangeToParts | Self::DateTimeFormatFormatRangeToParts => {
                "formatRangeToParts"
            }
            Self::NumberFormatSupportedLocalesOf | Self::DateTimeFormatSupportedLocalesOf => {
                "supportedLocalesOf"
            }
            Self::PluralRulesConstructor => "PluralRules",
            Self::RelativeTimeFormatConstructor => "RelativeTimeFormat",
            Self::LocaleConstructor => "Locale",
            Self::LocaleAccessor(kind) => kind.function_name(),
            Self::LocaleMethod(kind) => kind.name(),
            Self::GetCanonicalLocales => "getCanonicalLocales",
            Self::ListFormatConstructor => "ListFormat",
            Self::ListFormatFormat => "format",
            Self::ListFormatFormatToParts => "formatToParts",
            Self::ListFormatResolvedOptions => "resolvedOptions",
            Self::ListFormatSupportedLocalesOf => "supportedLocalesOf",
        }
    }
}
