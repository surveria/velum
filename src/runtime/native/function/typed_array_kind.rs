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
    Slice,
    Some,
    Sort,
    Subarray,
    ToLocaleString,
    ToReversed,
    ToSorted,
    ToString,
    ToStringTagGetter,
    Values,
    With,
}

impl TypedArrayFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::CopyWithin | Self::Slice | Self::With => 2.0,
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
            | Self::Includes
            | Self::IndexOf
            | Self::Join
            | Self::LastIndexOf
            | Self::Map
            | Self::Reduce
            | Self::ReduceRight
            | Self::Set
            | Self::Some
            | Self::Sort
            | Self::Subarray
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
            | Self::ToReversed
            | Self::ToString
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
            Self::Slice => "slice",
            Self::Some => "some",
            Self::Sort => "sort",
            Self::Subarray => "subarray",
            Self::ToLocaleString => "toLocaleString",
            Self::ToReversed => "toReversed",
            Self::ToSorted => "toSorted",
            Self::ToString => "toString",
            Self::ToStringTagGetter => "get [Symbol.toStringTag]",
            Self::Values => "values",
            Self::With => "with",
        }
    }
}
