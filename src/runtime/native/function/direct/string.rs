use crate::{
    api::native_call::NativeCallTarget,
    error::Result,
    runtime::{Context, call::RuntimeCallArgs, native::NativeFunctionKind},
    value::Value,
};

const fn runtime_call_args(args: &[Value]) -> RuntimeCallArgs<'_> {
    RuntimeCallArgs::values(args)
}

impl Context {
    pub(super) fn eval_direct_string_native_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match target {
            NativeCallTarget::String => Some(self.eval_direct_string_constructor(args)),
            NativeCallTarget::StringFromCharCode => {
                Some(self.eval_direct_string_from_char_code(args))
            }
            NativeCallTarget::StringFromCodePoint => {
                Some(self.eval_direct_string_from_code_point(args))
            }
            NativeCallTarget::StringRaw => Some(self.eval_direct_string_raw(args)),
            NativeCallTarget::StringPrototypeAt => {
                Some(self.eval_direct_string_prototype_at(args, this_value))
            }
            NativeCallTarget::StringPrototypeCharAt => {
                Some(self.eval_direct_string_prototype_char_at(args, this_value))
            }
            NativeCallTarget::StringPrototypeCharCodeAt => {
                Some(self.eval_direct_string_prototype_char_code_at(args, this_value))
            }
            NativeCallTarget::StringPrototypeCodePointAt => {
                Some(self.eval_direct_string_prototype_code_point_at(args, this_value))
            }
            NativeCallTarget::StringPrototypeConcat => {
                Some(self.eval_direct_string_prototype_concat(args, this_value))
            }
            NativeCallTarget::StringPrototypeEndsWith => {
                Some(self.eval_direct_string_prototype_ends_with(args, this_value))
            }
            NativeCallTarget::StringPrototypeIncludes => {
                Some(self.eval_direct_string_prototype_includes(args, this_value))
            }
            NativeCallTarget::StringPrototypeIndexOf => {
                Some(self.eval_direct_string_prototype_index_of(args, this_value))
            }
            NativeCallTarget::StringPrototypeLastIndexOf => {
                Some(self.eval_direct_string_prototype_last_index_of(args, this_value))
            }
            NativeCallTarget::StringPrototypeMatch => {
                Some(self.eval_string_prototype_match(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypePadEnd => {
                Some(self.eval_string_prototype_pad_end(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypePadStart => {
                Some(self.eval_string_prototype_pad_start(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeRepeat => {
                Some(self.eval_direct_string_prototype_repeat(args, this_value))
            }
            NativeCallTarget::StringPrototypeReplace => {
                Some(self.eval_string_prototype_replace(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeSearch => {
                Some(self.eval_string_prototype_search(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeSlice => {
                Some(self.eval_direct_string_prototype_slice(args, this_value))
            }
            NativeCallTarget::StringPrototypeSplit => {
                Some(self.eval_string_prototype_split(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeStartsWith => {
                Some(self.eval_direct_string_prototype_starts_with(args, this_value))
            }
            NativeCallTarget::StringPrototypeSubstring => {
                Some(self.eval_direct_string_prototype_substring(args, this_value))
            }
            NativeCallTarget::StringPrototypeToLocaleLowerCase
            | NativeCallTarget::StringPrototypeToLowerCase => {
                Some(self.eval_string_prototype_to_lower_case(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeToLocaleUpperCase
            | NativeCallTarget::StringPrototypeToUpperCase => {
                Some(self.eval_string_prototype_to_upper_case(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeToString => {
                Some(self.eval_string_prototype_to_string(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeTrim => {
                Some(self.eval_string_prototype_trim(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeTrimEnd => {
                Some(self.eval_string_prototype_trim_end(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeTrimStart => {
                Some(self.eval_string_prototype_trim_start(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeValueOf => {
                Some(self.eval_string_prototype_value_of(runtime_call_args(args), this_value))
            }
            _ => None,
        }
    }

    pub(super) fn eval_string_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::String => Some(self.eval_string_constructor(args)),
            NativeFunctionKind::StringPrototypeAnnexB(kind) => {
                Some(self.eval_string_prototype_annex_b(kind, args, this_value))
            }
            NativeFunctionKind::StringFromCharCode => Some(self.eval_string_from_char_code(args)),
            NativeFunctionKind::StringFromCodePoint => Some(self.eval_string_from_code_point(args)),
            NativeFunctionKind::StringRaw => Some(self.eval_string_raw(args)),
            NativeFunctionKind::StringPrototypeAt => {
                Some(self.eval_string_prototype_at(args, this_value))
            }
            NativeFunctionKind::StringPrototypeCharAt => {
                Some(self.eval_string_prototype_char_at(args, this_value))
            }
            NativeFunctionKind::StringPrototypeCharCodeAt => {
                Some(self.eval_string_prototype_char_code_at(args, this_value))
            }
            NativeFunctionKind::StringPrototypeCodePointAt => {
                Some(self.eval_string_prototype_code_point_at(args, this_value))
            }
            NativeFunctionKind::StringPrototypeConcat => {
                Some(self.eval_string_prototype_concat(args, this_value))
            }
            NativeFunctionKind::StringPrototypeEndsWith => {
                Some(self.eval_string_prototype_ends_with(args, this_value))
            }
            NativeFunctionKind::StringPrototypeIncludes => {
                Some(self.eval_string_prototype_includes(args, this_value))
            }
            NativeFunctionKind::StringPrototypeIndexOf => {
                Some(self.eval_string_prototype_index_of(args, this_value))
            }
            NativeFunctionKind::StringPrototypeLastIndexOf => {
                Some(self.eval_string_prototype_last_index_of(args, this_value))
            }
            NativeFunctionKind::StringPrototypeLocaleCompare => {
                Some(self.eval_string_prototype_locale_compare(args, this_value))
            }
            NativeFunctionKind::StringPrototypeMatch => {
                Some(self.eval_string_prototype_match(args, this_value))
            }
            NativeFunctionKind::StringPrototypeNormalize => {
                Some(self.eval_string_prototype_normalize(args, this_value))
            }
            NativeFunctionKind::StringPrototypePadEnd => {
                Some(self.eval_string_prototype_pad_end(args, this_value))
            }
            NativeFunctionKind::StringPrototypePadStart => {
                Some(self.eval_string_prototype_pad_start(args, this_value))
            }
            NativeFunctionKind::StringPrototypeRepeat => {
                Some(self.eval_string_prototype_repeat(args, this_value))
            }
            NativeFunctionKind::StringPrototypeReplace => {
                Some(self.eval_string_prototype_replace(args, this_value))
            }
            NativeFunctionKind::StringPrototypeSearch => {
                Some(self.eval_string_prototype_search(args, this_value))
            }
            NativeFunctionKind::StringPrototypeSlice => {
                Some(self.eval_string_prototype_slice(args, this_value))
            }
            NativeFunctionKind::StringPrototypeSplit => {
                Some(self.eval_string_prototype_split(args, this_value))
            }
            NativeFunctionKind::StringPrototypeStartsWith => {
                Some(self.eval_string_prototype_starts_with(args, this_value))
            }
            NativeFunctionKind::StringPrototypeSubstring => {
                Some(self.eval_string_prototype_substring(args, this_value))
            }
            NativeFunctionKind::StringPrototypeToLocaleLowerCase
            | NativeFunctionKind::StringPrototypeToLowerCase => {
                Some(self.eval_string_prototype_to_lower_case(args, this_value))
            }
            NativeFunctionKind::StringPrototypeToLocaleUpperCase
            | NativeFunctionKind::StringPrototypeToUpperCase => {
                Some(self.eval_string_prototype_to_upper_case(args, this_value))
            }
            NativeFunctionKind::StringPrototypeToString => {
                Some(self.eval_string_prototype_to_string(args, this_value))
            }
            NativeFunctionKind::StringPrototypeTrim => {
                Some(self.eval_string_prototype_trim(args, this_value))
            }
            NativeFunctionKind::StringPrototypeTrimEnd => {
                Some(self.eval_string_prototype_trim_end(args, this_value))
            }
            NativeFunctionKind::StringPrototypeTrimStart => {
                Some(self.eval_string_prototype_trim_start(args, this_value))
            }
            NativeFunctionKind::StringPrototypeValueOf => {
                Some(self.eval_string_prototype_value_of(args, this_value))
            }
            _ => self.eval_modern_string_native_function_kind(kind, args, this_value),
        }
    }
}
