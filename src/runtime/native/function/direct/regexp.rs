use crate::{
    api::native_call::NativeCallTarget,
    error::Result,
    runtime::{Context, call::RuntimeCallArgs},
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
            NativeCallTarget::RegExp => Some(self.eval_direct_regexp_constructor(args)),
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
            NativeFunctionKind::RegExpPrototypeExec => {
                Some(self.eval_regexp_prototype_exec(args, this_value))
            }
            NativeFunctionKind::RegExpPrototypeTest => {
                Some(self.eval_regexp_prototype_test(args, this_value))
            }
            _ => None,
        }
    }
}
