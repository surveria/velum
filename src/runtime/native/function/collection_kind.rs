use super::kind::NativeFunctionKind;

const MAP_NAME: &str = "Map";
const SET_NAME: &str = "Set";
pub(in crate::runtime::native) const WEAK_MAP_NAME: &str = "WeakMap";
pub(in crate::runtime::native) const WEAK_SET_NAME: &str = "WeakSet";
const COLLECTION_METHOD_GET_NAME: &str = "get";
const COLLECTION_METHOD_SET_NAME: &str = "set";
const COLLECTION_METHOD_ADD_NAME: &str = "add";
const COLLECTION_METHOD_HAS_NAME: &str = "has";
const COLLECTION_METHOD_DELETE_NAME: &str = "delete";
const COLLECTION_METHOD_CLEAR_NAME: &str = "clear";
const COLLECTION_METHOD_FOR_EACH_NAME: &str = "forEach";
const COLLECTION_METHOD_ENTRIES_NAME: &str = "entries";
const COLLECTION_METHOD_KEYS_NAME: &str = "keys";
const COLLECTION_METHOD_VALUES_NAME: &str = "values";
const COLLECTION_SIZE_GETTER_NAME: &str = "size";
const COLLECTION_ITERATOR_NEXT_NAME: &str = "next";
const COLLECTION_ITERATOR_SELF_NAME: &str = "[Symbol.iterator]";

impl NativeFunctionKind {
    pub(in crate::runtime::native::function) const fn collection_length(self) -> Option<f64> {
        match self {
            Self::Map
            | Self::Set
            | Self::SetSizeGetter
            | Self::MapSizeGetter
            | Self::MapClear
            | Self::SetClear
            | Self::MapEntries
            | Self::MapKeys
            | Self::MapValues
            | Self::SetEntries
            | Self::SetValues
            | Self::CollectionIteratorNext(_)
            | Self::IteratorSelf
            | Self::WeakMap
            | Self::WeakSet => Some(0.0),
            Self::MapGet
            | Self::WeakMapGet
            | Self::MapHas
            | Self::WeakMapHas
            | Self::MapDelete
            | Self::WeakMapDelete
            | Self::MapForEach
            | Self::SetAdd
            | Self::WeakSetAdd
            | Self::SetHas
            | Self::WeakSetHas
            | Self::SetDelete
            | Self::WeakSetDelete
            | Self::SetForEach => Some(1.0),
            Self::MapSet | Self::WeakMapSet => Some(2.0),
            _ => None,
        }
    }

    pub(in crate::runtime::native::function) const fn collection_name(
        self,
    ) -> Option<&'static str> {
        match self {
            Self::Map => Some(MAP_NAME),
            Self::Set => Some(SET_NAME),
            Self::WeakMap => Some(WEAK_MAP_NAME),
            Self::WeakSet => Some(WEAK_SET_NAME),
            Self::MapGet | Self::WeakMapGet => Some(COLLECTION_METHOD_GET_NAME),
            Self::MapSet | Self::WeakMapSet => Some(COLLECTION_METHOD_SET_NAME),
            Self::SetAdd | Self::WeakSetAdd => Some(COLLECTION_METHOD_ADD_NAME),
            Self::MapHas | Self::SetHas | Self::WeakMapHas | Self::WeakSetHas => {
                Some(COLLECTION_METHOD_HAS_NAME)
            }
            Self::MapDelete | Self::SetDelete | Self::WeakMapDelete | Self::WeakSetDelete => {
                Some(COLLECTION_METHOD_DELETE_NAME)
            }
            Self::MapClear | Self::SetClear => Some(COLLECTION_METHOD_CLEAR_NAME),
            Self::MapForEach | Self::SetForEach => Some(COLLECTION_METHOD_FOR_EACH_NAME),
            Self::MapEntries | Self::SetEntries => Some(COLLECTION_METHOD_ENTRIES_NAME),
            Self::MapKeys => Some(COLLECTION_METHOD_KEYS_NAME),
            Self::MapValues | Self::SetValues => Some(COLLECTION_METHOD_VALUES_NAME),
            Self::MapSizeGetter | Self::SetSizeGetter => Some(COLLECTION_SIZE_GETTER_NAME),
            Self::CollectionIteratorNext(_) => Some(COLLECTION_ITERATOR_NEXT_NAME),
            Self::IteratorSelf => Some(COLLECTION_ITERATOR_SELF_NAME),
            _ => None,
        }
    }
}
