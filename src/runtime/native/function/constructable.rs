use super::{DateFunctionKind, NativeFunctionKind};

impl NativeFunctionKind {
    pub(in crate::runtime) const fn is_constructable(self) -> bool {
        matches!(
            self,
            Self::Array
                | Self::ArrayBuffer
                | Self::AsyncFunction
                | Self::Boolean
                | Self::ErrorConstructor(_)
                | Self::Function
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
                | Self::Uint8Array
                | Self::Date(DateFunctionKind::Constructor)
        )
    }
}
