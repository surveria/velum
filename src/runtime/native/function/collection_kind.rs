use super::kind::NativeFunctionKind;

const MAP_NAME: &str = "Map";
const FINALIZATION_REGISTRY_NAME: &str = "FinalizationRegistry";
const FINALIZATION_REGISTRY_REGISTER_NAME: &str = "register";
const FINALIZATION_REGISTRY_UNREGISTER_NAME: &str = "unregister";
const WEAK_REF_NAME: &str = "WeakRef";
const WEAK_REF_DEREF_NAME: &str = "deref";
const MAP_GROUP_BY_NAME: &str = "groupBy";
const MAP_GET_OR_INSERT_NAME: &str = "getOrInsert";
const MAP_GET_OR_INSERT_COMPUTED_NAME: &str = "getOrInsertComputed";
const WEAK_MAP_GET_OR_INSERT_NAME: &str = "getOrInsert";
const WEAK_MAP_GET_OR_INSERT_COMPUTED_NAME: &str = "getOrInsertComputed";
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
const COLLECTION_SIZE_GETTER_NAME: &str = "get size";
const SET_UNION_NAME: &str = "union";
const SET_INTERSECTION_NAME: &str = "intersection";
const SET_DIFFERENCE_NAME: &str = "difference";
const SET_SYMMETRIC_DIFFERENCE_NAME: &str = "symmetricDifference";
const SET_IS_SUBSET_OF_NAME: &str = "isSubsetOf";
const SET_IS_SUPERSET_OF_NAME: &str = "isSupersetOf";
const SET_IS_DISJOINT_FROM_NAME: &str = "isDisjointFrom";
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
            | Self::WeakSet
            | Self::WeakRefDeref => Some(0.0),
            Self::FinalizationRegistry | Self::FinalizationRegistryUnregister | Self::WeakRef => {
                Some(1.0)
            }
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
            | Self::SetForEach
            | Self::SetUnion
            | Self::SetIntersection
            | Self::SetDifference
            | Self::SetSymmetricDifference
            | Self::SetIsSubsetOf
            | Self::SetIsSupersetOf
            | Self::SetIsDisjointFrom => Some(1.0),
            Self::MapSet
            | Self::WeakMapSet
            | Self::MapGroupBy
            | Self::MapGetOrInsert
            | Self::MapGetOrInsertComputed
            | Self::WeakMapGetOrInsert
            | Self::WeakMapGetOrInsertComputed => Some(2.0),
            Self::FinalizationRegistryRegister => Some(2.0),
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
            Self::FinalizationRegistry => Some(FINALIZATION_REGISTRY_NAME),
            Self::FinalizationRegistryRegister => Some(FINALIZATION_REGISTRY_REGISTER_NAME),
            Self::FinalizationRegistryUnregister => Some(FINALIZATION_REGISTRY_UNREGISTER_NAME),
            Self::WeakRef => Some(WEAK_REF_NAME),
            Self::WeakRefDeref => Some(WEAK_REF_DEREF_NAME),
            Self::MapGet | Self::WeakMapGet => Some(COLLECTION_METHOD_GET_NAME),
            Self::MapGroupBy => Some(MAP_GROUP_BY_NAME),
            Self::MapGetOrInsert => Some(MAP_GET_OR_INSERT_NAME),
            Self::MapGetOrInsertComputed => Some(MAP_GET_OR_INSERT_COMPUTED_NAME),
            Self::WeakMapGetOrInsert => Some(WEAK_MAP_GET_OR_INSERT_NAME),
            Self::WeakMapGetOrInsertComputed => Some(WEAK_MAP_GET_OR_INSERT_COMPUTED_NAME),
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
            Self::SetUnion => Some(SET_UNION_NAME),
            Self::SetIntersection => Some(SET_INTERSECTION_NAME),
            Self::SetDifference => Some(SET_DIFFERENCE_NAME),
            Self::SetSymmetricDifference => Some(SET_SYMMETRIC_DIFFERENCE_NAME),
            Self::SetIsSubsetOf => Some(SET_IS_SUBSET_OF_NAME),
            Self::SetIsSupersetOf => Some(SET_IS_SUPERSET_OF_NAME),
            Self::SetIsDisjointFrom => Some(SET_IS_DISJOINT_FROM_NAME),
            _ => None,
        }
    }
}
