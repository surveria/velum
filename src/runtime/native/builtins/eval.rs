use crate::{
    error::Result,
    runtime::{Context, call::RuntimeCallArgs, control::Completion},
    value::Value,
};

use super::{EVAL_NAME, NativeFunctionKind, dynamic_compilation_error};

impl Context {
    pub(in crate::runtime::native) fn eval_function_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Eval) {
            return Ok(Value::NativeFunction(id));
        }

        let function = self.create_native_function(NativeFunctionKind::Eval, Value::Undefined)?;
        self.insert_global_builtin(EVAL_NAME, function.clone())?;
        Ok(function)
    }

    pub(in crate::runtime::native) fn eval_eval_function(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_eval_function_with_strict(args, false)
    }

    pub(in crate::runtime) fn eval_eval_function_with_strict(
        &mut self,
        args: RuntimeCallArgs<'_>,
        strict_mode: bool,
    ) -> Result<Value> {
        let Some(argument) = args.as_slice().first() else {
            return Ok(Value::Undefined);
        };

        match argument {
            Value::String(source) => {
                let script = crate::compiled_script::CompiledScript::compile_eval(
                    source,
                    self.limits.clone(),
                    strict_mode,
                )
                .map_err(dynamic_compilation_error)?;
                eval_completion_result(self.eval_compiled_completion(&script)?)
            }
            Value::HeapString(source) => {
                let script = crate::compiled_script::CompiledScript::compile_eval(
                    source.as_str(),
                    self.limits.clone(),
                    strict_mode,
                )
                .map_err(dynamic_compilation_error)?;
                eval_completion_result(self.eval_compiled_completion(&script)?)
            }
            value => Ok(value.clone()),
        }
    }
}

fn eval_completion_result(completion: Completion) -> Result<Value> {
    completion.into_result()
}
