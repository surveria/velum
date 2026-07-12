use super::{
    NativeFunctionKind, NativeFunctionSlot, OBJECT_ASSIGN_SLOT, OBJECT_CREATE_SLOT,
    OBJECT_DEFINE_PROPERTIES_SLOT, OBJECT_DEFINE_PROPERTY_SLOT, OBJECT_ENTRIES_SLOT,
    OBJECT_FREEZE_SLOT, OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_SLOT,
    OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_SLOT, OBJECT_GET_PROTOTYPE_OF_SLOT, OBJECT_HAS_OWN_SLOT,
    OBJECT_IS_EXTENSIBLE_SLOT, OBJECT_IS_FROZEN_SLOT, OBJECT_IS_SEALED_SLOT, OBJECT_IS_SLOT,
    OBJECT_KEYS_SLOT, OBJECT_PREVENT_EXTENSIONS_SLOT, OBJECT_SEAL_SLOT,
    OBJECT_SET_PROTOTYPE_OF_SLOT, OBJECT_SLOT, OBJECT_VALUES_SLOT,
};

pub(super) const fn object_slot(kind: NativeFunctionKind) -> Option<NativeFunctionSlot> {
    match kind {
        NativeFunctionKind::Object => Some(OBJECT_SLOT),
        NativeFunctionKind::ObjectAssign => Some(OBJECT_ASSIGN_SLOT),
        NativeFunctionKind::ObjectCreate => Some(OBJECT_CREATE_SLOT),
        NativeFunctionKind::ObjectDefineProperties => Some(OBJECT_DEFINE_PROPERTIES_SLOT),
        NativeFunctionKind::ObjectDefineProperty => Some(OBJECT_DEFINE_PROPERTY_SLOT),
        NativeFunctionKind::ObjectEntries => Some(OBJECT_ENTRIES_SLOT),
        NativeFunctionKind::ObjectFreeze => Some(OBJECT_FREEZE_SLOT),
        NativeFunctionKind::ObjectGetPrototypeOf => Some(OBJECT_GET_PROTOTYPE_OF_SLOT),
        NativeFunctionKind::ObjectGetOwnPropertyDescriptor => {
            Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_SLOT)
        }
        NativeFunctionKind::ObjectGetOwnPropertyDescriptors => {
            Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_SLOT)
        }
        NativeFunctionKind::ObjectHasOwn => Some(OBJECT_HAS_OWN_SLOT),
        NativeFunctionKind::ObjectIs => Some(OBJECT_IS_SLOT),
        NativeFunctionKind::ObjectIsExtensible => Some(OBJECT_IS_EXTENSIBLE_SLOT),
        NativeFunctionKind::ObjectIsFrozen => Some(OBJECT_IS_FROZEN_SLOT),
        NativeFunctionKind::ObjectIsSealed => Some(OBJECT_IS_SEALED_SLOT),
        NativeFunctionKind::ObjectKeys => Some(OBJECT_KEYS_SLOT),
        NativeFunctionKind::ObjectPreventExtensions => Some(OBJECT_PREVENT_EXTENSIONS_SLOT),
        NativeFunctionKind::ObjectSetPrototypeOf => Some(OBJECT_SET_PROTOTYPE_OF_SLOT),
        NativeFunctionKind::ObjectSeal => Some(OBJECT_SEAL_SLOT),
        NativeFunctionKind::ObjectValues => Some(OBJECT_VALUES_SLOT),
        _ => None,
    }
}
