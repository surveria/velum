use crate::{
    runtime::function::{FunctionIntrinsicDefaults, FunctionProperties},
    runtime::object::{
        DataPropertyDescriptor, PropertyConfigurable, PropertyEnumerable, PropertyWritable,
    },
    value::Value,
};

mod array;
mod array_kind;
mod call_target;
mod collection;
mod collection_kind;
mod date_kind;
mod direct;
mod kind;
mod object_kind;
mod primitive;
mod primitive_kind;
mod reflect_kind;
mod registry;
mod string_kind;

use super::number_intrinsic_property;

pub(in crate::runtime::native) use array_kind::ARRAY_NAME;
pub(in crate::runtime) use date_kind::{
    DATE_NAME, DATE_NOW_NAME, DATE_PARSE_NAME, DATE_UTC_NAME, DateFunctionKind,
};
pub(in crate::runtime) use kind::NativeFunctionKind;
pub(in crate::runtime::native) use kind::{
    BOOLEAN_NAME, EVAL_NAME, FUNCTION_NAME, FUNCTION_PROTOTYPE_APPLY_NAME,
    FUNCTION_PROTOTYPE_BIND_NAME, FUNCTION_PROTOTYPE_CALL_NAME, GLOBAL_DECODE_URI_COMPONENT_NAME,
    GLOBAL_DECODE_URI_NAME, GLOBAL_ENCODE_URI_COMPONENT_NAME, GLOBAL_ENCODE_URI_NAME,
    GLOBAL_IS_FINITE_NAME, GLOBAL_IS_NAN_NAME, GLOBAL_PARSE_FLOAT_NAME, GLOBAL_PARSE_INT_NAME,
    JSON_IS_RAW_JSON_NAME, JSON_NAME, JSON_PARSE_NAME, JSON_RAW_JSON_NAME, JSON_STRINGIFY_NAME,
    MATH_ABS_NAME, MATH_ACOS_NAME, MATH_ACOSH_NAME, MATH_ASIN_NAME, MATH_ASINH_NAME,
    MATH_ATAN_NAME, MATH_ATAN2_NAME, MATH_ATANH_NAME, MATH_CBRT_NAME, MATH_CEIL_NAME,
    MATH_CLZ32_NAME, MATH_COS_NAME, MATH_COSH_NAME, MATH_EXP_NAME, MATH_EXPM1_NAME,
    MATH_F16ROUND_NAME, MATH_FLOOR_NAME, MATH_FROUND_NAME, MATH_HYPOT_NAME, MATH_IMUL_NAME,
    MATH_LOG_NAME, MATH_LOG1P_NAME, MATH_LOG2_NAME, MATH_LOG10_NAME, MATH_MAX_NAME, MATH_MIN_NAME,
    MATH_NAME, MATH_POW_NAME, MATH_RANDOM_NAME, MATH_ROUND_NAME, MATH_SIGN_NAME, MATH_SIN_NAME,
    MATH_SINH_NAME, MATH_SQRT_NAME, MATH_SUM_PRECISE_NAME, MATH_TAN_NAME, MATH_TANH_NAME,
    MATH_TRUNC_NAME, NUMBER_IS_FINITE_NAME, NUMBER_IS_INTEGER_NAME, NUMBER_IS_NAN_NAME,
    NUMBER_IS_SAFE_INTEGER_NAME, NUMBER_NAME, OBJECT_ASSIGN_NAME, OBJECT_CREATE_NAME,
    OBJECT_DEFINE_PROPERTIES_NAME, OBJECT_DEFINE_PROPERTY_NAME, OBJECT_ENTRIES_NAME,
    OBJECT_FREEZE_NAME, OBJECT_FROM_ENTRIES_NAME, OBJECT_GET_OWN_PROPERTY_DESCRIPTOR_NAME,
    OBJECT_GET_OWN_PROPERTY_DESCRIPTORS_NAME, OBJECT_GET_OWN_PROPERTY_NAMES_NAME,
    OBJECT_GET_PROTOTYPE_OF_NAME, OBJECT_HAS_OWN_NAME, OBJECT_IS_EXTENSIBLE_NAME,
    OBJECT_IS_FROZEN_NAME, OBJECT_IS_NAME, OBJECT_IS_SEALED_NAME, OBJECT_KEYS_NAME, OBJECT_NAME,
    OBJECT_PREVENT_EXTENSIONS_NAME, OBJECT_SEAL_NAME, OBJECT_SET_PROTOTYPE_OF_NAME,
    OBJECT_VALUES_NAME, PROMISE_CATCH_NAME, PROMISE_NAME, PROMISE_REJECT_NAME,
    PROMISE_RESOLVE_NAME, PROMISE_THEN_NAME, PROXY_NAME, PROXY_REVOCABLE_NAME, REGEXP_NAME,
    REGEXP_PROTOTYPE_EXEC_NAME, REGEXP_PROTOTYPE_TEST_NAME, REGEXP_PROTOTYPE_TO_STRING_NAME,
    STRING_FROM_CHAR_CODE_NAME, STRING_FROM_CODE_POINT_NAME, STRING_NAME, STRING_PROTOTYPE_AT_NAME,
    STRING_PROTOTYPE_CHAR_AT_NAME, STRING_PROTOTYPE_CHAR_CODE_AT_NAME,
    STRING_PROTOTYPE_CODE_POINT_AT_NAME, STRING_PROTOTYPE_CONCAT_NAME,
    STRING_PROTOTYPE_ENDS_WITH_NAME, STRING_PROTOTYPE_INCLUDES_NAME,
    STRING_PROTOTYPE_INDEX_OF_NAME, STRING_PROTOTYPE_LAST_INDEX_OF_NAME,
    STRING_PROTOTYPE_MATCH_NAME, STRING_PROTOTYPE_PAD_END_NAME, STRING_PROTOTYPE_PAD_START_NAME,
    STRING_PROTOTYPE_REPEAT_NAME, STRING_PROTOTYPE_REPLACE_NAME, STRING_PROTOTYPE_SEARCH_NAME,
    STRING_PROTOTYPE_SLICE_NAME, STRING_PROTOTYPE_SPLIT_NAME, STRING_PROTOTYPE_STARTS_WITH_NAME,
    STRING_PROTOTYPE_SUBSTRING_NAME, STRING_PROTOTYPE_TO_LOCALE_LOWER_CASE_NAME,
    STRING_PROTOTYPE_TO_LOCALE_UPPER_CASE_NAME, STRING_PROTOTYPE_TO_LOWER_CASE_NAME,
    STRING_PROTOTYPE_TO_STRING_NAME, STRING_PROTOTYPE_TO_UPPER_CASE_NAME,
    STRING_PROTOTYPE_TRIM_END_NAME, STRING_PROTOTYPE_TRIM_LEFT_NAME, STRING_PROTOTYPE_TRIM_NAME,
    STRING_PROTOTYPE_TRIM_RIGHT_NAME, STRING_PROTOTYPE_TRIM_START_NAME,
    STRING_PROTOTYPE_VALUE_OF_NAME, STRING_RAW_NAME, SYMBOL_NAME,
};
pub(in crate::runtime) use kind::{
    GLOBAL_THIS_NAME, INFINITY_NAME, NAN_NAME, OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME,
    OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_NAME, OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME,
    OBJECT_PROTOTYPE_TO_LOCALE_STRING_NAME, OBJECT_PROTOTYPE_TO_STRING_NAME,
    OBJECT_PROTOTYPE_VALUE_OF_NAME,
};
pub(in crate::runtime::native) use primitive_kind::{
    BOOLEAN_PROTOTYPE_TO_STRING_NAME, BOOLEAN_PROTOTYPE_VALUE_OF_NAME,
    NUMBER_PROTOTYPE_TO_EXPONENTIAL_NAME, NUMBER_PROTOTYPE_TO_FIXED_NAME,
    NUMBER_PROTOTYPE_TO_LOCALE_STRING_NAME, NUMBER_PROTOTYPE_TO_PRECISION_NAME,
    NUMBER_PROTOTYPE_TO_STRING_NAME, NUMBER_PROTOTYPE_VALUE_OF_NAME,
    SYMBOL_PROTOTYPE_TO_STRING_NAME, SYMBOL_PROTOTYPE_VALUE_OF_NAME,
};
pub(in crate::runtime::native) use reflect_kind::REFLECT_NAME;
pub(in crate::runtime) use registry::NativeFunctionRegistry;

