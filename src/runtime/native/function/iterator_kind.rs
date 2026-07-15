use crate::{runtime::collections::CollectionIteratorId, value::ObjectId};

pub(in crate::runtime) const ITERATOR_NAME: &str = "Iterator";
pub(in crate::runtime::native) const ITERATOR_CONCAT_NAME: &str = "concat";
pub(in crate::runtime::native) const ITERATOR_FROM_NAME: &str = "from";
pub(in crate::runtime::native) const ITERATOR_ZIP_KEYED_NAME: &str = "zipKeyed";
pub(in crate::runtime::native) const ITERATOR_ZIP_NAME: &str = "zip";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_MAP_NAME: &str = "map";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_FILTER_NAME: &str = "filter";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_TAKE_NAME: &str = "take";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_DROP_NAME: &str = "drop";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_FLAT_MAP_NAME: &str = "flatMap";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_REDUCE_NAME: &str = "reduce";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_TO_ARRAY_NAME: &str = "toArray";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_FOR_EACH_NAME: &str = "forEach";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_SOME_NAME: &str = "some";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_EVERY_NAME: &str = "every";
pub(in crate::runtime::native) const ITERATOR_PROTOTYPE_FIND_NAME: &str = "find";
const ITERATOR_HELPER_NEXT_NAME: &str = "next";
const ITERATOR_HELPER_RETURN_NAME: &str = "return";
const ITERATOR_PROTOTYPE_CONSTRUCTOR_GETTER_NAME: &str = "get constructor";
const ITERATOR_PROTOTYPE_CONSTRUCTOR_SETTER_NAME: &str = "set constructor";
const ITERATOR_PROTOTYPE_DISPOSE_NAME: &str = "[Symbol.dispose]";
const ITERATOR_PROTOTYPE_TO_STRING_TAG_GETTER_NAME: &str = "get [Symbol.toStringTag]";
const ITERATOR_PROTOTYPE_TO_STRING_TAG_SETTER_NAME: &str = "set [Symbol.toStringTag]";

const ITERATOR_UNARY_LENGTH: f64 = 1.0;
const ITERATOR_NULLARY_LENGTH: f64 = 0.0;

/// Native function kinds owned by the `Iterator` global and its helper
/// objects. Helper prototype methods recover per-instance state from their
/// receiver; state-token variants retain ids in the collection arena.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum IteratorFunctionKind {
    Constructor,
    From {
        helper_prototype: ObjectId,
        wrapped_prototype: ObjectId,
        collection_prototype: ObjectId,
    },
    Concat,
    Zip,
    ZipKeyed,
    PrototypeMap,
    PrototypeFilter,
    PrototypeTake,
    PrototypeDrop,
    PrototypeFlatMap,
    PrototypeReduce,
    PrototypeToArray,
    PrototypeForEach,
    PrototypeSome,
    PrototypeEvery,
    PrototypeFind,
    PrototypeDispose,
    PrototypeConstructorGetter,
    PrototypeConstructorSetter,
    PrototypeToStringTagGetter,
    PrototypeToStringTagSetter,
    HelperPrototypeNext,
    HelperPrototypeReturn,
    HelperNext(CollectionIteratorId),
    StaticNext(CollectionIteratorId),
    StaticReturn(CollectionIteratorId),
    WrapNext(CollectionIteratorId),
    WrapReturn(CollectionIteratorId),
}

