use super::{DataViewFunctionKind, DateFunctionKind, IteratorFunctionKind, NativeFunctionKind};

impl NativeFunctionKind {
    pub(in crate::runtime) const fn is_constructable(self) -> bool {
        matches!(
            self,
            Self::Array
                | Self::ArrayBuffer
                | Self::AsyncFunction
                | Self::AsyncGeneratorFunction
                | Self::Boolean
                | Self::DataView(DataViewFunctionKind::Constructor)
                | Self::ErrorConstructor(_)
                | Self::Function
                | Self::Iterator(IteratorFunctionKind::Constructor)
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
        )
    }
}
