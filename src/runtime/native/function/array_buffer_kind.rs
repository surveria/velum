#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum ArrayBufferFunctionKind {
    IsView,
    ByteLengthGetter,
    MaxByteLengthGetter,
    ResizableGetter,
    DetachedGetter,
    ImmutableGetter,
    Resize,
    Slice,
    Transfer,
    TransferToFixedLength,
    SliceToImmutable,
    TransferToImmutable,
}

impl ArrayBufferFunctionKind {
    pub(in crate::runtime::native) const fn index(self) -> usize {
        match self {
            Self::IsView => 0,
            Self::ByteLengthGetter => 1,
            Self::MaxByteLengthGetter => 2,
            Self::ResizableGetter => 3,
            Self::DetachedGetter => 4,
            Self::Resize => 5,
            Self::Slice => 6,
            Self::Transfer => 7,
            Self::TransferToFixedLength => 8,
            Self::ImmutableGetter => 9,
            Self::SliceToImmutable => 10,
            Self::TransferToImmutable => 11,
        }
    }

    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::IsView | Self::Resize => 1.0,
            Self::Slice | Self::SliceToImmutable => 2.0,
            Self::ByteLengthGetter
            | Self::MaxByteLengthGetter
            | Self::ResizableGetter
            | Self::DetachedGetter
            | Self::ImmutableGetter
            | Self::Transfer
            | Self::TransferToFixedLength
            | Self::TransferToImmutable => 0.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::IsView => "isView",
            Self::ByteLengthGetter => "get byteLength",
            Self::MaxByteLengthGetter => "get maxByteLength",
            Self::ResizableGetter => "get resizable",
            Self::DetachedGetter => "get detached",
            Self::ImmutableGetter => "get immutable",
            Self::Resize => "resize",
            Self::Slice => "slice",
            Self::Transfer => "transfer",
            Self::TransferToFixedLength => "transferToFixedLength",
            Self::SliceToImmutable => "sliceToImmutable",
            Self::TransferToImmutable => "transferToImmutable",
        }
    }
}