impl IteratorFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Constructor
            | Self::Concat
            | Self::PrototypeToArray
            | Self::PrototypeDispose
            | Self::PrototypeConstructorGetter
            | Self::PrototypeToStringTagGetter
            | Self::HelperPrototypeNext
            | Self::HelperPrototypeReturn
            | Self::HelperNext(_)
            | Self::StaticNext(_)
            | Self::StaticReturn(_)
            | Self::WrapNext(_)
            | Self::WrapReturn(_) => ITERATOR_NULLARY_LENGTH,
            Self::From { .. }
            | Self::Zip
            | Self::ZipKeyed
            | Self::PrototypeMap
            | Self::PrototypeFilter
            | Self::PrototypeTake
            | Self::PrototypeDrop
            | Self::PrototypeFlatMap
            | Self::PrototypeReduce
            | Self::PrototypeForEach
            | Self::PrototypeSome
            | Self::PrototypeEvery
            | Self::PrototypeFind
            | Self::PrototypeConstructorSetter
            | Self::PrototypeToStringTagSetter => ITERATOR_UNARY_LENGTH,
        }
    }

    pub(in crate::runtime::native) const fn name(self) -> &'static str {
        match self {
            Self::Constructor => ITERATOR_NAME,
            Self::Concat => ITERATOR_CONCAT_NAME,
            Self::From { .. } => ITERATOR_FROM_NAME,
            Self::Zip => ITERATOR_ZIP_NAME,
            Self::ZipKeyed => ITERATOR_ZIP_KEYED_NAME,
            Self::PrototypeMap => ITERATOR_PROTOTYPE_MAP_NAME,
            Self::PrototypeFilter => ITERATOR_PROTOTYPE_FILTER_NAME,
            Self::PrototypeTake => ITERATOR_PROTOTYPE_TAKE_NAME,
            Self::PrototypeDrop => ITERATOR_PROTOTYPE_DROP_NAME,
            Self::PrototypeFlatMap => ITERATOR_PROTOTYPE_FLAT_MAP_NAME,
            Self::PrototypeReduce => ITERATOR_PROTOTYPE_REDUCE_NAME,
            Self::PrototypeToArray => ITERATOR_PROTOTYPE_TO_ARRAY_NAME,
            Self::PrototypeForEach => ITERATOR_PROTOTYPE_FOR_EACH_NAME,
            Self::PrototypeSome => ITERATOR_PROTOTYPE_SOME_NAME,
            Self::PrototypeEvery => ITERATOR_PROTOTYPE_EVERY_NAME,
            Self::PrototypeFind => ITERATOR_PROTOTYPE_FIND_NAME,
            Self::PrototypeDispose => ITERATOR_PROTOTYPE_DISPOSE_NAME,
            Self::PrototypeConstructorGetter => ITERATOR_PROTOTYPE_CONSTRUCTOR_GETTER_NAME,
            Self::PrototypeConstructorSetter => ITERATOR_PROTOTYPE_CONSTRUCTOR_SETTER_NAME,
            Self::PrototypeToStringTagGetter => ITERATOR_PROTOTYPE_TO_STRING_TAG_GETTER_NAME,
            Self::PrototypeToStringTagSetter => ITERATOR_PROTOTYPE_TO_STRING_TAG_SETTER_NAME,
            Self::HelperPrototypeNext
            | Self::HelperNext(_)
            | Self::StaticNext(_)
            | Self::WrapNext(_) => ITERATOR_HELPER_NEXT_NAME,
            Self::HelperPrototypeReturn | Self::StaticReturn(_) | Self::WrapReturn(_) => {
                ITERATOR_HELPER_RETURN_NAME
            }
        }
    }

    /// Per-instance helper functions carry live state ids and must never be
    /// deduplicated through the registry slot table.
    pub(in crate::runtime::native) const fn state_id(self) -> Option<CollectionIteratorId> {
        match self {
            Self::HelperNext(id)
            | Self::StaticNext(id)
            | Self::StaticReturn(id)
            | Self::WrapNext(id)
            | Self::WrapReturn(id) => Some(id),
            Self::Constructor
            | Self::Concat
            | Self::From { .. }
            | Self::Zip
            | Self::ZipKeyed
            | Self::PrototypeMap
            | Self::PrototypeFilter
            | Self::PrototypeTake
            | Self::PrototypeDrop
            | Self::PrototypeFlatMap
            | Self::PrototypeReduce
            | Self::PrototypeToArray
            | Self::PrototypeForEach
            | Self::PrototypeSome
            | Self::PrototypeEvery
            | Self::PrototypeFind
            | Self::PrototypeDispose
            | Self::PrototypeConstructorGetter
            | Self::PrototypeConstructorSetter
            | Self::PrototypeToStringTagGetter
            | Self::PrototypeToStringTagSetter
            | Self::HelperPrototypeNext
            | Self::HelperPrototypeReturn => None,
        }
    }

    pub(in crate::runtime) const fn prototype_anchors(self) -> Option<[ObjectId; 3]> {
        match self {
            Self::From {
                helper_prototype,
                wrapped_prototype,
                collection_prototype,
            } => Some([helper_prototype, wrapped_prototype, collection_prototype]),
            _ => None,
        }
    }
}
