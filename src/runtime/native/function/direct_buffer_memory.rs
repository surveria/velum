use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

use super::NativeFunctionKind;

impl Context {
    pub(super) fn eval_buffer_memory_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::Atomics(kind) => {
                Some(self.eval_atomics_native_function_kind(kind, args))
            }
            NativeFunctionKind::ArrayBuffer => Some(Err(Error::type_error(
                "ArrayBuffer constructor requires 'new'",
            ))),
            NativeFunctionKind::ArrayBufferPrototype(kind) => {
                Some(self.eval_array_buffer_native_function_kind(kind, args, this_value))
            }
            NativeFunctionKind::SharedArrayBuffer => Some(Err(Error::type_error(
                "SharedArrayBuffer constructor requires 'new'",
            ))),
            NativeFunctionKind::SharedArrayBufferPrototype(kind) => {
                Some(self.eval_shared_array_buffer_native_function_kind(kind, args, this_value))
            }
            _ => None,
        }
    }
}
