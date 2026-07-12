use crate::{
    api::native_call::NativeCallTarget,
    error::Result,
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

use super::NativeFunctionKind;

impl Context {
    pub(in crate::runtime) fn eval_direct_primitive_native_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match target {
            NativeCallTarget::BooleanPrototypeToString => {
                Some(self.eval_direct_boolean_prototype_to_string(this_value))
            }
            NativeCallTarget::BooleanPrototypeValueOf => {
                Some(self.eval_direct_boolean_prototype_value_of(this_value))
            }
            NativeCallTarget::NumberPrototypeToLocaleString
            | NativeCallTarget::NumberPrototypeToString => {
                Some(self.eval_direct_number_prototype_to_string(args, this_value))
            }
            NativeCallTarget::NumberPrototypeValueOf => {
                Some(self.eval_direct_number_prototype_value_of(this_value))
            }
            NativeCallTarget::SymbolPrototypeDescriptionGetter => {
                Some(self.eval_direct_symbol_prototype_description(this_value))
            }
            NativeCallTarget::SymbolPrototypeToString => {
                Some(self.eval_direct_symbol_prototype_to_string(this_value))
            }
            NativeCallTarget::SymbolPrototypeValueOf => {
                Some(self.eval_direct_symbol_prototype_value_of(this_value))
            }
            _ => None,
        }
    }

    pub(in crate::runtime) fn eval_primitive_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::BooleanPrototypeToString => {
                Some(self.eval_boolean_prototype_to_string(args, this_value))
            }
            NativeFunctionKind::BooleanPrototypeValueOf => {
                Some(self.eval_boolean_prototype_value_of(args, this_value))
            }
            NativeFunctionKind::BigIntPrototypeToLocaleString => {
                Some(self.eval_bigint_prototype_to_locale_string(args, this_value))
            }
            NativeFunctionKind::BigIntPrototypeToString => {
                Some(self.eval_bigint_prototype_to_string(args, this_value))
            }
            NativeFunctionKind::BigIntPrototypeValueOf => {
                Some(self.eval_bigint_prototype_value_of(args, this_value))
            }
            NativeFunctionKind::NumberPrototypeToLocaleString
            | NativeFunctionKind::NumberPrototypeToString => {
                Some(self.eval_number_prototype_to_string(args, this_value))
            }
            NativeFunctionKind::NumberPrototypeValueOf => {
                Some(self.eval_number_prototype_value_of(args, this_value))
            }
            NativeFunctionKind::NumberPrototypeToFixed => {
                Some(self.eval_number_prototype_to_fixed(args, this_value))
            }
            NativeFunctionKind::NumberPrototypeToExponential => {
                Some(self.eval_number_prototype_to_exponential(args, this_value))
            }
            NativeFunctionKind::NumberPrototypeToPrecision => {
                Some(self.eval_number_prototype_to_precision(args, this_value))
            }
            NativeFunctionKind::SymbolPrototypeDescriptionGetter => {
                Some(self.eval_symbol_prototype_description(args, this_value))
            }
            NativeFunctionKind::SymbolPrototypeToPrimitive => {
                Some(self.eval_symbol_prototype_to_primitive(args, this_value))
            }
            NativeFunctionKind::SymbolPrototypeToString => {
                Some(self.eval_symbol_prototype_to_string(args, this_value))
            }
            NativeFunctionKind::SymbolPrototypeValueOf => {
                Some(self.eval_symbol_prototype_value_of(args, this_value))
            }
            _ => None,
        }
    }
}
