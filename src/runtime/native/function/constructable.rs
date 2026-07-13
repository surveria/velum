use super::{
    AsyncDisposableStackFunctionKind, DataViewFunctionKind, DateFunctionKind,
    DisposableStackFunctionKind, IntlFunctionKind, IteratorFunctionKind, NativeFunctionKind,
    ShadowRealmFunctionKind, TemporalFunctionKind,
};

impl NativeFunctionKind {
    pub(in crate::runtime::native) const fn has_own_prototype_property(self) -> bool {
        matches!(
            self,
            Self::Array
                | Self::AsyncFunction
                | Self::AsyncGeneratorFunction
                | Self::GeneratorFunction
                | Self::AsyncDisposableStack(AsyncDisposableStackFunctionKind::Constructor)
                | Self::Boolean
                | Self::BigInt
                | Self::DataView(DataViewFunctionKind::Constructor)
                | Self::ErrorConstructor(_)
                | Self::Function
                | Self::FinalizationRegistry
                | Self::WeakRef
                | Self::Iterator(IteratorFunctionKind::Constructor)
                | Self::Intl(
                    IntlFunctionKind::DateTimeFormatConstructor
                        | IntlFunctionKind::DurationFormatConstructor
                        | IntlFunctionKind::CollatorConstructor
                        | IntlFunctionKind::LocaleConstructor
                        | IntlFunctionKind::ListFormatConstructor
                        | IntlFunctionKind::NumberFormatConstructor
                        | IntlFunctionKind::SegmenterConstructor
                        | IntlFunctionKind::PluralRulesConstructor
                        | IntlFunctionKind::RelativeTimeFormatConstructor
                )
                | Self::Number
                | Self::Object
                | Self::Promise
                | Self::RegExp
                | Self::String
                | Self::Map
                | Self::Set
                | Self::Symbol
                | Self::ArrayBuffer
                | Self::SharedArrayBuffer
                | Self::ShadowRealm(ShadowRealmFunctionKind::Constructor)
                | Self::TypedArrayIntrinsic
                | Self::TypedArray(_)
                | Self::WeakMap
                | Self::WeakSet
                | Self::Date(DateFunctionKind::Constructor)
                | Self::Temporal(
                    TemporalFunctionKind::Constructor
                        | TemporalFunctionKind::PlainDateConstructor
                        | TemporalFunctionKind::PlainDateTimeConstructor
                        | TemporalFunctionKind::InstantConstructor
                        | TemporalFunctionKind::PlainMonthDayConstructor
                        | TemporalFunctionKind::PlainTimeConstructor
                        | TemporalFunctionKind::PlainYearMonthConstructor
                        | TemporalFunctionKind::ZonedDateTimeConstructor
                )
                | Self::DisposableStack(
                    super::disposable_stack_kind::DisposableStackFunctionKind::Constructor,
                )
        )
    }

    pub(in crate::runtime) const fn is_constructable(self) -> bool {
        matches!(
            self,
            Self::Array
                | Self::ArrayBuffer
                | Self::SharedArrayBuffer
                | Self::ShadowRealm(ShadowRealmFunctionKind::Constructor)
                | Self::AsyncFunction
                | Self::AsyncGeneratorFunction
                | Self::GeneratorFunction
                | Self::Boolean
                | Self::BigInt
                | Self::DataView(DataViewFunctionKind::Constructor)
                | Self::ErrorConstructor(_)
                | Self::Function
                | Self::FinalizationRegistry
                | Self::WeakRef
                | Self::Iterator(IteratorFunctionKind::Constructor)
                | Self::Intl(
                    IntlFunctionKind::DateTimeFormatConstructor
                        | IntlFunctionKind::DurationFormatConstructor
                        | IntlFunctionKind::CollatorConstructor
                        | IntlFunctionKind::LocaleConstructor
                        | IntlFunctionKind::ListFormatConstructor
                        | IntlFunctionKind::NumberFormatConstructor
                        | IntlFunctionKind::SegmenterConstructor
                        | IntlFunctionKind::PluralRulesConstructor
                        | IntlFunctionKind::RelativeTimeFormatConstructor
                )
                | Self::Number
                | Self::Object
                | Self::Promise
                | Self::Proxy
                | Self::RegExp
                | Self::String
                | Self::Map
                | Self::Set
                | Self::WeakMap
                | Self::WeakSet
                | Self::TypedArray(_)
                | Self::Date(DateFunctionKind::Constructor)
                | Self::Temporal(
                    TemporalFunctionKind::Constructor
                        | TemporalFunctionKind::PlainDateConstructor
                        | TemporalFunctionKind::PlainDateTimeConstructor
                        | TemporalFunctionKind::InstantConstructor
                        | TemporalFunctionKind::PlainMonthDayConstructor
                        | TemporalFunctionKind::PlainTimeConstructor
                        | TemporalFunctionKind::PlainYearMonthConstructor
                        | TemporalFunctionKind::ZonedDateTimeConstructor
                )
                | Self::AsyncDisposableStack(AsyncDisposableStackFunctionKind::Constructor)
                | Self::DisposableStack(DisposableStackFunctionKind::Constructor)
        )
    }
}
