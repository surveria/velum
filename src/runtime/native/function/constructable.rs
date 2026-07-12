use super::{
    DataViewFunctionKind, DateFunctionKind, DisposableStackFunctionKind, IteratorFunctionKind,
    NativeFunctionKind,
};

impl NativeFunctionKind {
    pub(in crate::runtime::native) const fn has_own_prototype_property(self) -> bool {
        matches!(
            self,
            Self::Array
                | Self::AsyncFunction
                | Self::AsyncGeneratorFunction
                | Self::Boolean
                | Self::BigInt
                | Self::DataView(DataViewFunctionKind::Constructor)
                | Self::ErrorConstructor(_)
                | Self::Function
                | Self::Iterator(IteratorFunctionKind::Constructor)
                | Self::Number
                | Self::Object
                | Self::Promise
                | Self::RegExp
                | Self::String
                | Self::Map
                | Self::Set
                | Self::Symbol
                | Self::ArrayBuffer
                | Self::TypedArrayIntrinsic
                | Self::TypedArray(_)
                | Self::WeakMap
                | Self::WeakSet
                | Self::Date(DateFunctionKind::Constructor)
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
                | Self::AsyncFunction
                | Self::AsyncGeneratorFunction
                | Self::Boolean
                | Self::BigInt
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
                | Self::DisposableStack(DisposableStackFunctionKind::Constructor)
        )
    }
}
