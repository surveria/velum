use crate::value::ObjectId;

pub(in crate::runtime) const INTL_NAME: &str = "Intl";

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
            | Self::DateTimeFormatFormatGetter => 0.0,
            Self::DateTimeFormatFormatToParts
            | Self::DurationFormatFormat
            | Self::SupportedValuesOf
            | Self::NumberFormatBoundFormat(_)
            | Self::NumberFormatFormatToParts
            | Self::NumberFormatSupportedLocalesOf
            | Self::DateTimeFormatBoundFormat(_)
            | Self::DateTimeFormatSupportedLocalesOf => 1.0,
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
        }
    }
}
