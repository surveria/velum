use crate::{
    error::Result,
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

use super::kind::NativeFunctionKind;

const OBJECT_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_ASSIGN_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_CREATE_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_DEFINE_PROPERTIES_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_DEFINE_PROPERTY_FUNCTION_LENGTH: f64 = 3.0;
const OBJECT_ENTRIES_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_FREEZE_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_GROUP_BY_FUNCTION_LENGTH: f64 = 2.0;
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
const OBJECT_PROTOTYPE_DEFINE_ACCESSOR_FUNCTION_LENGTH: f64 = 2.0;
const OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_PROTOTYPE_LOOKUP_ACCESSOR_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_FUNCTION_LENGTH: f64 = 1.0;
const OBJECT_PROTOTYPE_PROTO_GETTER_FUNCTION_LENGTH: f64 = 0.0;
const OBJECT_PROTOTYPE_PROTO_SETTER_FUNCTION_LENGTH: f64 = 1.0;
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
            Self::ObjectGroupBy => Some(OBJECT_GROUP_BY_FUNCTION_LENGTH),
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
            Self::ObjectPrototypeDefineGetter | Self::ObjectPrototypeDefineSetter => {
                Some(OBJECT_PROTOTYPE_DEFINE_ACCESSOR_FUNCTION_LENGTH)
            }
            Self::ObjectPrototypeHasOwnProperty => {
                Some(OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_FUNCTION_LENGTH)
            }
            Self::ObjectPrototypeLookupGetter | Self::ObjectPrototypeLookupSetter => {
                Some(OBJECT_PROTOTYPE_LOOKUP_ACCESSOR_FUNCTION_LENGTH)
            }
            Self::ObjectPrototypePropertyIsEnumerable => {
                Some(OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_FUNCTION_LENGTH)
            }
            Self::ObjectPrototypeProtoGetter => Some(OBJECT_PROTOTYPE_PROTO_GETTER_FUNCTION_LENGTH),
            Self::ObjectPrototypeProtoSetter => Some(OBJECT_PROTOTYPE_PROTO_SETTER_FUNCTION_LENGTH),
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

impl Context {
    pub(super) fn eval_object_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::Object => Some(self.eval_object_constructor(args)),
            NativeFunctionKind::ObjectAssign => Some(self.eval_object_assign(args)),
            NativeFunctionKind::ObjectCreate => Some(self.eval_object_create(args)),
            NativeFunctionKind::ObjectDefineProperties => {
                Some(self.eval_object_define_properties(args))
            }
            NativeFunctionKind::ObjectDefineProperty => {
                Some(self.eval_object_define_property(args))
            }
            NativeFunctionKind::ObjectEntries => Some(self.eval_object_entries(args)),
            NativeFunctionKind::ObjectFreeze => Some(self.eval_object_freeze(args)),
            NativeFunctionKind::ObjectGroupBy => Some(self.eval_object_group_by(args)),
            NativeFunctionKind::ObjectGetOwnPropertyDescriptor => {
                Some(self.eval_object_get_own_property_descriptor(args))
            }
            NativeFunctionKind::ObjectGetOwnPropertyDescriptors => {
                Some(self.eval_object_get_own_property_descriptors(args))
            }
            NativeFunctionKind::ObjectGetOwnPropertyNames => {
                Some(self.eval_object_get_own_property_names(args))
            }
            NativeFunctionKind::ObjectGetOwnPropertySymbols => {
                Some(self.eval_object_get_own_property_symbols(args))
            }
            NativeFunctionKind::ObjectGetPrototypeOf => {
                Some(self.eval_object_get_prototype_of(args))
            }
            NativeFunctionKind::ObjectHasOwn => Some(self.eval_object_has_own(args)),
            NativeFunctionKind::ObjectIs => Some(Ok(Self::eval_direct_object_is(args.as_slice()))),
            NativeFunctionKind::ObjectIsExtensible => Some(self.eval_object_is_extensible(args)),
            NativeFunctionKind::ObjectIsFrozen => Some(self.eval_object_is_frozen(args)),
            NativeFunctionKind::ObjectIsSealed => Some(self.eval_object_is_sealed(args)),
            NativeFunctionKind::ObjectKeys => Some(self.eval_object_keys(args)),
            NativeFunctionKind::ObjectPreventExtensions => {
                Some(self.eval_object_prevent_extensions(args))
            }
            NativeFunctionKind::ObjectPrototypeDefineGetter => {
                Some(self.eval_object_prototype_define_getter(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeDefineSetter => {
                Some(self.eval_object_prototype_define_setter(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeHasOwnProperty => {
                Some(self.eval_object_prototype_has_own_property(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeLookupGetter => {
                Some(self.eval_object_prototype_lookup_getter(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeLookupSetter => {
                Some(self.eval_object_prototype_lookup_setter(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypePropertyIsEnumerable => {
                Some(self.eval_object_prototype_property_is_enumerable(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeProtoGetter => {
                Some(self.eval_object_prototype_proto_getter(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeProtoSetter => {
                Some(self.eval_object_prototype_proto_setter(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeToString => {
                Some(self.eval_object_prototype_to_string(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeValueOf => {
                Some(self.eval_object_prototype_value_of(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeToLocaleString => {
                Some(self.eval_object_prototype_to_locale_string(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypeIsPrototypeOf => {
                Some(self.eval_object_prototype_is_prototype_of(args, this_value))
            }
            NativeFunctionKind::ObjectFromEntries => Some(self.eval_object_from_entries(args)),
            NativeFunctionKind::ObjectSetPrototypeOf => {
                Some(self.eval_object_set_prototype_of(args))
            }
            NativeFunctionKind::ObjectSeal => Some(self.eval_object_seal(args)),
            NativeFunctionKind::ObjectValues => Some(self.eval_object_values(args)),
            _ => None,
        }
    }
}
