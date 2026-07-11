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
        self.eval_eval_function_with_mode(args, false, false)
    }

    pub(in crate::runtime) fn eval_eval_function_with_strict(
        &mut self,
        args: RuntimeCallArgs<'_>,
        strict_mode: bool,
    ) -> Result<Value> {
        self.eval_eval_function_with_mode(args, strict_mode, true)
    }

    fn eval_eval_function_with_mode(
        &mut self,
        args: RuntimeCallArgs<'_>,
        strict_mode: bool,
        direct: bool,
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
                self.reject_direct_eval_parameter_conflict(&script, strict_mode, direct)?;
                eval_completion_result(self.eval_compiled_completion(&script)?)
            }
            Value::HeapString(source) => {
                let script = crate::compiled_script::CompiledScript::compile_eval(
                    source.as_str(),
                    self.limits.clone(),
                    strict_mode,
                )
                .map_err(dynamic_compilation_error)?;
                self.reject_direct_eval_parameter_conflict(&script, strict_mode, direct)?;
                eval_completion_result(self.eval_compiled_completion(&script)?)
            }
            value => Ok(value.clone()),
        }
    }

    fn reject_direct_eval_parameter_conflict(
        &self,
        script: &crate::compiled_script::CompiledScript,
        strict_mode: bool,
        direct: bool,
    ) -> Result<()> {
        if strict_mode || !direct {
            return Ok(());
        }
        let Some(function_id) = self
            .activation_frames
            .iter()
            .rev()
            .find_map(crate::runtime::activation::ActivationFrame::function_id)
        else {
            return Ok(());
        };
        if !self
            .function(function_id)?
            .bytecode
            .has_parameter_defaults()
        {
            return Ok(());
        }
        let Some(parameter_scope) = self.locals.last() else {
            return Ok(());
        };
        for binding in script.bytecode().hoist_plan().var_declarations() {
            let Some(atom) = self.atom(binding.name().as_str()) else {
                continue;
            };
            if parameter_scope.contains(atom) {
                return Err(crate::error::Error::exception(
                    crate::value::ErrorName::SyntaxError,
                    format!("'{}' has already been declared", binding.name()),
                ));
            }
        }
        Ok(())
    }
}

fn eval_completion_result(completion: Completion) -> Result<Value> {
    completion.into_result()
}
