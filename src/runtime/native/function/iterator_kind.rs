use crate::{runtime::collections::CollectionIteratorId, value::ObjectId};

pub(in crate::runtime) const ITERATOR_NAME: &str = "Iterator";
pub(in crate::runtime::native) const ITERATOR_FROM_NAME: &str = "from";
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
/// objects. Helper `next`/`return` variants carry the per-instance state id
/// in the same arena that backs collection iterator snapshots.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum IteratorFunctionKind {
    Constructor,
    From {
        helper_prototype: ObjectId,
        wrapped_prototype: ObjectId,
    },
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
    HelperNext(CollectionIteratorId),
    HelperReturn(CollectionIteratorId),
    WrapNext(CollectionIteratorId),
    WrapReturn(CollectionIteratorId),
}

impl IteratorFunctionKind {
    pub(in crate::runtime::native) const fn length(self) -> f64 {
        match self {
            Self::Constructor
            | Self::PrototypeToArray
            | Self::PrototypeDispose
            | Self::PrototypeConstructorGetter
            | Self::PrototypeToStringTagGetter
            | Self::HelperNext(_)
            | Self::HelperReturn(_)
            | Self::WrapNext(_)
            | Self::WrapReturn(_) => ITERATOR_NULLARY_LENGTH,
            Self::From { .. }
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
            Self::From { .. } => ITERATOR_FROM_NAME,
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
            Self::HelperNext(_) | Self::WrapNext(_) => ITERATOR_HELPER_NEXT_NAME,
            Self::HelperReturn(_) | Self::WrapReturn(_) => ITERATOR_HELPER_RETURN_NAME,
        }
    }

    /// Per-instance helper functions carry live state ids and must never be
    /// deduplicated through the registry slot table.
    pub(in crate::runtime::native) const fn state_id(self) -> Option<CollectionIteratorId> {
        match self {
            Self::HelperNext(id)
            | Self::HelperReturn(id)
            | Self::WrapNext(id)
            | Self::WrapReturn(id) => Some(id),
            Self::Constructor
            | Self::From { .. }
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
            | Self::PrototypeToStringTagSetter => None,
        }
    }

    pub(in crate::runtime) const fn prototype_anchors(self) -> Option<(ObjectId, ObjectId)> {
        match self {
            Self::From {
                helper_prototype,
                wrapped_prototype,
            } => Some((helper_prototype, wrapped_prototype)),
            _ => None,
        }
    }
}
