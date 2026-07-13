pub(in crate::runtime) const INTL_NAME: &str = "Intl";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum IntlFunctionKind {
    DateTimeFormatConstructor,
    DateTimeFormatFormat,
    DateTimeFormatFormatToParts,
    DateTimeFormatResolvedOptions,
    DurationFormatConstructor,
    DurationFormatFormat,
    SupportedValuesOf,
    CollatorConstructor,
    NumberFormatConstructor,
    PluralRulesConstructor,
    RelativeTimeFormatConstructor,
}

impl IntlFunctionKind {
    pub(in crate::runtime::native) const fn index(self) -> usize {
        match self {
            Self::DateTimeFormatConstructor => 0,
            Self::DateTimeFormatFormat => 1,
            Self::DateTimeFormatFormatToParts => 2,
            Self::DateTimeFormatResolvedOptions => 3,
            Self::DurationFormatConstructor => 4,
            Self::DurationFormatFormat => 5,
            Self::SupportedValuesOf => 6,
            Self::CollatorConstructor => 7,
            Self::NumberFormatConstructor => 8,
            Self::PluralRulesConstructor => 9,
            Self::RelativeTimeFormatConstructor => 10,
        }
    }

    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::DateTimeFormatConstructor
            | Self::DurationFormatConstructor
            | Self::DateTimeFormatResolvedOptions
            | Self::CollatorConstructor
            | Self::NumberFormatConstructor
            | Self::PluralRulesConstructor
            | Self::RelativeTimeFormatConstructor => 0.0,
            Self::DateTimeFormatFormat
            | Self::DateTimeFormatFormatToParts
            | Self::DurationFormatFormat
            | Self::SupportedValuesOf => 1.0,
        }
    }

    pub(in crate::runtime) const fn name(self) -> &'static str {
        match self {
            Self::DateTimeFormatConstructor => "DateTimeFormat",
            Self::DateTimeFormatFormat | Self::DurationFormatFormat => "format",
            Self::DateTimeFormatFormatToParts => "formatToParts",
            Self::DateTimeFormatResolvedOptions => "resolvedOptions",
            Self::DurationFormatConstructor => "DurationFormat",
            Self::SupportedValuesOf => "supportedValuesOf",
            Self::CollatorConstructor => "Collator",
            Self::NumberFormatConstructor => "NumberFormat",
            Self::PluralRulesConstructor => "PluralRules",
            Self::RelativeTimeFormatConstructor => "RelativeTimeFormat",
        }
    }
}
