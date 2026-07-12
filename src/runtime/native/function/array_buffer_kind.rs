#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum ArrayBufferFunctionKind {
    IsView,
    ByteLengthGetter,
    MaxByteLengthGetter,
    ResizableGetter,
    DetachedGetter,
    Resize,
    Slice,
    Transfer,
    TransferToFixedLength,
}

impl ArrayBufferFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::IsView | Self::Resize => 1.0,
            Self::Slice => 2.0,
            Self::ByteLengthGetter
            | Self::MaxByteLengthGetter
            | Self::ResizableGetter
            | Self::DetachedGetter
            | Self::Transfer
            | Self::TransferToFixedLength => 0.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::IsView => "isView",
            Self::ByteLengthGetter => "get byteLength",
            Self::MaxByteLengthGetter => "get maxByteLength",
            Self::ResizableGetter => "get resizable",
            Self::DetachedGetter => "get detached",
            Self::Resize => "resize",
            Self::Slice => "slice",
            Self::Transfer => "transfer",
            Self::TransferToFixedLength => "transferToFixedLength",
        }
    }
}
