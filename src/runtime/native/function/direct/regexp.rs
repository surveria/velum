use crate::{
    api::native_call::NativeCallTarget,
    error::Result,
    runtime::{Context, call::RuntimeCallArgs, native::RegExpCallMode},
    value::Value,
};

use super::NativeFunctionKind;

impl Context {
    pub(super) fn eval_direct_regexp_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match target {
            NativeCallTarget::RegExp => {
                Some(self.eval_direct_regexp_constructor(args, RegExpCallMode::Call))
            }
            NativeCallTarget::RegExpPrototypeExec => {
                Some(self.eval_regexp_prototype_exec(RuntimeCallArgs::values(args), this_value))
            }
            NativeCallTarget::RegExpPrototypeTest => {
                Some(self.eval_regexp_prototype_test(RuntimeCallArgs::values(args), this_value))
            }
            _ => None,
        }
    }

    pub(super) fn eval_regexp_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::RegExp => Some(self.eval_regexp_constructor(args)),
            NativeFunctionKind::RegExpEscape => Some(self.eval_regexp_escape(args)),
            NativeFunctionKind::RegExpPrototypeCompile => {
                Some(self.eval_regexp_prototype_compile(args, this_value))
            }
            NativeFunctionKind::RegExpPrototypeDotAllGetter
            | NativeFunctionKind::RegExpPrototypeFlagsGetter
            | NativeFunctionKind::RegExpPrototypeGlobalGetter
            | NativeFunctionKind::RegExpPrototypeHasIndicesGetter
            | NativeFunctionKind::RegExpPrototypeIgnoreCaseGetter
            | NativeFunctionKind::RegExpPrototypeMultilineGetter
            | NativeFunctionKind::RegExpPrototypeSourceGetter
            | NativeFunctionKind::RegExpPrototypeStickyGetter
            | NativeFunctionKind::RegExpPrototypeUnicodeGetter
            | NativeFunctionKind::RegExpPrototypeUnicodeSetsGetter => {
                Some(self.eval_regexp_prototype_getter(kind, this_value))
            }
            NativeFunctionKind::RegExpPrototypeExec => {
                Some(self.eval_regexp_prototype_exec(args, this_value))
            }
            NativeFunctionKind::RegExpPrototypeTest => {
                Some(self.eval_regexp_prototype_test(args, this_value))
            }
            NativeFunctionKind::RegExpPrototypeToString => {
                Some(self.eval_regexp_prototype_to_string(args, this_value))
            }
            NativeFunctionKind::RegExpPrototypeSymbolMatch => {
                Some(self.eval_regexp_prototype_symbol_match(args, this_value))
            }
            NativeFunctionKind::RegExpPrototypeSymbolMatchAll => {
                Some(self.eval_regexp_prototype_symbol_match_all(args, this_value))
            }
            NativeFunctionKind::RegExpPrototypeSymbolReplace => {
                Some(self.eval_regexp_prototype_symbol_replace(args, this_value))
            }
            NativeFunctionKind::RegExpPrototypeSymbolSearch => {
                Some(self.eval_regexp_prototype_symbol_search(args, this_value))
            }
            NativeFunctionKind::RegExpPrototypeSymbolSplit => {
                Some(self.eval_regexp_prototype_symbol_split(args, this_value))
            }
            _ => None,
        }
    }
}
