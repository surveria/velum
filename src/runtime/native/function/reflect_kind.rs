use super::kind::NativeFunctionKind;

pub(in crate::runtime::native) const REFLECT_NAME: &str = "Reflect";
const REFLECT_APPLY_NAME: &str = "apply";
const REFLECT_CONSTRUCT_NAME: &str = "construct";
const REFLECT_DEFINE_PROPERTY_NAME: &str = "defineProperty";
const REFLECT_DELETE_PROPERTY_NAME: &str = "deleteProperty";
const REFLECT_GET_NAME: &str = "get";
const REFLECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME: &str = "getOwnPropertyDescriptor";
const REFLECT_GET_PROTOTYPE_OF_NAME: &str = "getPrototypeOf";
const REFLECT_HAS_NAME: &str = "has";
const REFLECT_IS_EXTENSIBLE_NAME: &str = "isExtensible";
const REFLECT_OWN_KEYS_NAME: &str = "ownKeys";
const REFLECT_PREVENT_EXTENSIONS_NAME: &str = "preventExtensions";
const REFLECT_SET_NAME: &str = "set";
const REFLECT_SET_PROTOTYPE_OF_NAME: &str = "setPrototypeOf";
const REFLECT_LENGTH_ONE: f64 = 1.0;
const REFLECT_LENGTH_TWO: f64 = 2.0;
const REFLECT_LENGTH_THREE: f64 = 3.0;

impl NativeFunctionKind {
    pub(in crate::runtime::native::function) const fn reflect_length(self) -> Option<f64> {
        match self {
            Self::ReflectGetPrototypeOf
            | Self::ReflectIsExtensible
            | Self::ReflectOwnKeys
            | Self::ReflectPreventExtensions => Some(REFLECT_LENGTH_ONE),
            Self::ReflectConstruct
            | Self::ReflectDeleteProperty
            | Self::ReflectGet
            | Self::ReflectGetOwnPropertyDescriptor
            | Self::ReflectHas
            | Self::ReflectSetPrototypeOf => Some(REFLECT_LENGTH_TWO),
            Self::ReflectApply | Self::ReflectDefineProperty | Self::ReflectSet => {
                Some(REFLECT_LENGTH_THREE)
            }
            _ => None,
        }
    }

    pub(in crate::runtime::native::function) const fn reflect_name(self) -> Option<&'static str> {
        match self {
            Self::ReflectApply => Some(REFLECT_APPLY_NAME),
            Self::ReflectConstruct => Some(REFLECT_CONSTRUCT_NAME),
            Self::ReflectDefineProperty => Some(REFLECT_DEFINE_PROPERTY_NAME),
            Self::ReflectDeleteProperty => Some(REFLECT_DELETE_PROPERTY_NAME),
            Self::ReflectGet => Some(REFLECT_GET_NAME),
            Self::ReflectGetOwnPropertyDescriptor => Some(REFLECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME),
            Self::ReflectGetPrototypeOf => Some(REFLECT_GET_PROTOTYPE_OF_NAME),
            Self::ReflectHas => Some(REFLECT_HAS_NAME),
            Self::ReflectIsExtensible => Some(REFLECT_IS_EXTENSIBLE_NAME),
            Self::ReflectOwnKeys => Some(REFLECT_OWN_KEYS_NAME),
            Self::ReflectPreventExtensions => Some(REFLECT_PREVENT_EXTENSIONS_NAME),
            Self::ReflectSet => Some(REFLECT_SET_NAME),
            Self::ReflectSetPrototypeOf => Some(REFLECT_SET_PROTOTYPE_OF_NAME),
            _ => None,
        }
    }
}
