use super::kind::NativeFunctionKind;

const OBJECT_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_ASSIGN_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_CREATE_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_DEFINE_PROPERTIES_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_DEFINE_PROPERTY_FUNCTION_LENGTH: f64 = 3.0;
const OBJECT_ENTRIES_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_FREEZE_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_GET_PROTOTYPE_OF_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_GET_OWN_PROPERTY_NAMES_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_GET_OWN_PROPERTY_SYMBOLS_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_HAS_OWN_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_IS_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_IS_EXTENSIBLE_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_IS_FROZEN_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_IS_SEALED_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_KEYS_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_PREVENT_EXTENSIONS_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_FROM_ENTRIES_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_PROTOTYPE_TO_STRING_FUNCTION_LENGTH: f64 = 0.0;
const OBJECT_SET_PROTOTYPE_OF_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_SEAL_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_VALUES_FUNCTION_LENGTH: f64 = 1.0;

impl NativeFunctionKind {
    pub(in crate::runtime::native::function) const fn object_length(self) -> Option<f64> {
        match self {
            Self::Object => Some(OBJECT_FUNCTION_LENGTH),
            Self::ObjectAssign => Some(OBJECT_ASSIGN_FUNCTION_LENGTH),
            Self::ObjectCreate => Some(OBJECT_CREATE_FUNCTION_LENGTH),
            Self::ObjectDefineProperties => Some(OBJECT_DEFINE_PROPERTIES_FUNCTION_LENGTH),
            Self::ObjectDefineProperty => Some(OBJECT_DEFINE_PROPERTY_FUNCTION_LENGTH),
            Self::ObjectEntries => Some(OBJECT_ENTRIES_FUNCTION_LENGTH),
            Self::ObjectFreeze => Some(OBJECT_FREEZE_FUNCTION_LENGTH),
            Self::ObjectGetPrototypeOf => Some(OBJECT_GET_PROTOTYPE_OF_FUNCTION_LENGTH),
            Self::ObjectGetOwnPropertyDescriptor => {
                Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_FUNCTION_LENGTH)
            }
            Self::ObjectGetOwnPropertyDescriptors => {
                Some(OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_FUNCTION_LENGTH)
            }
            Self::ObjectGetOwnPropertyNames => Some(OBJECT_GET_OWN_PROPERTY_NAMES_FUNCTION_LENGTH),
            Self::ObjectGetOwnPropertySymbols => {
                Some(OBJECT_GET_OWN_PROPERTY_SYMBOLS_FUNCTION_LENGTH)
            }
            Self::ObjectHasOwn => Some(OBJECT_HAS_OWN_FUNCTION_LENGTH),
            Self::ObjectIs => Some(OBJECT_IS_FUNCTION_LENGTH),
            Self::ObjectIsExtensible => Some(OBJECT_IS_EXTENSIBLE_FUNCTION_LENGTH),
            Self::ObjectIsFrozen => Some(OBJECT_IS_FROZEN_FUNCTION_LENGTH),
            Self::ObjectIsSealed => Some(OBJECT_IS_SEALED_FUNCTION_LENGTH),
            Self::ObjectKeys => Some(OBJECT_KEYS_FUNCTION_LENGTH),
            Self::ObjectPreventExtensions => Some(OBJECT_PREVENT_EXTENSIONS_FUNCTION_LENGTH),
            Self::ObjectPrototypeHasOwnProperty => {
                Some(OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_FUNCTION_LENGTH)
            }
            Self::ObjectPrototypePropertyIsEnumerable => {
                Some(OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_FUNCTION_LENGTH)
            }
            Self::ObjectPrototypeToString
            | Self::ObjectPrototypeValueOf
            | Self::ObjectPrototypeToLocaleString => {
                Some(OBJECT_PROTOTYPE_TO_STRING_FUNCTION_LENGTH)
            }
            Self::ObjectPrototypeIsPrototypeOf => {
                Some(OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_FUNCTION_LENGTH)
            }
            Self::ObjectFromEntries => Some(OBJECT_FROM_ENTRIES_FUNCTION_LENGTH),
            Self::ObjectSetPrototypeOf => Some(OBJECT_SET_PROTOTYPE_OF_FUNCTION_LENGTH),
            Self::ObjectSeal => Some(OBJECT_SEAL_FUNCTION_LENGTH),
            Self::ObjectValues => Some(OBJECT_VALUES_FUNCTION_LENGTH),
            _ => None,
        }
    }
}
