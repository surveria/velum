pub(in crate::runtime::native) const SHARED_ARRAY_BUFFER_NAME: &str = "SharedArrayBuffer";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum SharedArrayBufferFunctionKind {
    ByteLengthGetter,
    MaxByteLengthGetter,
    GrowableGetter,
    Grow,
    Slice,
}

impl SharedArrayBufferFunctionKind {
    pub(in crate::runtime::native) const fn index(self) -> usize {
        match self {
            Self::ByteLengthGetter => 0,
            Self::MaxByteLengthGetter => 1,
            Self::GrowableGetter => 2,
            Self::Grow => 3,
            Self::Slice => 4,
        }
    }

    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Grow => 1.0,
            Self::Slice => 2.0,
            Self::ByteLengthGetter | Self::MaxByteLengthGetter | Self::GrowableGetter => 0.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::ByteLengthGetter => "get byteLength",
            Self::MaxByteLengthGetter => "get maxByteLength",
            Self::GrowableGetter => "get growable",
            Self::Grow => "grow",
            Self::Slice => "slice",
        }
    }
}
