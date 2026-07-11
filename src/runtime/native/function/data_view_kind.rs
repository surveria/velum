use crate::runtime::object::DataViewElementKind;

pub(in crate::runtime) const DATA_VIEW_NAME: &str = "DataView";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum DataViewFunctionKind {
    Constructor,
    BufferGetter,
    ByteLengthGetter,
    ByteOffsetGetter,
    Get(DataViewElementKind),
    Set(DataViewElementKind),
}

impl DataViewFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Constructor | Self::Get(_) => 1.0,
            Self::Set(_) => 2.0,
            Self::BufferGetter | Self::ByteLengthGetter | Self::ByteOffsetGetter => 0.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Constructor => DATA_VIEW_NAME,
            Self::BufferGetter => "get buffer",
            Self::ByteLengthGetter => "get byteLength",
            Self::ByteOffsetGetter => "get byteOffset",
            Self::Get(kind) => kind.get_name(),
            Self::Set(kind) => kind.set_name(),
        }
    }
}
