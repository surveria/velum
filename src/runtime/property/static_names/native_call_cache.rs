use crate::{
    runtime::{native::NativeFunctionKind, object::CacheablePropertyLookup},
    value::NativeFunctionId,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct StaticPropertyNativeCallCache {
    pub(super) object_property: Option<StaticObjectPropertyNativeCallCache>,
    pub(super) function: NativeFunctionId,
    pub(super) kind: NativeFunctionKind,
}

impl StaticPropertyNativeCallCache {
    pub(super) const fn new(function: NativeFunctionId, kind: NativeFunctionKind) -> Self {
        Self {
            object_property: None,
            function,
            kind,
        }
    }

    pub(super) const fn new_object_property(
        lookup: CacheablePropertyLookup,
        version: u64,
        function: NativeFunctionId,
        kind: NativeFunctionKind,
    ) -> Self {
        Self {
            object_property: Some(StaticObjectPropertyNativeCallCache::new(lookup, version)),
            function,
            kind,
        }
    }

    pub(super) fn kind_if_current(self, function: NativeFunctionId) -> Option<NativeFunctionKind> {
        if self.function == function {
            return Some(self.kind);
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StaticObjectPropertyNativeCallCache {
    pub(super) lookup: CacheablePropertyLookup,
    pub(super) version: u64,
}

impl StaticObjectPropertyNativeCallCache {
    const fn new(lookup: CacheablePropertyLookup, version: u64) -> Self {
        Self { lookup, version }
    }
}
