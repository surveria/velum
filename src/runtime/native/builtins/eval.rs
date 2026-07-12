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
            Value::String(source) => self.eval_string_source(source, strict_mode, direct),
            Value::HeapString(source) => {
                self.eval_string_source(source.as_str(), strict_mode, direct)
            }
            value => Ok(value.clone()),
        }
    }

    fn eval_string_source(
        &mut self,
        source: &str,
        strict_mode: bool,
        direct: bool,
    ) -> Result<Value> {
        let super_binding = direct.then(|| self.current_super_frame()).flatten();
        let allow_super_property = super_binding.is_some();
        let allow_super_call = super_binding.as_ref().is_some_and(|binding| {
            binding.constructor.is_some() && binding.allow_direct_eval_super_call.get()
        });
        let script = crate::compiled_script::CompiledScript::compile_eval(
            source,
            self.limits.clone(),
            strict_mode,
            allow_super_property,
            allow_super_call,
        )
        .map_err(dynamic_compilation_error)?;
        self.reject_direct_eval_parameter_conflict(&script, strict_mode, direct)?;
        if direct {
            return eval_completion_result(
                self.eval_compiled_eval_completion(&script, script.strict())?,
            );
        }

        let boundary = self.push_eval_activation_boundary()?;
        let result = self.eval_compiled_eval_completion(&script, script.strict());
        let boundary_result = self.pop_eval_activation_boundary(boundary);
        boundary_result?;
        eval_completion_result(result?)
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
