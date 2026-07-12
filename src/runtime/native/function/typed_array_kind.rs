#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum TypedArrayFunctionKind {
    At,
    BufferGetter,
    ByteLengthGetter,
    ByteOffsetGetter,
    CopyWithin,
    Entries,
    Every,
    Fill,
    Filter,
    Find,
    FindIndex,
    FindLast,
    FindLastIndex,
    ForEach,
    From,
    FromBase64,
    FromHex,
    Includes,
    IndexOf,
    Join,
    Keys,
    LastIndexOf,
    LengthGetter,
    Map,
    Of,
    Reduce,
    ReduceRight,
    Reverse,
    Set,
    SetFromBase64,
    SetFromHex,
    Slice,
    Some,
    Sort,
    Subarray,
    ToLocaleString,
    ToBase64,
    ToHex,
    ToReversed,
    ToSorted,
    ToStringTagGetter,
    Values,
    With,
}

impl TypedArrayFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::CopyWithin | Self::Slice | Self::Subarray | Self::With => 2.0,
            Self::At
            | Self::Every
            | Self::Fill
            | Self::Filter
            | Self::Find
            | Self::FindIndex
            | Self::FindLast
            | Self::FindLastIndex
            | Self::ForEach
            | Self::From
            | Self::FromBase64
            | Self::FromHex
            | Self::Includes
            | Self::IndexOf
            | Self::Join
            | Self::LastIndexOf
            | Self::Map
            | Self::Reduce
            | Self::ReduceRight
            | Self::Set
            | Self::SetFromBase64
            | Self::SetFromHex
            | Self::Some
            | Self::Sort
            | Self::ToSorted => 1.0,
            Self::BufferGetter
            | Self::ByteLengthGetter
            | Self::ByteOffsetGetter
            | Self::Entries
            | Self::Keys
            | Self::LengthGetter
            | Self::Of
            | Self::Reverse
            | Self::ToLocaleString
            | Self::ToBase64
            | Self::ToHex
            | Self::ToReversed
            | Self::ToStringTagGetter
            | Self::Values => 0.0,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::At => "at",
            Self::BufferGetter => "get buffer",
            Self::ByteLengthGetter => "get byteLength",
            Self::ByteOffsetGetter => "get byteOffset",
            Self::CopyWithin => "copyWithin",
            Self::Entries => "entries",
            Self::Every => "every",
            Self::Fill => "fill",
            Self::Filter => "filter",
            Self::Find => "find",
            Self::FindIndex => "findIndex",
            Self::FindLast => "findLast",
            Self::FindLastIndex => "findLastIndex",
            Self::ForEach => "forEach",
            Self::From => "from",
            Self::FromBase64 => "fromBase64",
            Self::FromHex => "fromHex",
            Self::Includes => "includes",
            Self::IndexOf => "indexOf",
            Self::Join => "join",
            Self::Keys => "keys",
            Self::LastIndexOf => "lastIndexOf",
            Self::LengthGetter => "get length",
            Self::Map => "map",
            Self::Of => "of",
            Self::Reduce => "reduce",
            Self::ReduceRight => "reduceRight",
            Self::Reverse => "reverse",
            Self::Set => "set",
            Self::SetFromBase64 => "setFromBase64",
            Self::SetFromHex => "setFromHex",
            Self::Slice => "slice",
            Self::Some => "some",
            Self::Sort => "sort",
            Self::Subarray => "subarray",
            Self::ToLocaleString => "toLocaleString",
            Self::ToBase64 => "toBase64",
            Self::ToHex => "toHex",
            Self::ToReversed => "toReversed",
            Self::ToSorted => "toSorted",
            Self::ToStringTagGetter => "get [Symbol.toStringTag]",
            Self::Values => "values",
            Self::With => "with",
        }
    }
}
