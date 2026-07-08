use super::kind::NativeFunctionKind;

const MAP_NAME: &str = "Map";
const SET_NAME: &str = "Set";
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
            | Self::IteratorSelf => Some(0.0),
            Self::MapGet
            | Self::MapHas
            | Self::MapDelete
            | Self::MapForEach
            | Self::SetAdd
            | Self::SetHas
            | Self::SetDelete
            | Self::SetForEach => Some(1.0),
            Self::MapSet => Some(2.0),
            _ => None,
        }
    }

    pub(in crate::runtime::native::function) const fn collection_name(
        self,
    ) -> Option<&'static str> {
        match self {
            Self::Map => Some(MAP_NAME),
            Self::Set => Some(SET_NAME),
            Self::MapGet => Some(COLLECTION_METHOD_GET_NAME),
            Self::MapSet => Some(COLLECTION_METHOD_SET_NAME),
            Self::SetAdd => Some(COLLECTION_METHOD_ADD_NAME),
            Self::MapHas | Self::SetHas => Some(COLLECTION_METHOD_HAS_NAME),
            Self::MapDelete | Self::SetDelete => Some(COLLECTION_METHOD_DELETE_NAME),
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