#[derive(Debug, Clone)]
pub(in crate::runtime) struct NativeFunction {
    kind: NativeFunctionKind,
    properties: FunctionProperties,
}

impl NativeFunction {
    pub(in crate::runtime::native) fn new(
        kind: NativeFunctionKind,
        prototype: Value,
        name: Value,
    ) -> Self {
        let prototype_default = DataPropertyDescriptor::new(
            prototype.clone(),
            PropertyWritable::No,
            PropertyEnumerable::No,
            PropertyConfigurable::No,
        );
        let intrinsic_defaults = FunctionIntrinsicDefaults::new(
            Value::Number(kind.length()),
            name,
            Some(prototype_default),
        );
        Self {
            kind,
            properties: FunctionProperties::new(prototype, intrinsic_defaults),
        }
    }

    pub(in crate::runtime) const fn kind(&self) -> NativeFunctionKind {
        self.kind
    }

    pub(in crate::runtime) const fn properties(&self) -> &FunctionProperties {
        &self.properties
    }

    pub(in crate::runtime) const fn properties_mut(&mut self) -> &mut FunctionProperties {
        &mut self.properties
    }

    pub(in crate::runtime) fn intrinsic_property(&self, property: &str) -> Option<Value> {
        match self.kind {
            NativeFunctionKind::Number => number_intrinsic_property(property),
            _ => None,
        }
    }

    pub(in crate::runtime) fn has_intrinsic_property(&self, property: &str) -> bool {
        self.intrinsic_property(property).is_some()
    }
}
